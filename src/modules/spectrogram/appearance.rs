use super::*;
use crate::modules::retro::{Appearance as Texture, Finish, Look};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(super) enum Coloring {
    #[default]
    Energy,
    Frequency,
    Time,
}
impl Coloring {
    fn label(self) -> &'static str {
        match self {
            Self::Energy => "Energy gradient",
            Self::Frequency => "Across frequencies",
            Self::Time => "Across time",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(super) struct Appearance {
    #[serde(flatten)]
    pub texture: Texture,
    pub coloring: Coloring,
    pub color_steps: usize,
    pub fade_floor: f32,
}
impl Default for Appearance {
    fn default() -> Self {
        Self {
            texture: Texture::default(),
            coloring: Coloring::Energy,
            color_steps: 0,
            fade_floor: 0.0,
        }
    }
}
impl Spectrogram {
    pub(super) fn apply_look(&mut self, look: Look) {
        let (texture, palette) = look.style();
        self.appearance = Appearance {
            texture,
            fade_floor: if look == Look::Studio { 0.0 } else { 0.18 },
            color_steps: if matches!(look, Look::PixelFire | Look::Classic) {
                12
            } else {
                0
            },
            ..Default::default()
        };
        self.palette = palette;
        self.contrast = 1.0;
        self.smooth = look == Look::Studio || look == Look::Neon;
        self.grid = false;
        self.labels = look == Look::Studio;
    }
}
impl Appearance {
    pub fn controls(&mut self, ui: &mut egui::Ui) {
        self.texture.controls(ui, false);
        super::super::property_row(ui, "Color direction", |ui| {
            super::super::properties_combo("spectrogram-coloring", ui.available_width(), 3).selected_text(self.coloring.label()).show_ui(ui, |ui| {
                for color in [Coloring::Energy, Coloring::Frequency, Coloring::Time] { ui.selectable_value(&mut self.coloring, color, color.label()); }
            }).response.help_text("Energy colors each cell by measured level. Frequency and time spread the gradient across that axis, with signal level still controlling brightness.");
        });
        ui.add(crate::parameter::Parameter::new(&mut self.color_steps, 0..=32, 0).key("appearance.color_steps").text("Color steps"))
            .help_text("Zero or one keeps a continuous gradient. Two or more quantizes color into distinct retro bands; measurements stay continuous.");
        ui.add(crate::parameter::Parameter::new(&mut self.fade_floor, 0.0..=0.5, 0.0).key("appearance.fade_floor").bounds(0.0..=1.0).text("Fade floor"))
            .help_text("Fade weak cells toward the canvas below this normalized display level. Removes a colored noise floor without changing the dB scale or captured data.");
    }
    pub fn color(
        &self,
        coordinates: [f32; 3],
        palette: Palette,
        contrast: f32,
        theme: &AppTheme,
        background: Color32,
    ) -> Color32 {
        let [level, frequency, time] = coordinates;
        let mut t = self.texture.coordinate(match self.coloring {
            Coloring::Energy => level,
            Coloring::Frequency => frequency,
            Coloring::Time => time,
        });
        let steps = self.color_steps.min(32);
        if steps >= 2 {
            t = (t * (steps - 1) as f32).round() / (steps - 1) as f32;
        }
        let color = self
            .texture
            .tint(palette.color_with_contrast(t, contrast, theme));
        let color = if self.texture.finish == Finish::Smooth && self.texture.glow > 0.0 {
            mix(
                color,
                Color32::WHITE,
                self.texture.glow.clamp(0.0, 1.0) * level * 0.2,
            )
        } else {
            color
        };
        let fade = if self.fade_floor > 0.0 {
            (level / self.fade_floor.clamp(0.001, 1.0)).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let intensity = if self.coloring == Coloring::Energy {
            fade
        } else {
            fade * level
        };
        if intensity >= 1.0 {
            color
        } else {
            mix(background, color, intensity)
        }
    }
    /// Four subpixels per cell provide gaps and round dots while keeping the
    /// largest texture below 1.6 million pixels. Generated only on cache changes.
    pub fn texture(&self, image: egui::ColorImage, background: Color32) -> egui::ColorImage {
        if self.texture.finish == Finish::Smooth {
            return image;
        }
        let [width, height] = image.size;
        let scale = 4;
        let output_width = width * scale;
        let mut pixels = vec![background; output_width * height * scale];
        let edge = (1.0 - self.texture.cell_gap.clamp(0.0, 0.85)) * 0.5;
        let mut mask = [0.0; 16];
        for y in 0..scale {
            for x in 0..scale {
                let dx = (x as f32 + 0.5) / scale as f32 - 0.5;
                let dy = (y as f32 + 0.5) / scale as f32 - 0.5;
                let distance = if self.texture.finish == Finish::Dots {
                    dx.hypot(dy)
                } else {
                    dx.abs().max(dy.abs())
                };
                let coverage = ((edge - distance) * scale as f32 + 0.5).clamp(0.0, 1.0);
                let halo = (1.0 - (distance - edge).max(0.0) * 3.0).max(0.0)
                    * self.texture.glow.clamp(0.0, 1.0)
                    * 0.2;
                mask[y * scale + x] = coverage.max(halo);
            }
        }
        for y in 0..height {
            for x in 0..width {
                let color = image.pixels[y * width + x];
                for sy in 0..scale {
                    for sx in 0..scale {
                        pixels[(y * scale + sy) * output_width + x * scale + sx] =
                            mix(background, color, mask[sy * scale + sx]);
                    }
                }
            }
        }
        egui::ColorImage::new([output_width, height * scale], pixels)
    }
}
