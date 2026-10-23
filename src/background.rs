//! Scene-owned canvas background, shared by workspace panes and recording output.
use crate::{modules::property_row, parameter::Parameter};
use egui::{self, Color32};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    #[default]
    Black,
    Color,
    Linear,
    Radial,
}
impl Mode {
    fn label(self) -> &'static str {
        match self {
            Self::Black => "Black",
            Self::Color => "Solid color",
            Self::Linear => "Linear gradient",
            Self::Radial => "Radial gradient",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Background {
    pub mode: Mode,
    pub start: [u8; 3],
    pub end: [u8; 3],
    pub alpha: [u8; 2],
    pub angle: f32,
}
impl Default for Background {
    fn default() -> Self {
        Self {
            mode: Mode::Black,
            start: [24, 32, 64],
            end: [3, 6, 18],
            alpha: [255; 2],
            angle: 90.0,
        }
    }
}
#[derive(Clone)]
struct Cached {
    settings: Background,
    texture: egui::TextureHandle,
}
pub const CONTEXT: &str = "scene-canvas-background";
/// The workspace already painted the full pane, including its header and gutters.
/// Recording uses its own egui context and paints its complete output normally.
pub const PANE_PAINTED: &str = "scene-background-painted-by-pane";
pub fn pane_painted(ui: &egui::Ui) -> bool {
    ui.data(|d| {
        d.get_temp::<bool>(egui::Id::new(PANE_PAINTED))
            .unwrap_or(false)
    })
}
impl Background {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
    pub fn valid(&self) -> bool {
        self.angle.is_finite() && (0.0..=360.0).contains(&self.angle)
    }
    /// Normalized canvas coordinates: zero degrees runs left to right; 90 top to bottom.
    pub fn sample(&self, uv: [f32; 2]) -> Color32 {
        let color = |[r, g, b]: [u8; 3], alpha| Color32::from_rgba_unmultiplied(r, g, b, alpha);
        let t = match self.mode {
            Mode::Black => return Color32::BLACK,
            Mode::Color => return color(self.start, self.alpha[0]),
            Mode::Linear => {
                let a = self.angle.to_radians();
                let (y, x) = a.sin_cos();
                0.5 + ((uv[0] - 0.5) * x + (uv[1] - 0.5) * y) / (x.abs() + y.abs()).max(0.001)
            }
            Mode::Radial => ((uv[0] - 0.5).hypot(uv[1] - 0.5) * 2.0).min(1.0),
        };
        color(self.start, self.alpha[0])
            .lerp_to_gamma(color(self.end, self.alpha[1]), t.clamp(0.0, 1.0))
    }
    pub fn paint(&self, ui: &egui::Ui, rect: egui::Rect) {
        self.paint_rounded(ui, rect, 0);
    }
    pub fn paint_rounded(&self, ui: &egui::Ui, rect: egui::Rect, radius: u8) {
        if self.alpha != [255; 2] {
            ui.painter().rect_filled(rect, radius, Color32::BLACK);
        }
        if matches!(self.mode, Mode::Black | Mode::Color) {
            ui.painter()
                .rect_filled(rect, radius, self.sample([0.0; 2]));
            return;
        }
        // One cached texture shared across panes; no per-frame rasterization or upload.
        let id = egui::Id::new("scene-background-texture");
        let cached = ui.data_mut(|d| d.get_temp::<Cached>(id));
        let cached = match cached {
            Some(cached) if cached.settings == *self => cached,
            previous => {
                let size = 128;
                let pixels = (0..size * size)
                    .map(|i| {
                        self.sample([
                            (i % size) as f32 / (size - 1) as f32,
                            (i / size) as f32 / (size - 1) as f32,
                        ])
                    })
                    .collect();
                let image = egui::ColorImage::new([size, size], pixels);
                let texture = if let Some(mut previous) = previous {
                    previous.texture.set(image, egui::TextureOptions::LINEAR);
                    previous.texture
                } else {
                    ui.ctx()
                        .load_texture("Scene background", image, egui::TextureOptions::LINEAR)
                };
                let cached = Cached {
                    settings: *self,
                    texture,
                };
                ui.data_mut(|d| d.insert_temp(id, cached.clone()));
                cached
            }
        };
        ui.painter().add(
            egui::epaint::RectShape::filled(rect, radius, Color32::WHITE).with_texture(
                cached.texture.id(),
                egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
            ),
        );
    }
    pub fn controls(&mut self, ui: &mut egui::Ui) {
        // This is already scoped to Appearance, before visualizer-specific groups.
        crate::modules::settings_group(ui, "Scene background", |ui| {
            property_row(ui, "Fill", |ui| {
                crate::dropdown::ComboBox::from_id_salt("scene-background-mode")
                    .width(ui.available_width())
                    .selected_text(self.mode.label())
                    .show_ui(ui, |ui| {
                        for mode in [Mode::Black, Mode::Color, Mode::Linear, Mode::Radial] {
                            ui.selectable_value(&mut self.mode, mode, mode.label());
                        }
                    });
            });
            if self.mode != Mode::Black {
                property_row(
                    ui,
                    if self.mode == Mode::Color {
                        "Color"
                    } else {
                        "Start color"
                    },
                    |ui| {
                        crate::color_picker::color_edit_button_srgba(
                            ui,
                            &mut self.start,
                            &mut self.alpha[0],
                        );
                    },
                );
            }
            if matches!(self.mode, Mode::Linear | Mode::Radial) {
                property_row(ui, "End color", |ui| {
                    crate::color_picker::color_edit_button_srgba(
                        ui,
                        &mut self.end,
                        &mut self.alpha[1],
                    );
                });
            }
            if self.mode == Mode::Linear {
                ui.add(Parameter::new(&mut self.angle, 0.0..=360.0, 90.0).text("Angle °"));
            }
            ui.add(
                egui::Label::new(
                    egui::RichText::new("Shared by every sketch in this scene.")
                        .weak()
                        .size(11.0),
                )
                .wrap(),
            );
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn backgrounds_have_stable_opaque_colors_and_gradient_directions() {
        let mut b = Background::default();
        assert_eq!(b.sample([0.2, 0.8]), Color32::BLACK);
        b.mode = Mode::Color;
        b.start = [200, 30, 60];
        b.end = [20, 80, 180];
        assert_eq!(b.sample([0.2, 0.8]), Color32::from_rgb(200, 30, 60));
        b.mode = Mode::Linear;
        b.angle = 0.0;
        assert_eq!(b.sample([0.0, 0.5]), Color32::from_rgb(200, 30, 60));
        assert_eq!(b.sample([1.0, 0.5]), Color32::from_rgb(20, 80, 180));
        b.angle = 90.0;
        assert_eq!(b.sample([0.5, 0.0]), Color32::from_rgb(200, 30, 60));
        assert_eq!(b.sample([0.5, 1.0]), Color32::from_rgb(20, 80, 180));
        b.mode = Mode::Radial;
        assert_eq!(b.sample([0.5, 0.5]), Color32::from_rgb(200, 30, 60));
        assert_eq!(b.sample([1.0, 0.5]), Color32::from_rgb(20, 80, 180));
        assert!(
            !Background {
                angle: f32::NAN,
                ..b
            }
            .valid()
        );
        assert!(!Background { angle: 361.0, ..b }.valid());
    }
    #[test]
    fn background_opacity_roundtrips_and_older_backgrounds_stay_opaque() {
        let background = Background {
            mode: Mode::Color,
            start: [200, 100, 50],
            alpha: [128, 255],
            ..Default::default()
        };
        assert_eq!(background.sample([0.5, 0.5]).a(), 128);
        let mut saved = serde_json::to_value(background).unwrap();
        assert_eq!(
            serde_json::from_value::<Background>(saved.clone()).unwrap(),
            background
        );
        saved.as_object_mut().unwrap().remove("alpha");
        assert_eq!(
            serde_json::from_value::<Background>(saved).unwrap().alpha,
            [255; 2]
        );
    }

    #[test]
    fn gradient_upload_is_shared_by_panes_and_only_changes_with_settings() {
        let ctx = egui::Context::default();
        let render = |b: Background| {
            ctx.run_ui(egui::RawInput::default(), |ui| {
                for i in 0..3 {
                    b.paint(
                        ui,
                        egui::Rect::from_min_size(
                            egui::pos2(i as f32 * 100.0, 0.0),
                            egui::vec2(100.0, 100.0),
                        ),
                    );
                }
            })
        };
        let b = Background {
            mode: Mode::Linear,
            ..Default::default()
        };
        let mut first = render(b);
        assert!(!first.textures_delta.set.is_empty());
        first.textures_delta.clear();
        let mut unchanged = render(b);
        assert!(unchanged.textures_delta.set.is_empty());
        unchanged.textures_delta.clear();
        let mut changed = render(Background { angle: 10.0, ..b });
        assert_eq!(changed.textures_delta.set.len(), 1);
        changed.textures_delta.clear();
    }
}
