//! Project image catalog and browser. Saved scenes keep their embedded images;
//! the catalog retains imports which are not referenced by a saved scene.
use egui::{Color32, FontId, Pos2, Rect, RichText, Sense, Vec2};

use crate::{
    modules::{ImageAsset, ModulePane, ModuleSettings},
    project::SceneState,
    theme::AppTheme,
    workspace_ui,
};

pub const MAX_ASSETS: usize = 128;
pub const CHOOSE_IMAGE_REQUEST: &str = "choose-project-image";
pub const PROJECT_SCOPE: &str = "project-media-scope";
const IMPORTS: &str = "project-media-imports";

pub fn register_import(ctx: &egui::Context, scope: u64, image: ImageAsset) {
    ctx.data_mut(|data| {
        let queue = data.get_temp_mut_or_default::<Vec<(u64, ImageAsset)>>(egui::Id::new(IMPORTS));
        if queue.len() < MAX_ASSETS {
            queue.push((scope, image));
        }
    });
    ctx.request_repaint();
}

pub fn take_imports(ctx: &egui::Context, scope: u64) -> Vec<ImageAsset> {
    ctx.data_mut(|data| data.remove_temp::<Vec<(u64, ImageAsset)>>(egui::Id::new(IMPORTS)))
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(owner, image)| (owner == scope).then_some(image))
        .collect()
}

pub fn scene_images(state: &SceneState) -> impl Iterator<Item = &ImageAsset> {
    state
        .panes
        .iter()
        .flat_map(|pane| &pane.settings)
        .filter_map(|settings| {
            if let ModuleSettings::Waterfall(settings) = settings {
                settings.imported_image()
            } else {
                None
            }
        })
}

/// Generic project resources. Opaque files are cataloged without decoding video
/// frames or changing the media capabilities of an individual visualizer.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum Resource {
    Image(ImageAsset),
    File(std::sync::Arc<StoredFile>),
}

#[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct StoredFile {
    pub name: String,
    /// Portable file bytes encoded as hex, like existing embedded texture data.
    pub data: String,
}
impl Resource {
    pub fn image(&self) -> Option<&ImageAsset> {
        if let Self::Image(image) = self {
            Some(image)
        } else {
            None
        }
    }
    pub fn valid(&self) -> bool {
        match self {
            Self::Image(_) => true, // ImageAsset validates its embedded data on decode.
            Self::File(file) => {
                !file.name.is_empty()
                    && file.name.len() <= 240
                    && !file
                        .name
                        .chars()
                        .any(|c| c.is_control() || matches!(c, '/' | '\\'))
                    && file.data.len() <= 32 * 1024 * 1024 * 2
                    && !file.data.is_empty()
                    && file.data.len().is_multiple_of(2)
                    && file.data.bytes().all(|b| b.is_ascii_hexdigit())
            }
        }
    }
}

pub fn remember(images: &mut Vec<Resource>, image: ImageAsset) -> bool {
    if image == ImageAsset::starter()
        || images
            .iter()
            .any(|resource| resource.image() == Some(&image))
        || images.len() >= MAX_ASSETS
    {
        return false;
    }
    images.push(Resource::Image(image));
    true
}

pub fn remember_file(media: &mut Vec<Resource>, name: &str, data: &str) {
    if media.len() < MAX_ASSETS && !media.iter().any(|resource| {
        matches!(resource, Resource::File(file) if file.name == name && file.data == data)
    }) {
        media.push(Resource::File(std::sync::Arc::new(StoredFile { name: name.into(), data: data.into() })));
    }
}

pub fn scene_files(state: &SceneState) -> impl Iterator<Item = (&str, &str)> {
    state
        .panes
        .iter()
        .flat_map(|pane| &pane.milkdrop_presets)
        .flat_map(|preset| {
            preset
                .textures
                .iter()
                .map(|(name, data)| (name.as_str(), data.as_str()))
        })
}

pub fn preserve_scene(media: &mut Vec<Resource>, state: &SceneState) {
    for image in scene_images(state) {
        remember(media, image.clone());
    }
    for (name, data) in scene_files(state) {
        remember_file(media, name, data);
    }
}

pub struct Entry {
    pub image: ImageAsset,
    pub locations: Vec<String>,
    pub placeholder: bool,
}

pub fn entries(book: &crate::scenes::Book, panes: &[ModulePane]) -> Vec<Entry> {
    let mut entries: Vec<Entry> = Vec::new();
    let mut add = |image: &ImageAsset, location: Option<String>| {
        let index = entries
            .iter()
            .position(|entry| entry.image == *image)
            .unwrap_or_else(|| {
                entries.push(Entry {
                    image: image.clone(),
                    locations: Vec::new(),
                    placeholder: false,
                });
                entries.len() - 1
            });
        if let Some(location) = location
            && !entries[index].locations.contains(&location)
        {
            entries[index].locations.push(location);
        }
    };
    for image in book.media.iter().filter_map(Resource::image) {
        add(image, None);
    }
    for scene in &book.scenes {
        for image in scene_images(&scene.state) {
            add(
                image,
                (scene.id != book.active_scene).then(|| scene.name.clone()),
            );
        }
    }
    let current_name = book
        .current()
        .map_or("Current scene", |scene| scene.name.as_str());
    for pane in panes {
        if let Some(image) = pane.imported_image() {
            add(image, Some(current_name.to_owned()));
        }
    }
    entries.sort_by_cached_key(|entry| entry.image.name().to_lowercase());
    entries.insert(
        0,
        Entry {
            image: ImageAsset::starter(),
            locations: Vec::new(),
            placeholder: true,
        },
    );
    entries
}

pub struct FileEntry {
    pub name: String,
    pub bytes: usize,
    pub locations: Vec<String>,
    pub kind: &'static str,
}

fn file_kind(name: &str) -> &'static str {
    match std::path::Path::new(name)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "gif" => "GIF",
        "mp4" | "mov" | "m4v" | "webm" | "avi" => "Video",
        "png" | "jpg" | "jpeg" | "bmp" | "webp" => "Image",
        "dds" | "tga" | "ktx" | "ktx2" => "Texture",
        _ => "File",
    }
}

pub struct Catalog {
    pub images: Vec<Entry>,
    pub files: Vec<FileEntry>,
}

pub fn catalog(book: &crate::scenes::Book, panes: &[ModulePane]) -> Catalog {
    let mut textures: Vec<(&str, &str, Vec<String>)> = Vec::new();
    let mut add = |name, data, location: Option<&str>| {
        let index = textures
            .iter()
            .position(|(n, d, _)| *n == name && *d == data)
            .unwrap_or_else(|| {
                textures.push((name, data, Vec::new()));
                textures.len() - 1
            });
        if let Some(location) = location
            && !textures[index]
                .2
                .iter()
                .any(|existing| existing == location)
        {
            textures[index].2.push(location.to_owned());
        }
    };
    for scene in &book.scenes {
        for (name, data) in scene_files(&scene.state) {
            add(
                name,
                data,
                (scene.id != book.active_scene).then_some(scene.name.as_str()),
            );
        }
    }
    let current_name = book
        .current()
        .map_or("Current scene", |scene| scene.name.as_str());
    for pane in panes {
        for (name, data) in pane.media_textures() {
            add(name, data, Some(current_name));
        }
    }
    for resource in &book.media {
        if let Resource::File(file) = resource {
            add(&file.name, &file.data, None);
        }
    }
    let mut textures: Vec<_> = textures
        .into_iter()
        .map(|(name, data, locations)| FileEntry {
            name: name.to_owned(),
            bytes: data.len() / 2,
            locations,
            kind: file_kind(name),
        })
        .collect();
    textures.sort_by_cached_key(|entry| entry.name.to_lowercase());
    Catalog {
        images: entries(book, panes),
        files: textures,
    }
}

struct Thumbnail {
    image: ImageAsset,
    texture: egui::TextureHandle,
}

pub struct Browser {
    selected: Option<ImageAsset>,
    thumbnails: Vec<Thumbnail>,
    pub picking_for: Option<String>,
    pub scope: u64,
    seen_presets: Vec<std::sync::Weak<crate::milkdrop::preset::Preset>>,
}
impl Default for Browser {
    fn default() -> Self {
        static SCOPE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        Self {
            selected: None,
            thumbnails: Vec::new(),
            picking_for: None,
            scope: SCOPE.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            seen_presets: Vec::new(),
        }
    }
}

pub struct Response {
    pub close: bool,
    pub apply: Option<ImageAsset>,
}

impl Browser {
    pub fn retain_uploads(&mut self, media: &mut Vec<Resource>, panes: &[ModulePane]) {
        self.seen_presets.retain(|preset| preset.strong_count() > 0);
        for pane in panes {
            if let Some(image) = pane.imported_image() {
                remember(media, image.clone());
            }
            for preset in pane
                .media_presets()
                .iter()
                .filter(|preset| !preset.textures.is_empty())
            {
                let weak = std::sync::Arc::downgrade(preset);
                if self.seen_presets.iter().any(|seen| seen.ptr_eq(&weak)) {
                    continue;
                }
                for (name, data) in &preset.textures {
                    remember_file(media, name, data);
                }
                self.seen_presets.push(weak);
            }
        }
    }
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        rect: Rect,
        theme: &AppTheme,
        catalog: &Catalog,
    ) -> Response {
        let entries = &catalog.images;
        let (mut child, close) = workspace_ui::browser_frame(ui, rect, theme, "Media");
        let mut apply = None;
        self.thumbnails
            .retain(|thumb| entries.iter().any(|entry| entry.image == thumb.image));
        if self
            .selected
            .as_ref()
            .is_some_and(|selected| !entries.iter().any(|entry| entry.image == *selected))
        {
            self.selected = None;
        }
        egui::Frame::new().inner_margin(10).show(&mut child, |ui| {
            ui.label(RichText::new("Project media").strong());
            let count = entries.len() + catalog.files.len();
            ui.weak(format!(
                "{count} asset{}",
                if count == 1 { "" } else { "s" }
            ));
            if self.picking_for.is_some() {
                ui.label("Choose an image");
                ui.weak("Click an image to use it, or press Esc to cancel.");
            }
        });
        workspace_ui::sidebar_rule(&mut child, rect);
        egui::ScrollArea::vertical()
            .id_salt("project-media")
            .auto_shrink([false, false])
            .show(&mut child, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                if entries.is_empty() && catalog.files.is_empty() {
                    egui::Frame::new().inner_margin(12).show(ui, |ui| {
                        ui.label("No media yet");
                        ui.weak("Media uploaded in your inspectors appears here.");
                        ui.weak(
                            "Images are saved with this project and available across its scenes.",
                        );
                    });
                }
                for (index, entry) in entries.iter().enumerate() {
                    let (row, response) = ui
                        .allocate_exact_size(Vec2::new(ui.available_width(), 82.0), Sense::click());
                    #[cfg(test)]
                    ui.data_mut(|data| {
                        data.insert_temp(egui::Id::new(("test-media-image", index)), row)
                    });
                    let selected = self.selected.as_ref() == Some(&entry.image);
                    if selected || response.hovered() {
                        ui.painter().rect_filled(
                            row,
                            0.0,
                            theme
                                .foreground
                                .gamma_multiply(if selected { 0.07 } else { 0.035 }),
                        );
                    }
                    if ui.is_rect_visible(row) {
                        let texture = if let Some(thumb) = self
                            .thumbnails
                            .iter()
                            .find(|thumb| thumb.image == entry.image)
                        {
                            &thumb.texture
                        } else {
                            self.thumbnails.push(Thumbnail {
                                image: entry.image.clone(),
                                texture: ui.ctx().load_texture(
                                    format!("project-media-{index}"),
                                    entry.image.thumbnail(),
                                    egui::TextureOptions::LINEAR,
                                ),
                            });
                            &self.thumbnails.last().unwrap().texture
                        };
                        let size = texture.size_vec2();
                        let size = size * (60.0 / size.max_elem());
                        let thumbnail = Rect::from_center_size(
                            Pos2::new(row.left() + 40.0, row.center().y),
                            size,
                        );
                        ui.painter().image(
                            texture.id(),
                            thumbnail,
                            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                            Color32::WHITE,
                        );
                        let left = row.left() + 80.0;
                        let width = (row.right() - left - 10.0).max(1.0);
                        for (line, text, color, font) in [
                            (0, entry.image.name().to_owned(), theme.foreground, 12.0),
                            (
                                1,
                                format!("{} × {}", entry.image.size()[0], entry.image.size()[1]),
                                theme.muted,
                                11.0,
                            ),
                            (
                                2,
                                if entry.placeholder {
                                    "Built-in placeholder".into()
                                } else if entry.locations.is_empty() {
                                    "Not used in a scene".into()
                                } else {
                                    entry.locations.join(", ")
                                },
                                theme.muted,
                                11.0,
                            ),
                        ] {
                            let mut job = egui::text::LayoutJob::simple(
                                text,
                                FontId::proportional(font),
                                color,
                                width,
                            );
                            job.wrap.max_rows = 1;
                            let galley = ui.painter().layout_job(job);
                            ui.painter().galley(
                                Pos2::new(left, row.top() + 14.0 + line as f32 * 18.0),
                                galley,
                                color,
                            );
                        }
                    }
                    if response.clicked() {
                        self.selected = Some(entry.image.clone());
                        if self.picking_for.is_some() {
                            apply = Some(entry.image.clone());
                        }
                    }
                    response
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .on_hover_text(format!(
                            "{}\n{}",
                            entry.image.name(),
                            if entry.locations.is_empty() {
                                "Available throughout this project".into()
                            } else {
                                entry.locations.join("\n")
                            }
                        ));
                }
                if !catalog.files.is_empty() {
                    workspace_ui::sidebar_rule(ui, rect);
                    egui::CollapsingHeader::new("Other media")
                        .default_open(true)
                        .show_unindented(ui, |ui| {
                            for texture in &catalog.files {
                                egui::Frame::new().inner_margin(10).show(ui, |ui| {
                                    ui.add(egui::Label::new(&texture.name).truncate())
                                        .on_hover_text(&texture.name);
                                    ui.weak(format!(
                                        "{} · {:.0} KB",
                                        texture.kind,
                                        texture.bytes as f32 / 1024.0
                                    ));
                                    ui.weak(if texture.locations.is_empty() {
                                        "Available in this project".into()
                                    } else {
                                        texture.locations.join(", ")
                                    });
                                });
                            }
                        });
                }
            });
        Response { close, apply }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub fn image(name: &str) -> ImageAsset {
        let mut bytes = std::io::Cursor::new(Vec::new());
        image::write_buffer_with_format(
            &mut bytes,
            &[255, 0, 0, 255, 0, 255, 0, 255],
            2,
            1,
            image::ExtendedColorType::Rgba8,
            image::ImageFormat::Png,
        )
        .unwrap();
        ImageAsset::from_bytes(name, bytes.get_ref()).unwrap()
    }

    fn scene(pane: &ModulePane) -> SceneState {
        SceneState {
            background: Default::default(),
            macros: Some(Default::default()),
            signals: Default::default(),
            panes: vec![pane.snapshot()],
            layout: Some(crate::workspace::Tile::Pane(0)),
            selected_pane: 0,
            analysis: Default::default(),
        }
    }

    #[test]
    fn catalog_covers_all_scenes_uses_live_active_panes_and_excludes_stock_images() {
        let mut pane = ModulePane::new(crate::modules::ModuleKind::Waterfall);
        let mut book = crate::scenes::Book::new(scene(&pane));
        assert!(entries(&book, &[pane])[0].placeholder);
        pane = ModulePane::new(crate::modules::ModuleKind::Waterfall);
        pane.use_image(image("First.png"));
        book.store(scene(&pane));
        book.insert("Another scene".into(), scene(&pane)).unwrap();
        pane.use_image(image("Second.png"));
        let catalog = entries(&book, &[pane]);
        assert_eq!(catalog.len(), 3);
        assert!(catalog[0].placeholder);
        assert_eq!(catalog[1].image.name(), "First.png");
        assert_eq!(catalog[1].locations, ["Scene 1"]);
        assert_eq!(catalog[2].locations, ["Another scene"]);
        assert!(catalog[1..].iter().all(|entry| entry.locations.len() == 1));
    }

    #[test]
    fn project_roundtrip_preserves_unused_imports_without_duplicating_referenced_images() {
        let used = image("Used.png");
        let unused = image("Unused.png");
        let mut pane = ModulePane::new(crate::modules::ModuleKind::Waterfall);
        pane.use_image(used.clone());
        let mut book = crate::scenes::Book::new(scene(&pane));
        assert!(remember(&mut book.media, used));
        assert!(remember(&mut book.media, unused.clone()));
        assert!(!remember(&mut book.media, unused));
        book.store(scene(&pane));
        assert_eq!(book.media.len(), 1);
        let project = crate::project::Project {
            format: "symphos-project".into(),
            version: 2,
            scenes: book,
        };
        let bytes = serde_json::to_vec(&project).unwrap();
        assert_eq!(
            String::from_utf8_lossy(&bytes).matches("\"png\"").count(),
            2
        );
        let restored = crate::project::Project::decode(&bytes).unwrap();
        assert_eq!(entries(&restored.scenes, &[pane]).len(), 3);
        assert_eq!(
            restored.scenes.media[0].image().unwrap().name(),
            "Unused.png"
        );
    }

    #[test]
    fn replacing_images_and_deleting_scenes_retains_their_imports() {
        let mut pane = ModulePane::new(crate::modules::ModuleKind::Waterfall);
        pane.use_image(image("First.png"));
        let mut book = crate::scenes::Book::new(scene(&pane));
        pane.use_image(image("Second.png"));
        book.store(scene(&pane));
        assert_eq!(book.media[0].image().unwrap().name(), "First.png");
        let original = book.active_scene;
        let fresh = ModulePane::new(crate::modules::ModuleKind::Waterfall);
        book.insert("Empty".into(), scene(&fresh)).unwrap();
        book.remove(original);
        let catalog = entries(&book, &[fresh]);
        assert_eq!(catalog.len(), 3);
        assert!(catalog.iter().all(|entry| entry.locations.is_empty()));
    }

    #[test]
    fn browser_reuses_small_thumbnails() {
        let ctx = egui::Context::default();
        let image = image("Preview.png");
        let mut browser = Browser {
            selected: Some(image.clone()),
            ..Default::default()
        };
        let catalog = Catalog {
            images: vec![Entry {
                image: image.clone(),
                locations: Vec::new(),
                placeholder: false,
            }],
            files: Vec::new(),
        };
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(300.0, 700.0));
        let mut texture = None;
        for _ in 0..2 {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(rect),
                    ..Default::default()
                },
                |ui| {
                    browser.show(ui, rect, &AppTheme::default(), &catalog);
                },
            );
            output.textures_delta.clear();
            assert_eq!(browser.thumbnails.len(), 1);
            let handle = &browser.thumbnails[0].texture;
            assert!(handle.size().iter().all(|edge| *edge <= 96));
            if let Some(previous) = texture {
                assert_eq!(handle.id(), previous);
            }
            texture = Some(handle.id());
        }
    }

    #[test]
    fn completed_uploads_stay_with_their_original_project() {
        let ctx = egui::Context::default();
        let first = Browser::default();
        let next = Browser::default();
        let uploaded = image("Uploaded.png");
        register_import(&ctx, first.scope, uploaded.clone());
        assert_eq!(
            take_imports(&ctx, first.scope).as_slice(),
            std::slice::from_ref(&uploaded)
        );
        register_import(&ctx, first.scope, uploaded);
        assert!(take_imports(&ctx, next.scope).is_empty());
    }

    #[test]
    fn generic_files_roundtrip_and_the_browser_does_not_name_a_component() {
        let pane = ModulePane::new(crate::modules::ModuleKind::Waterfall);
        let mut book = crate::scenes::Book::new(scene(&pane));
        for name in ["Animation.gif", "Clip.mov", "Surface.dds"] {
            book.media
                .push(Resource::File(std::sync::Arc::new(StoredFile {
                    name: name.into(),
                    data: "00010203".into(),
                })));
        }
        let project = crate::project::Project {
            format: "symphos-project".into(),
            version: 2,
            scenes: book,
        };
        let project =
            crate::project::Project::decode(&serde_json::to_vec(&project).unwrap()).unwrap();
        let catalog = catalog(&project.scenes, &[pane]);
        assert_eq!(
            catalog
                .files
                .iter()
                .map(|file| file.kind)
                .collect::<Vec<_>>(),
            ["GIF", "Video", "Texture"]
        );
        assert!(catalog.images[0].placeholder);
        let mut browser = Browser::default();
        let ctx = egui::Context::default();
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(300.0, 900.0));
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(rect),
                ..Default::default()
            },
            |ui| {
                browser.show(ui, rect, &AppTheme::default(), &catalog);
            },
        );
        output.textures_delta.clear();
        for name in [
            "Animation.gif",
            "Clip.mov",
            "Surface.dds",
            "Starter landscape",
        ] {
            assert!(
                output
                    .shapes
                    .iter()
                    .any(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.job.text == name))
            );
        }
        assert!(!output.shapes.iter().any(
            |s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.job.text.contains("Waterfall"))
        ));
        let unsafe_file = Resource::File(std::sync::Arc::new(StoredFile {
            name: "../escape.mp4".into(),
            data: "00".into(),
        }));
        assert!(!unsafe_file.valid());
    }

    #[test]
    fn milkdrop_textures_are_listed_with_their_project_scene_usage() {
        let mut pane = ModulePane::new(crate::modules::ModuleKind::Milkdrop).snapshot();
        let mut preset = (*pane.milkdrop_presets[0]).clone();
        preset.textures.insert("Imported.dds".into(), "0000".into());
        pane.milkdrop_presets = vec![std::sync::Arc::new(preset)];
        let mut live = ModulePane::new(crate::modules::ModuleKind::Milkdrop);
        live.restore(&pane);
        let mut book = crate::scenes::Book::new(scene(&live));
        let listing = catalog(&book, std::slice::from_ref(&live));
        assert_eq!(listing.files.len(), 1);
        assert_eq!(listing.files[0].name, "Imported.dds");
        assert_eq!(listing.files[0].bytes, 2);
        assert_eq!(listing.files[0].locations, ["Scene 1"]);
        let mut browser = Browser::default();
        browser.retain_uploads(&mut book.media, std::slice::from_ref(&live));
        assert_eq!(book.media.len(), 1);
        book.store(scene(&live));
        assert!(
            book.media.is_empty(),
            "referenced textures are not embedded again in the catalog"
        );
        let fresh = ModulePane::new(crate::modules::ModuleKind::Milkdrop);
        let listing = catalog(&book, std::slice::from_ref(&fresh));
        assert_eq!(
            listing.files.len(),
            1,
            "removed textures stay available before the scene cache refreshes"
        );
        assert!(listing.files[0].locations.is_empty());
        book.store(scene(&fresh));
        assert_eq!(book.media.len(), 1);
        let json = serde_json::to_string(&book).unwrap();
        assert_eq!(json.matches("\"data\":\"0000\"").count(), 1);
    }
}
