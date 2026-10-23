//! Portable, bounded image material. Decode only on import/load; snapshots share pixels.
use std::{
    io::{Cursor, Read},
    path::Path,
    sync::{Arc, OnceLock, mpsc},
};

use egui::{Color32, Pos2};
use image::ImageDecoder;
use serde::{Deserialize, Serialize};

const MAX_EDGE: u32 = 1024;
const MAX_FILE: usize = 32 * 1024 * 1024;
const MAX_PNG: usize = 5 * 1024 * 1024;

#[derive(Clone)]
pub(super) struct Asset(Arc<Data>);
struct Data {
    name: String,
    png: String,
    pixels: egui::ColorImage,
}
impl std::fmt::Debug for Asset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WaterfallImage")
            .field("name", &self.0.name)
            .field("size", &self.0.pixels.size)
            .finish()
    }
}
impl PartialEq for Asset {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0) || (self.0.name == other.0.name && self.0.png == other.0.png)
    }
}
impl Serialize for Asset {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct Stored<'a> {
            name: &'a str,
            png: &'a str,
        }
        Stored {
            name: &self.0.name,
            png: &self.0.png,
        }
        .serialize(serializer)
    }
}
impl<'de> Deserialize<'de> for Asset {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Stored {
            name: String,
            png: String,
        }
        let stored = Stored::deserialize(deserializer)?;
        if stored.name.is_empty()
            || stored.name.len() > 240
            || stored.png.len() > MAX_PNG * 2
            || !stored.png.len().is_multiple_of(2)
        {
            return Err(serde::de::Error::custom("Invalid Waterfall image asset"));
        }
        let bytes = unhex(&stored.png).map_err(serde::de::Error::custom)?;
        let decoded = decode(&bytes, MAX_EDGE).map_err(serde::de::Error::custom)?;
        if image::guess_format(&bytes).ok() != Some(image::ImageFormat::Png) {
            return Err(serde::de::Error::custom("Expected embedded PNG image"));
        }
        Ok(Self(Arc::new(Data {
            name: stored.name,
            png: stored.png,
            pixels: decoded,
        })))
    }
}

fn decode(bytes: &[u8], max_edge: u32) -> Result<egui::ColorImage, String> {
    let mut reader = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    if !matches!(
        reader.format(),
        Some(image::ImageFormat::Png | image::ImageFormat::Jpeg | image::ImageFormat::Bmp)
    ) {
        return Err("Choose a PNG, JPEG, or BMP image.".into());
    }
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(max_edge);
    limits.max_image_height = Some(max_edge);
    limits.max_alloc = Some(256 * 1024 * 1024);
    reader.limits(limits);
    let mut decoder = reader
        .into_decoder()
        .map_err(|e| format!("Could not decode image: {e}"))?;
    let orientation = decoder.orientation().map_err(|e| e.to_string())?;
    let mut decoded = image::DynamicImage::from_decoder(decoder).map_err(|e| e.to_string())?;
    decoded.apply_orientation(orientation);
    if decoded.width() > MAX_EDGE || decoded.height() > MAX_EDGE {
        decoded = decoded.resize(MAX_EDGE, MAX_EDGE, image::imageops::FilterType::Lanczos3);
    }
    let rgba = decoded.into_rgba8();
    if rgba.width() == 0 || rgba.height() == 0 {
        return Err("Image is empty.".into());
    }
    Ok(egui::ColorImage::from_rgba_unmultiplied(
        [rgba.width() as usize, rgba.height() as usize],
        &rgba,
    ))
}
fn unhex(value: &str) -> Result<Vec<u8>, String> {
    value
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| {
            let digit = |c: u8| {
                (c as char)
                    .to_digit(16)
                    .ok_or_else(|| "Invalid embedded image data".to_owned())
            };
            Ok((digit(pair[0])? * 16 + digit(pair[1])?) as u8)
        })
        .collect()
}
impl Asset {
    pub(super) fn starter() -> Self {
        static STARTER: OnceLock<Asset> = OnceLock::new();
        STARTER
            .get_or_init(|| {
                Self::from_bytes(
                    "Starter landscape",
                    include_bytes!("../../../assets/waterfall/starter.jpeg"),
                )
                .expect("validated bundled Waterfall starter image")
            })
            .clone()
    }
    fn from_bytes(name: &str, bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MAX_FILE {
            return Err("Choose an image smaller than 32 MB.".into());
        }
        let pixels = decode(bytes, 8192)?;
        let raw: Vec<_> = pixels
            .pixels
            .iter()
            .flat_map(Color32::to_srgba_unmultiplied)
            .collect();
        let mut png = Cursor::new(Vec::new());
        image::write_buffer_with_format(
            &mut png,
            &raw,
            pixels.size[0] as u32,
            pixels.size[1] as u32,
            image::ExtendedColorType::Rgba8,
            image::ImageFormat::Png,
        )
        .map_err(|e| e.to_string())?;
        let png = png.into_inner();
        if png.len() > MAX_PNG {
            return Err("Image is too large to store in a preset.".into());
        }
        const HEX: &[u8] = b"0123456789abcdef";
        let encoded: String = png
            .iter()
            .flat_map(|b| {
                [
                    HEX[(b >> 4) as usize] as char,
                    HEX[(b & 15) as usize] as char,
                ]
            })
            .collect();
        let name: String = name.chars().take(60).collect();
        Ok(Self(Arc::new(Data {
            name,
            png: encoded,
            pixels,
        })))
    }
    fn import(path: &Path) -> Result<Self, String> {
        let mut bytes = Vec::new();
        std::fs::File::open(path)
            .map_err(|e| e.to_string())?
            .take(MAX_FILE as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        Self::from_bytes(
            &path.file_name().unwrap_or_default().to_string_lossy(),
            &bytes,
        )
    }
    pub(super) fn sample(&self, uv: Pos2) -> Color32 {
        let [w, h] = self.0.pixels.size;
        let x = uv.x.clamp(0.0, 1.0) * (w - 1) as f32;
        let y = uv.y.clamp(0.0, 1.0) * (h - 1) as f32;
        let (x0, y0) = (x.floor() as usize, y.floor() as usize);
        let at = |x, y| self.0.pixels.pixels[y * w + x];
        let top = at(x0, y0).lerp_to_gamma(at((x0 + 1).min(w - 1), y0), x.fract());
        let bottom = at(x0, (y0 + 1).min(h - 1))
            .lerp_to_gamma(at((x0 + 1).min(w - 1), (y0 + 1).min(h - 1)), x.fract());
        top.lerp_to_gamma(bottom, y.fract())
    }
    pub(super) fn uv(&self, uv: Pos2) -> Pos2 {
        let [w, h] = self.0.pixels.size;
        // Two white rows reserve (0,0) for ordinary untextured mesh vertices.
        Pos2::new(
            (0.5 + uv.x * (w - 1) as f32) / w as f32,
            (2.5 + uv.y * (h - 1) as f32) / (h + 2) as f32,
        )
    }
    fn atlas(&self) -> egui::ColorImage {
        let [w, h] = self.0.pixels.size;
        let mut pixels = vec![Color32::WHITE; w * 2];
        pixels.extend_from_slice(&self.0.pixels.pixels);
        egui::ColorImage::new([w, h + 2], pixels)
    }
}

#[derive(Default)]
pub(super) struct Runtime {
    loaded: Option<(Asset, egui::TextureHandle)>,
    pending: Option<mpsc::Receiver<Result<Asset, String>>>,
    error: Option<String>,
}
impl Runtime {
    pub(super) fn texture_id(&self) -> egui::TextureId {
        self.loaded
            .as_ref()
            .map_or(egui::TextureId::default(), |(_, handle)| handle.id())
    }
    pub(super) fn poll(&mut self, ctx: &egui::Context, selected: &mut Option<Asset>) -> bool {
        if let Some(pending) = &self.pending {
            match pending.try_recv() {
                Ok(result) => {
                    self.pending = None;
                    match result {
                        Ok(asset) => {
                            *selected = Some(asset);
                            self.error = None;
                            return true;
                        }
                        Err(error) => self.error = Some(error),
                    }
                }
                Err(mpsc::TryRecvError::Empty) => {
                    ctx.request_repaint_after(std::time::Duration::from_millis(50))
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.pending = None;
                    self.error = Some("Image import stopped. Please try again.".into());
                }
            }
        }
        false
    }
    pub(super) fn prepare(&mut self, ctx: &egui::Context, asset: &Asset) -> egui::TextureId {
        if !self
            .loaded
            .as_ref()
            .is_some_and(|(previous, _)| previous == asset)
        {
            self.loaded = Some((
                asset.clone(),
                ctx.load_texture(
                    "waterfall-material",
                    asset.atlas(),
                    egui::TextureOptions::LINEAR,
                ),
            ));
        }
        self.loaded.as_ref().unwrap().1.id()
    }
    pub(super) fn remove(&mut self, selected: &mut Option<Asset>, removed: &mut bool) {
        *selected = None;
        *removed = true;
        self.pending = None;
        self.loaded = None;
        self.error = None;
    }
    pub(super) fn controls(
        &mut self,
        ui: &mut egui::Ui,
        selected: &mut Option<Asset>,
        removed: &mut bool,
    ) {
        use crate::help::HoverHelp;
        if self.poll(ui.ctx(), selected) {
            *removed = false;
        }
        if !*removed {
            let asset = selected.clone().unwrap_or_else(Asset::starter);
            let id = self.prepare(ui.ctx(), &asset);
            let uv = egui::Rect::from_min_max(asset.uv(Pos2::ZERO), asset.uv(Pos2::new(1.0, 1.0)));
            ui.horizontal(|ui| {
                ui.add(
                    egui::Image::new((id, egui::vec2(58.0, 58.0)))
                        .uv(uv)
                        .corner_radius(3.0),
                );
                ui.vertical(|ui| {
                    ui.label(&asset.0.name);
                    ui.weak(format!(
                        "{} × {}",
                        asset.0.pixels.size[0], asset.0.pixels.size[1]
                    ));
                });
            });
        } else {
            egui::Frame::new()
                .inner_margin(egui::Margin::same(10))
                .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
                .show(ui, |ui| {
                    ui.set_min_width((ui.available_width() - 20.0).max(0.0));
                    ui.label("No image selected");
                    ui.weak("Upload an image to map it onto the waterfall.");
                });
        }
        ui.add_space(5.0);
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(self.pending.is_none(), egui::Button::new(if *removed { "Upload image…" } else { "Replace / upload image…" }))
                .help_text("Choose a PNG, JPEG or BMP from your computer. The image is stored in the scene and presets, so the original file can be moved afterward. Replacing an image keeps the current one until the new file imports successfully.")
                .clicked()
            {
                match choose() {
                    Ok(Some(path)) => {
                        self.error = None;
                        let (send, receive) = mpsc::sync_channel(1);
                        self.pending = Some(receive);
                        std::thread::spawn(move || {
                            let _ = send.send(Asset::import(&path));
                        });
                    }
                    Ok(None) => {}
                    Err(error) => self.error = Some(error),
                }
            }
            if ui.add_enabled(!*removed, egui::Button::new("Remove image"))
                .help_text("Remove this image, including the stock starter, from the waterfall. Leaves an empty upload slot and shows gradient colors until you choose another image. Undo restores it. The original file on your computer is untouched.").clicked() {
                self.remove(selected, removed);
            }
        });
        if (*removed || selected.is_some()) && ui.small_button("Restore starter image").clicked() {
            *selected = None;
            *removed = false;
            self.pending = None;
            self.error = None;
        }
        if self.pending.is_some() {
            ui.weak("Importing image…");
            ui.ctx().request_repaint();
        }
        if let Some(error) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
    }
}

fn choose() -> Result<Option<std::path::PathBuf>, String> {
    #[cfg(target_os = "macos")]
    {
        unsafe extern "C" {
            fn symphos_image_choose() -> *mut std::ffi::c_char;
            fn symphos_project_path_free(path: *mut std::ffi::c_char);
        }
        // SAFETY: called on the AppKit main thread; free the bridge allocation once.
        unsafe {
            let path = symphos_image_choose();
            if path.is_null() {
                return Ok(None);
            }
            let result = std::ffi::CStr::from_ptr(path)
                .to_string_lossy()
                .into_owned();
            symphos_project_path_free(path);
            Ok(Some(result.into()))
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let output = std::process::Command::new("zenity")
            .args([
                "--file-selection",
                "--title=Choose Waterfall Image",
                "--file-filter=Images | *.png *.jpg *.jpeg *.bmp *.PNG *.JPG *.JPEG *.BMP",
            ])
            .output()
            .map_err(|_| {
                "Could not open the image picker. Install zenity to choose images.".to_owned()
            })?;
        Ok(output
            .status
            .success()
            .then(|| String::from_utf8_lossy(&output.stdout).trim().into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(super) fn fixture() -> Asset {
        let mut bytes = Cursor::new(Vec::new());
        image::write_buffer_with_format(
            &mut bytes,
            &[
                255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
            ],
            2,
            2,
            image::ExtendedColorType::Rgba8,
            image::ImageFormat::Png,
        )
        .unwrap();
        Asset::from_bytes("Test colors.png", bytes.get_ref()).unwrap()
    }

    #[test]
    fn imported_pixels_are_portable_bounded_and_keep_image_corners() {
        let asset = fixture();
        assert_eq!(asset.sample(Pos2::ZERO), Color32::RED);
        assert_eq!(asset.sample(Pos2::new(1.0, 0.0)), Color32::GREEN);
        assert_eq!(asset.sample(Pos2::new(0.0, 1.0)), Color32::BLUE);
        assert_eq!(asset.sample(Pos2::new(1.0, 1.0)), Color32::WHITE);
        let restored: Asset = serde_json::from_slice(&serde_json::to_vec(&asset).unwrap()).unwrap();
        assert_eq!(restored, asset);
        assert_eq!(restored.0.pixels, asset.0.pixels);
        let starter = Asset::starter();
        assert_eq!(starter.0.pixels.size, [1024, 1024]);
        assert!(starter.0.png.len() <= MAX_PNG * 2);
        assert!(Arc::ptr_eq(&starter.0, &Asset::starter().0));
    }

    #[test]
    fn image_atlas_keeps_mesh_overlays_white_and_reuses_the_gpu_upload() {
        let asset = fixture();
        let atlas = asset.atlas();
        assert!(
            atlas.pixels[..4]
                .iter()
                .all(|pixel| *pixel == Color32::WHITE)
        );
        assert_eq!(atlas.pixels[4], Color32::RED);
        let first_uv = asset.uv(Pos2::ZERO);
        let last_uv = asset.uv(Pos2::new(1.0, 1.0));
        assert_eq!(first_uv, Pos2::new(0.25, 0.625));
        assert_eq!(last_uv, Pos2::new(0.75, 0.875));
        let ctx = egui::Context::default();
        let mut runtime = Runtime::default();
        let first = runtime.prepare(&ctx, &asset);
        assert_eq!(first, runtime.prepare(&ctx, &asset.clone()));
        assert_ne!(first, runtime.prepare(&ctx, &Asset::starter()));
    }

    #[test]
    fn failed_import_keeps_the_previous_material_and_embedded_data_is_validated() {
        let original = fixture();
        let mut selected = Some(original.clone());
        let mut runtime = Runtime::default();
        let (send, receive) = mpsc::sync_channel(1);
        send.send(Err("Not a supported image".into())).unwrap();
        runtime.pending = Some(receive);
        runtime.poll(&egui::Context::default(), &mut selected);
        assert_eq!(selected, Some(original));
        assert!(runtime.pending.is_none());
        assert!(runtime.error.is_some());
        for png in ["z0", "0", "0000", ""] {
            assert!(
                serde_json::from_value::<Asset>(serde_json::json!({"name":"test", "png":png}))
                    .is_err()
            );
        }
        assert!(Asset::from_bytes("test", b"not an image").is_err());
    }

    #[test]
    fn removal_cancels_pending_import_and_a_new_upload_can_fill_the_empty_slot() {
        let mut selected = Some(fixture());
        let mut removed = false;
        let mut runtime = Runtime::default();
        let (send, receive) = mpsc::sync_channel(1);
        runtime.pending = Some(receive);
        runtime.remove(&mut selected, &mut removed);
        assert!(
            removed && selected.is_none() && runtime.loaded.is_none() && runtime.pending.is_none()
        );
        assert!(
            send.send(Ok(fixture())).is_err(),
            "late imports cannot restore a removed image"
        );
        let (send, receive) = mpsc::sync_channel(1);
        send.send(Ok(fixture())).unwrap();
        runtime.pending = Some(receive);
        assert!(runtime.poll(&egui::Context::default(), &mut selected));
        assert!(selected.is_some(), "a new upload fills the slot");
    }
}
