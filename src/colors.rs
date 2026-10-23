//! Shared component palettes and a portable, local color library.
use crate::{help::HoverHelp, theme::AppTheme};
use eframe::egui::{self, Color32, FontId, Rect, Sense, Stroke, Vec2};
use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

const MAX_COLORS: usize = 8;
const MAX_SAVED: usize = 64;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Gradient {
    pub colors: [[u8; 3]; MAX_COLORS],
    #[serde(default = "opaque_stops")]
    pub alpha: [u8; MAX_COLORS],
    #[serde(deserialize_with = "valid_count")]
    pub count: u8,
    #[serde(default)]
    pub cyclic: bool,
}
fn opaque_stops() -> [u8; MAX_COLORS] {
    [255; MAX_COLORS]
}

fn valid_count<'de, D: serde::Deserializer<'de>>(d: D) -> Result<u8, D::Error> {
    let n = u8::deserialize(d)?;
    if (2..=MAX_COLORS as u8).contains(&n) {
        Ok(n)
    } else {
        Err(serde::de::Error::custom("a gradient needs 2–8 colors"))
    }
}
impl Gradient {
    fn from_hex(colors: &[u32]) -> Self {
        let mut stops = [[0; 3]; MAX_COLORS];
        for (out, &c) in stops.iter_mut().zip(colors) {
            *out = [(c >> 16) as u8, (c >> 8) as u8, c as u8];
        }
        Self {
            colors: stops,
            count: colors.len() as u8,
            alpha: [255; 8],
            cyclic: false,
        }
    }
    pub fn color(self, t: f32) -> Color32 {
        let [r, g, b, a] = self.sample_rgba(t);
        Color32::from_rgba_unmultiplied(r, g, b, a)
    }
    fn sample_rgba(self, t: f32) -> [u8; 4] {
        let n = self.count.clamp(2, MAX_COLORS as u8) as usize;
        if self.cyclic {
            let x = t.rem_euclid(1.0) * n as f32;
            let i = x as usize;
            return self.interpolate(i % n, (i + 1) % n, x.fract());
        }
        let x = t.clamp(0.0, 1.0) * (n - 1) as f32;
        let i = (x as usize).min(n - 2);
        self.interpolate(i, i + 1, x - i as f32)
    }
    fn interpolate(self, i: usize, j: usize, t: f32) -> [u8; 4] {
        let t = t.clamp(0.0, 1.0);
        std::array::from_fn(|channel| {
            let a = if channel == 3 {
                self.alpha[i]
            } else {
                self.colors[i][channel]
            };
            let b = if channel == 3 {
                self.alpha[j]
            } else {
                self.colors[j][channel]
            };
            (a as f32 + (b as f32 - a as f32) * t) as u8
        })
    }
    pub fn gpu(self) -> [[f32; 4]; MAX_COLORS] {
        std::array::from_fn(|i| {
            let c = self.colors[i];
            [
                c[0] as f32 / 255.0,
                c[1] as f32 / 255.0,
                c[2] as f32 / 255.0,
                self.alpha[i] as f32 / 255.0,
            ]
        })
    }
    fn resample(self, count: u8) -> Self {
        let mut next = self;
        next.count = count;
        for i in 0..count as usize {
            let sample = self.sample_rgba(
                i as f32
                    / if self.cyclic {
                        count as f32
                    } else {
                        (count - 1) as f32
                    },
            );
            next.colors[i] = sample[..3].try_into().unwrap();
            next.alpha[i] = sample[3];
        }
        next
    }
}
fn blend(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let c = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t) as u8;
    let a = a.to_srgba_unmultiplied();
    let b = b.to_srgba_unmultiplied();
    Color32::from_rgba_unmultiplied(c(a[0], b[0]), c(a[1], b[1]), c(a[2], b[2]), c(a[3], b[3]))
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Palette {
    #[default]
    Theme,
    Ember,
    Ocean,
    Heatmap,
    Spectrum,
    Ice,
    Lime,
    Pearl,
    Viridis,
    Magma,
    Inferno,
    Plasma,
    Aurora,
    ClassicLed,
    PixelFire,
    Phosphor,
    AmberCrt,
    NeonNights,
    CandySunset,
    Ultraviolet,
    Monochrome,
    // Existing component presets retain their original names and colors.
    Solar,
    Ion,
    Kiln,
    Glacier,
    Custom(Gradient),
}
impl Palette {
    pub const FACTORY: [Self; 20] = [
        Self::Spectrum,
        Self::Ocean,
        Self::Ember,
        Self::Heatmap,
        Self::Viridis,
        Self::Magma,
        Self::Inferno,
        Self::Plasma,
        Self::Aurora,
        Self::Ice,
        Self::Lime,
        Self::Pearl,
        Self::ClassicLed,
        Self::PixelFire,
        Self::Phosphor,
        Self::AmberCrt,
        Self::NeonNights,
        Self::CandySunset,
        Self::Ultraviolet,
        Self::Monochrome,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Theme => "Default colors",
            Self::Ember => "Ember",
            Self::Ocean => "Ocean",
            Self::Heatmap => "Heatmap",
            Self::Spectrum => "Spectrum",
            Self::Ice => "Ice",
            Self::Lime => "Electric lime",
            Self::Pearl => "Pearl",
            Self::Viridis => "Viridis",
            Self::Magma => "Magma",
            Self::Inferno => "Inferno",
            Self::Plasma => "Plasma",
            Self::Aurora => "Aurora",
            Self::ClassicLed => "Classic LED",
            Self::PixelFire => "Pixel fire",
            Self::Phosphor => "Phosphor green",
            Self::AmberCrt => "Amber CRT",
            Self::NeonNights => "Neon nights",
            Self::CandySunset => "Candy sunset",
            Self::Ultraviolet => "Ultraviolet",
            Self::Monochrome => "Monochrome",
            Self::Solar => "Solar",
            Self::Ion => "Ion",
            Self::Kiln => "Kiln",
            Self::Glacier => "Glacier",
            Self::Custom(_) => "Custom",
        }
    }
    pub fn gradient(self, theme: &AppTheme) -> Gradient {
        if let Self::Custom(g) = self {
            return g;
        }
        if self == Self::Theme {
            return Gradient {
                colors: [
                    theme.accent_alt.to_array()[..3].try_into().unwrap(),
                    theme.accent.to_array()[..3].try_into().unwrap(),
                    [0; 3],
                    [0; 3],
                    [0; 3],
                    [0; 3],
                    [0; 3],
                    [0; 3],
                ],
                count: 2,
                alpha: [255; 8],
                cyclic: false,
            };
        }
        let mut gradient = Gradient::from_hex(match self {
            Self::Ember => &[0x8c2c46, 0xff8e41, 0xffb546],
            Self::Ocean => &[0x2d48ac, 0x268abf, 0x50e1d6],
            Self::Heatmap => &[
                0x2028a0, 0x2464f0, 0x00c8e6, 0x2dcd73, 0xf5e037, 0xff8c23, 0xf02d23,
            ],
            Self::Spectrum => &[0xffd441, 0xff7131, 0xd056ee, 0x5e67ff, 0x30e1e3, 0x9fe755],
            Self::Viridis => &[0x440154, 0x3b528b, 0x21918c, 0x5ec962, 0xfde725],
            Self::Magma => &[0x000004, 0x51127c, 0xb73779, 0xfb8761, 0xfcfdbf],
            Self::Inferno => &[0x000004, 0x57106e, 0xbc3754, 0xf98e09, 0xfcffa4],
            Self::Plasma => &[0x0d0887, 0x7e03a8, 0xcc4778, 0xf89540, 0xf0f921],
            Self::Aurora => &[0x092e44, 0x12a895, 0x9bf0c3, 0x837df4, 0xeb8fc3],
            Self::ClassicLed => &[0x139c37, 0x52ec42, 0xd2f03b, 0xffc52b, 0xff493b],
            Self::PixelFire => &[0x9e1208, 0xe83908, 0xff8712, 0xffc822, 0xf5ed50],
            Self::Phosphor => &[0x0e4427, 0x20a951, 0x68ed8c, 0xd3ffb1],
            Self::AmberCrt => &[0x60300c, 0xd87516, 0xffb833, 0xffe59e],
            Self::NeonNights => &[0x4632c7, 0xa63cf0, 0xff3cbe, 0x37d8ff],
            Self::CandySunset => &[0x5f3ca9, 0xdf5bc2, 0xff8b9c, 0xffd08d],
            Self::Ultraviolet => &[0x241344, 0x673bd1, 0xa57bff, 0xe5d4ff],
            Self::Monochrome => &[0x42464c, 0x8c949f, 0xd3d9e0, 0xffffff],
            Self::Ice => &[0x16477a, 0x41dcff, 0xe5faff],
            Self::Lime => &[0x205530, 0x96ff23, 0xf1ffad],
            Self::Pearl => &[0x535b69, 0xb2bccb, 0xf2f5fa],
            Self::Solar => &[0xffef91, 0xffb32a, 0xff5d37, 0xe13d79, 0xfb823d, 0xffd55f],
            Self::Ion => &[0x6af7e9, 0x3da4ff, 0x5c53e8, 0xbb61fc, 0xe6acff, 0x5fdfff],
            Self::Kiln => &[0xe1daaa, 0xff4c32],
            Self::Glacier => &[0xa0e3f5, 0x4364fa],
            Self::Theme | Self::Custom(_) => unreachable!(),
        });
        gradient.cyclic = matches!(self, Self::Spectrum | Self::Solar | Self::Ion);
        gradient
    }
    pub fn color(self, value: f32, theme: &AppTheme) -> Color32 {
        // Retain the original analyzer mapping for existing palettes.
        if matches!(self, Self::Theme | Self::Ember | Self::Ocean) {
            let (low, high) = match self {
                Self::Theme => (theme.accent_alt, theme.accent),
                Self::Ember => (
                    Color32::from_rgb(140, 44, 70),
                    Color32::from_rgb(255, 181, 70),
                ),
                _ => (
                    Color32::from_rgb(45, 72, 172),
                    Color32::from_rgb(80, 225, 214),
                ),
            };
            if value < 0.4 {
                blend(theme.background, low, value / 0.4)
            } else if value < 0.85 {
                blend(low, high, (value - 0.4) / 0.45)
            } else {
                blend(high, theme.foreground, (value - 0.85) / 0.15)
            }
        } else {
            self.gradient(theme).color(value)
        }
    }
    pub fn cycle(self, t: f32, theme: &AppTheme) -> Color32 {
        // Flux's existing cyclic palettes keep a seamless last-to-first segment.
        if matches!(self, Self::Spectrum | Self::Solar | Self::Ion | Self::Theme) {
            let g = if self == Self::Theme {
                Gradient {
                    colors: [
                        theme.accent.to_array()[..3].try_into().unwrap(),
                        theme.accent_alt.to_array()[..3].try_into().unwrap(),
                        theme.foreground.to_array()[..3].try_into().unwrap(),
                        theme.accent_alt.to_array()[..3].try_into().unwrap(),
                        theme.accent.to_array()[..3].try_into().unwrap(),
                        theme.foreground.to_array()[..3].try_into().unwrap(),
                        [0; 3],
                        [0; 3],
                    ],
                    count: 6,
                    alpha: [255; 8],
                    cyclic: true,
                }
            } else {
                self.gradient(theme)
            };
            let x = t.rem_euclid(1.0) * g.count as f32;
            let i = x as usize;
            let [r, g, b, a] =
                g.interpolate(i % g.count as usize, (i + 1) % g.count as usize, x.fract());
            Color32::from_rgba_unmultiplied(r, g, b, a)
        } else {
            let g = self.gradient(theme);
            g.color(if g.cyclic {
                t
            } else {
                1.0 - (t.rem_euclid(1.0) * 2.0 - 1.0).abs()
            })
        }
    }
    pub fn cellular(self, t: f32, theme: &AppTheme) -> Color32 {
        match self {
            Self::Lime => Color32::from_rgb(150, 255, 35),
            Self::Ice => Color32::from_rgb(65, 220, 255),
            Self::Ember => Color32::from_rgb(255, 142, 65),
            Self::Theme => theme.accent,
            _ => self.gradient(theme).color(t),
        }
    }
    pub fn strata(self, t: f32, theme: &AppTheme) -> Color32 {
        match self {
            Self::Pearl => theme.foreground,
            Self::Theme => blend(theme.foreground, theme.accent, t),
            _ => self.gradient(theme).color(t),
        }
    }
    pub fn contrast_controls(self, ui: &mut egui::Ui, contrast: &mut f32) {
        if self == Self::Heatmap {
            ui.add(crate::parameter::Parameter::new(contrast, 0.25..=3.0, 1.0).text("Contrast"))
                .help_text("Color contrast only; signal levels and heights are unchanged.");
        }
    }
    pub fn color_with_contrast(self, t: f32, contrast: f32, theme: &AppTheme) -> Color32 {
        let t = if self == Self::Heatmap {
            let t = t.clamp(0.0, 1.0);
            let a = t.powf(contrast);
            a / (a + (1.0 - t).powf(contrast))
        } else {
            t
        };
        self.color(t, theme)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct NamedPalette {
    name: String,
    gradient: Gradient,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Swatch {
    name: String,
    color: [u8; 3],
    #[serde(default = "crate::color_picker::opaque")]
    alpha: u8,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct LibraryData {
    #[serde(default = "library_version")]
    version: u32,
    palettes: Vec<NamedPalette>,
    swatches: Vec<Swatch>,
}
fn library_version() -> u32 {
    1
}
impl Default for LibraryData {
    fn default() -> Self {
        Self {
            version: 1,
            palettes: vec![],
            swatches: vec![],
        }
    }
}
struct Library {
    path: PathBuf,
    data: LibraryData,
    notice: Option<String>,
}
impl Library {
    fn load(path: PathBuf) -> Self {
        let mut library = Self {
            path,
            data: LibraryData::default(),
            notice: None,
        };
        match std::fs::read(&library.path) {
            Ok(bytes) => match Self::decode(&bytes) {
                Some(data) => library.data = data,
                None => {
                    library.notice = Some(
                        "Color library could not be read; the original file was retained.".into(),
                    );
                    let _ =
                        std::fs::copy(&library.path, library.path.with_extension("rejected.json"));
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => library.notice = Some(format!("Could not read colors: {e}")),
        };
        library
    }
    fn decode(bytes: &[u8]) -> Option<LibraryData> {
        if bytes.len() > 1_048_576 {
            return None;
        }
        let data: LibraryData = serde_json::from_slice(bytes).ok()?;
        let valid = |names: Vec<&str>| {
            names.len() <= MAX_SAVED
                && names
                    .iter()
                    .all(|n| !n.trim().is_empty() && n.chars().count() <= 40)
                && names
                    .iter()
                    .enumerate()
                    .all(|(i, a)| names.iter().skip(i + 1).all(|b| !a.eq_ignore_ascii_case(b)))
        };
        (data.version == 1
            && valid(data.palettes.iter().map(|p| p.name.as_str()).collect())
            && valid(data.swatches.iter().map(|s| s.name.as_str()).collect()))
        .then_some(data)
    }
    fn save(&mut self) {
        let result = serde_json::to_vec_pretty(&self.data)
            .map_err(std::io::Error::other)
            .and_then(|bytes| crate::workspace::atomic_write(&self.path, &bytes));
        if let Err(e) = result {
            self.notice = Some(format!("Could not save colors: {e}"));
            crate::issues::record("Color library", self.notice.as_deref().unwrap());
        } else {
            self.notice = Some("Saved to color library".into());
        }
    }
}
fn library(ctx: &egui::Context) -> Arc<Mutex<Library>> {
    let id = egui::Id::new("color-library");
    if let Some(library) = ctx.data(|d| d.get_temp::<Arc<Mutex<Library>>>(id)) {
        return library;
    }
    let library = Arc::new(Mutex::new(Library::load(
        crate::workspace::directory().join("colors.json"),
    )));
    ctx.data_mut(|d| d.insert_temp(id, library.clone()));
    library
}
#[derive(Clone)]
struct Editor {
    gradient: Gradient,
    source: Palette,
    name: String,
    swatch_name: String,
    selected: usize,
}
fn paint_gradient(ui: &egui::Ui, rect: Rect, g: Gradient) {
    let mut mesh = egui::Mesh::default();
    for i in 0..64 {
        let a = i as f32 / 64.0;
        let b = (i + 1) as f32 / 64.0;
        let base = mesh.vertices.len() as u32;
        for (x, y, c) in [
            (a, rect.top(), g.color(a)),
            (b, rect.top(), g.color(b)),
            (b, rect.bottom(), g.color(b)),
            (a, rect.bottom(), g.color(a)),
        ] {
            mesh.colored_vertex(egui::pos2(egui::lerp(rect.x_range(), x), y), c);
        }
        mesh.add_triangle(base, base + 1, base + 2);
        mesh.add_triangle(base, base + 2, base + 3);
    }
    ui.painter().add(egui::Shape::mesh(mesh));
}
fn tile(ui: &mut egui::Ui, name: &str, g: Gradient, selected: bool, width: f32) -> bool {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 38.0), Sense::click());
    ui.painter()
        .rect_filled(rect, 3.0, ui.visuals().widgets.inactive.bg_fill);
    paint_gradient(
        ui,
        Rect::from_min_max(rect.min + Vec2::new(4.0, 24.0), rect.max - Vec2::splat(4.0)),
        g,
    );
    ui.painter_at(rect).text(
        rect.min + Vec2::new(7.0, 5.0),
        egui::Align2::LEFT_TOP,
        name,
        FontId::proportional(12.0),
        ui.visuals().text_color(),
    );
    if selected || response.hovered() {
        ui.painter().rect_stroke(
            rect,
            3.0,
            Stroke::new(1.0, ui.visuals().selection.bg_fill),
            egui::StrokeKind::Inside,
        );
    }
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, name));
    response.help_text(name).clicked()
}
impl Palette {
    pub fn controls(&mut self, ui: &mut egui::Ui) {
        let theme = ui
            .ctx()
            .data(|d| d.get_temp::<AppTheme>(egui::Id::new("component-color-theme")))
            .unwrap_or_default();
        let shared = library(ui.ctx());
        let mut library = shared.lock().unwrap_or_else(|e| e.into_inner());
        let name = if let Self::Custom(g) = self {
            library
                .data
                .palettes
                .iter()
                .find(|p| p.gradient == *g)
                .map(|p| p.name.as_str())
                .unwrap_or("Custom")
        } else {
            self.label()
        }
        .to_string();
        crate::modules::property_row(ui, "Palette", |ui| {
            crate::dropdown::ComboBox::from_id_salt("component-palette")
                .width(ui.available_width())
                .truncate()
                .height(460.0)
                .selected_text(name)
                .show_ui(ui, |ui| {
                    ui.set_width(282.0);
                    ui.spacing_mut().item_spacing = Vec2::splat(4.0);
                    ui.spacing_mut().button_padding = Vec2::new(5.0, 2.0);
                    if tile(
                        ui,
                        Self::Theme.label(),
                        Self::Theme.gradient(&theme),
                        *self == Self::Theme,
                        282.0,
                    ) {
                        *self = Self::Theme;
                        ui.close();
                    }
                    for pair in Self::FACTORY.chunks(2) {
                        ui.horizontal(|ui| {
                            for &p in pair {
                                if tile(ui, p.label(), p.gradient(&theme), *self == p, 139.0) {
                                    *self = p;
                                    ui.close();
                                }
                            }
                        });
                    }
                    if !library.data.palettes.is_empty() {
                        ui.separator();
                        ui.weak("Saved palettes");
                        egui::ScrollArea::vertical()
                            .max_height(180.0)
                            .show(ui, |ui| {
                                for p in &library.data.palettes {
                                    if tile(
                                        ui,
                                        &p.name,
                                        p.gradient,
                                        *self == Self::Custom(p.gradient),
                                        282.0,
                                    ) {
                                        *self = Self::Custom(p.gradient);
                                        ui.close();
                                    }
                                }
                            });
                    }
                });
        });
        let (preview, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), 14.0), Sense::hover());
        paint_gradient(ui, preview, self.gradient(&theme));
        let id = ui.id().with("gradient-editor");
        let mut editor = ui
            .data(|d| d.get_temp::<Editor>(id))
            .unwrap_or_else(|| Editor {
                gradient: self.gradient(&theme),
                source: *self,
                name: String::new(),
                swatch_name: String::new(),
                selected: 0,
            });
        if editor.source != *self {
            editor.gradient = self.gradient(&theme);
            editor.source = *self;
            editor.selected = editor.selected.min(editor.gradient.count as usize - 1);
        }
        let mut changed = false;
        let before = editor.gradient;
        egui::CollapsingHeader::new("Edit gradient & swatches")
            .id_salt("gradient-customize")
            .show_unindented(ui, |ui| {
                for i in 0..editor.gradient.count as usize {
                    ui.push_id(i, |ui| {
                        ui.horizontal(|ui| {
                            if ui
                                .selectable_label(editor.selected == i, format!("{}", i + 1))
                                .clicked()
                            {
                                editor.selected = i;
                            }
                            if crate::color_picker::color_edit_button_srgba(
                                ui,
                                &mut editor.gradient.colors[i],
                                &mut editor.gradient.alpha[i],
                            )
                            .changed()
                            {
                                editor.selected = i;
                            }
                        });
                    });
                }
                ui.checkbox(&mut editor.gradient.cyclic, "Cyclic gradient");
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add_enabled(
                            editor.gradient.count < 8,
                            egui::Button::new("+ Stop").small(),
                        )
                        .clicked()
                    {
                        editor.gradient = editor.gradient.resample(editor.gradient.count + 1);
                    }
                    if ui
                        .add_enabled(
                            editor.gradient.count > 2,
                            egui::Button::new("− Stop").small(),
                        )
                        .clicked()
                    {
                        editor.gradient = editor.gradient.resample(editor.gradient.count - 1);
                        editor.selected = editor.selected.min(editor.gradient.count as usize - 1);
                    }
                    if ui.small_button("Reverse").clicked() {
                        editor.gradient.colors[..editor.gradient.count as usize].reverse();
                        editor.gradient.alpha[..editor.gradient.count as usize].reverse();
                    }
                    if ui.small_button("Solid color").clicked() {
                        let color = editor.gradient.colors[editor.selected];
                        editor.gradient.colors.fill(color);
                        let alpha = editor.gradient.alpha[editor.selected];
                        editor.gradient.alpha.fill(alpha);
                        editor.gradient.count = 2;
                        editor.gradient.cyclic = false;
                        editor.selected = 0;
                    }
                });
                ui.separator();
                ui.weak("Save palette");
                ui.add(
                    egui::TextEdit::singleline(&mut editor.name)
                        .hint_text("Palette name")
                        .char_limit(40)
                        .desired_width(ui.available_width()),
                );
                let existing = library
                    .data
                    .palettes
                    .iter()
                    .position(|p| p.name.eq_ignore_ascii_case(editor.name.trim()));
                if ui
                    .add_enabled(
                        !editor.name.trim().is_empty()
                            && (existing.is_some() || library.data.palettes.len() < MAX_SAVED),
                        egui::Button::new(if existing.is_some() {
                            "Update palette"
                        } else {
                            "Save palette"
                        })
                        .small(),
                    )
                    .clicked()
                {
                    let p = NamedPalette {
                        name: editor.name.trim().into(),
                        gradient: editor.gradient,
                    };
                    if let Some(i) = existing {
                        library.data.palettes[i] = p;
                    } else {
                        library.data.palettes.push(p);
                    }
                    library.save();
                    changed = true;
                }
                ui.separator();
                ui.weak("Swatches · apply to selected stop");
                ui.add(
                    egui::TextEdit::singleline(&mut editor.swatch_name)
                        .hint_text("Swatch name")
                        .char_limit(40)
                        .desired_width(ui.available_width()),
                );
                let existing = library
                    .data
                    .swatches
                    .iter()
                    .position(|s| s.name.eq_ignore_ascii_case(editor.swatch_name.trim()));
                if ui
                    .add_enabled(
                        !editor.swatch_name.trim().is_empty()
                            && (existing.is_some() || library.data.swatches.len() < MAX_SAVED),
                        egui::Button::new(if existing.is_some() {
                            "Update swatch"
                        } else {
                            "Save selected color"
                        })
                        .small(),
                    )
                    .clicked()
                {
                    let s = Swatch {
                        name: editor.swatch_name.trim().into(),
                        color: editor.gradient.colors[editor.selected],
                        alpha: editor.gradient.alpha[editor.selected],
                    };
                    if let Some(i) = existing {
                        library.data.swatches[i] = s;
                    } else {
                        library.data.swatches.push(s);
                    }
                    library.save();
                }
                let mut remove = None;
                ui.horizontal_wrapped(|ui| {
                    for (i, s) in library.data.swatches.iter().enumerate() {
                        ui.push_id(i, |ui| {
                            let (rect, response) =
                                ui.allocate_exact_size(Vec2::splat(22.0), Sense::click());
                            ui.painter().rect_filled(
                                rect,
                                3.0,
                                Color32::from_rgba_unmultiplied(
                                    s.color[0], s.color[1], s.color[2], s.alpha,
                                ),
                            );
                            response.widget_info(|| {
                                egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &s.name)
                            });
                            if response.clicked() {
                                editor.gradient.colors[editor.selected] = s.color;
                                editor.gradient.alpha[editor.selected] = s.alpha;
                            }
                            response.clone().help_text(&s.name);
                            crate::dropdown::context_popup(
                                ui,
                                egui::Popup::context_menu(&response),
                            )
                            .show(|ui| {
                                ui.label(&s.name);
                                if ui.button("Delete swatch").clicked() {
                                    remove = Some(i);
                                    ui.close();
                                }
                            });
                        });
                    }
                });
                if let Some(i) = remove {
                    library.data.swatches.remove(i);
                    library.save();
                }
                if !library.data.palettes.is_empty() {
                    egui::CollapsingHeader::new("Manage saved palettes").show_unindented(
                        ui,
                        |ui| {
                            let mut remove = None;
                            for (i, p) in library.data.palettes.iter().enumerate() {
                                ui.push_id(i, |ui| {
                                    ui.horizontal(|ui| {
                                        let width = (ui.available_width()
                                            - 24.0
                                            - ui.spacing().item_spacing.x)
                                            .max(40.0);
                                        let (rect, _) = ui.allocate_exact_size(
                                            Vec2::new(width, 22.0),
                                            Sense::hover(),
                                        );
                                        if ui
                                            .put(
                                                rect,
                                                egui::Button::new(&p.name).small().truncate(),
                                            )
                                            .help_text(&p.name)
                                            .clicked()
                                        {
                                            editor.gradient = p.gradient;
                                            changed = true;
                                        }
                                        if ui
                                            .small_button("×")
                                            .help_text("Delete saved palette")
                                            .clicked()
                                        {
                                            remove = Some(i);
                                        }
                                    });
                                });
                            }
                            if let Some(i) = remove {
                                library.data.palettes.remove(i);
                                library.save();
                            }
                        },
                    );
                }
                if let Some(notice) = &library.notice {
                    ui.small(notice);
                }
            });
        if changed || before != editor.gradient {
            *self = Self::Custom(editor.gradient);
            editor.source = *self;
        }
        ui.data_mut(|d| d.insert_temp(id, editor));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn custom_gradients_are_self_contained_and_keep_flux_cycles() {
        let theme = AppTheme::default();
        for palette in Palette::FACTORY {
            let saved = Palette::Custom(palette.gradient(&theme));
            let restored: Palette =
                serde_json::from_slice(&serde_json::to_vec(&saved).unwrap()).unwrap();
            for i in 0..100 {
                let t = i as f32 / 100.0;
                assert_eq!(
                    saved.gradient(&theme).color(t),
                    restored.gradient(&theme).color(t)
                );
                if palette == Palette::Spectrum {
                    assert_eq!(palette.cycle(t, &theme), saved.cycle(t, &theme));
                }
            }
        }
        let mut bad =
            serde_json::to_value(Palette::Custom(Palette::Ocean.gradient(&theme))).unwrap();
        bad["Custom"]["count"] = 9.into();
        assert!(serde_json::from_value::<Palette>(bad).is_err());
    }
    #[test]
    fn saved_palettes_and_swatches_survive_reload_and_bad_files_are_retained() {
        let dir = std::env::temp_dir().join(format!(
            "symphos-colors-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = dir.join("colors.json");
        let mut library = Library::load(path.clone());
        library.data.palettes.push(NamedPalette {
            name: "My set".into(),
            gradient: Palette::Aurora.gradient(&AppTheme::default()),
        });
        library.data.swatches.push(Swatch {
            name: "Accent".into(),
            color: [20, 180, 210],
            alpha: 255,
        });
        library.save();
        let restored = Library::load(path.clone());
        assert_eq!(
            restored.data.palettes[0].gradient,
            library.data.palettes[0].gradient
        );
        assert_eq!(restored.data.swatches[0].color, [20, 180, 210]);
        std::fs::write(&path, b"bad").unwrap();
        let rejected = Library::load(path.clone());
        assert!(rejected.notice.is_some());
        assert_eq!(
            std::fs::read(path.with_extension("rejected.json")).unwrap(),
            b"bad"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn library_rejects_duplicate_names_and_unbounded_data() {
        let p = NamedPalette {
            name: "Mine".into(),
            gradient: Palette::Ice.gradient(&AppTheme::default()),
        };
        let mut data = LibraryData {
            version: 1,
            palettes: vec![p.clone()],
            swatches: vec![],
        };
        assert!(Library::decode(&serde_json::to_vec(&data).unwrap()).is_some());
        data.version = 2;
        assert!(Library::decode(&serde_json::to_vec(&data).unwrap()).is_none());
        data.version = 1;
        let mut duplicate = p.clone();
        duplicate.name = "MINE".into();
        data.palettes.push(duplicate);
        assert!(Library::decode(&serde_json::to_vec(&data).unwrap()).is_none());
        data.palettes = vec![p; MAX_SAVED + 1];
        assert!(Library::decode(&serde_json::to_vec(&data).unwrap()).is_none());
        data.palettes.clear();
        data.swatches.push(Swatch {
            name: " ".into(),
            color: [0; 3],
            alpha: 255,
        });
        assert!(Library::decode(&serde_json::to_vec(&data).unwrap()).is_none());
    }
    #[test]
    fn color_editor_saves_a_named_palette_and_swatch_from_ui() {
        let dir = std::env::temp_dir().join(format!("symphos-color-ui-{}", std::process::id()));
        let path = dir.join("colors.json");
        let ctx = egui::Context::default();
        for theme in [egui::Theme::Dark, egui::Theme::Light] {
            ctx.style_mut_of(theme, |s| s.animation_time = 0.0);
        }
        let shared = Arc::new(Mutex::new(Library {
            path: path.clone(),
            data: LibraryData::default(),
            notice: None,
        }));
        ctx.data_mut(|d| d.insert_temp(egui::Id::new("color-library"), shared.clone()));
        let mut palette = Palette::Spectrum;
        let render = |palette: &mut Palette, events| {
            let mut id = egui::Id::NULL;
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        egui::Pos2::ZERO,
                        Vec2::new(320.0, 1800.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| {
                    crate::workspace_ui::sidebar_style(ui, &AppTheme::default());
                    id = ui.id().with("gradient-editor");
                    palette.controls(ui);
                },
            );
            output.textures_delta.clear();
            (output, id)
        };
        let locate = |output: &egui::FullOutput, label: &str| {
            output
                .shapes
                .iter()
                .find_map(|s| match &s.shape {
                    egui::Shape::Text(t) if t.galley.job.text == label => {
                        Some(t.pos + t.galley.size() * 0.5)
                    }
                    _ => None,
                })
                .unwrap_or_else(|| panic!("missing {label}"))
        };
        let pointer = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        let (output, id) = render(&mut palette, vec![]);
        render(&mut palette, vec![]);
        let pos = locate(&output, "Edit gradient & swatches");
        for pressed in [true, false] {
            render(
                &mut palette,
                vec![egui::Event::PointerMoved(pos), pointer(pos, pressed)],
            );
        }
        let mut editor = ctx.data(|d| d.get_temp::<Editor>(id)).unwrap();
        editor.name = "Stage colors".into();
        editor.swatch_name = "Warm white".into();
        editor.gradient.colors[0] = [240, 238, 220];
        ctx.data_mut(|d| d.insert_temp(id, editor));
        let (output, _) = render(&mut palette, vec![]);
        let pos = locate(&output, "Save palette");
        // The button and group label share text; choose the last (button) occurrence.
        let pos = output
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.job.text == "Save palette" => {
                    Some(t.pos + t.galley.size() * 0.5)
                }
                _ => None,
            })
            .next_back()
            .unwrap_or(pos);
        for pressed in [true, false] {
            render(
                &mut palette,
                vec![egui::Event::PointerMoved(pos), pointer(pos, pressed)],
            );
        }
        assert_eq!(shared.lock().unwrap().data.palettes[0].name, "Stage colors");
        let (output, _) = render(&mut palette, vec![]);
        let pos = locate(&output, "Save selected color");
        for pressed in [true, false] {
            render(
                &mut palette,
                vec![egui::Event::PointerMoved(pos), pointer(pos, pressed)],
            );
        }
        let restored = Library::load(path);
        assert_eq!(restored.data.swatches[0].color, [240, 238, 220]);
        assert_eq!(restored.data.swatches[0].name, "Warm white");
        assert_eq!(palette, Palette::Custom(restored.data.palettes[0].gradient));
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[cfg(test)]
mod opacity_tests {
    use super::*;
    #[test]
    fn gradient_opacity_reaches_cpu_gpu_resampling_and_saved_palettes() {
        let mut gradient = Gradient::from_hex(&[0xff0000, 0x0000ff]);
        gradient.alpha[0] = 0;
        gradient.alpha[1] = 128;
        assert_eq!(gradient.color(0.0).a(), 0);
        assert_eq!(gradient.color(1.0).a(), 128);
        assert_eq!(gradient.gpu()[0][3], 0.0);
        assert_eq!(gradient.gpu()[1][3], 128.0 / 255.0);
        let resized = gradient.resample(4);
        assert_eq!((resized.alpha[0], resized.alpha[3]), (0, 128));
        assert_eq!(
            resized.colors[0],
            [255, 0, 0],
            "transparent stops retain their RGB color"
        );
        let midpoint = gradient.sample_rgba(0.5);
        assert_eq!(midpoint, [127, 0, 127, 64]);
        let mut saved = serde_json::to_value(gradient).unwrap();
        assert_eq!(
            serde_json::from_value::<Gradient>(saved.clone()).unwrap(),
            gradient
        );
        saved.as_object_mut().unwrap().remove("alpha");
        assert_eq!(
            serde_json::from_value::<Gradient>(saved).unwrap().alpha,
            [255; 8]
        );
        let swatch: Swatch = serde_json::from_str(r#"{"name":"Old","color":[1,2,3]}"#).unwrap();
        assert_eq!(swatch.alpha, 255);
    }
}
