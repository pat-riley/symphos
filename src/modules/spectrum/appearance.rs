//! Spectrum-only styling. The source bands, reference and cursor remain measured data.
use super::*;
use crate::modules::{properties_combo, property_row};
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(super) enum Finish {
    #[default]
    Solid,
    Cells,
    Dots,
}
impl Finish {
    fn label(self) -> &'static str {
        match self {
            Self::Solid => "Solid bars",
            Self::Cells => "LED cells",
            Self::Dots => "Dot matrix",
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(super) enum Coloring {
    #[default]
    Level,
    Height,
    Frequency,
    Uniform,
}
impl Coloring {
    fn label(self) -> &'static str {
        match self {
            Self::Level => "Bar level",
            Self::Height => "Vertical gradient",
            Self::Frequency => "Across frequencies",
            Self::Uniform => "Single color",
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(super) enum PeakStyle {
    #[default]
    Line,
    Blocks,
    Dots,
}
impl PeakStyle {
    fn label(self) -> &'static str {
        match self {
            Self::Line => "Caps",
            Self::Blocks => "Blocks",
            Self::Dots => "Dots",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(super) struct Appearance {
    pub finish: Finish,
    pub coloring: Coloring,
    pub reverse: bool,
    pub cell_size: f32,
    pub cell_gap: f32,
    pub brightness: f32,
    pub glow: f32,
    pub unlit: f32,
    pub black_background: bool,
    pub peak_style: PeakStyle,
    pub white_peaks: bool,
}
impl Default for Appearance {
    fn default() -> Self {
        Self {
            finish: Finish::Solid,
            coloring: Coloring::Level,
            reverse: false,
            cell_size: 4.0,
            cell_gap: 0.22,
            brightness: 1.0,
            glow: 0.0,
            unlit: 0.0,
            black_background: false,
            peak_style: PeakStyle::Line,
            white_peaks: true,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
enum Look {
    Studio,
    PixelFire,
    Classic,
    Neon,
    Phosphor,
}
impl Look {
    const ALL: [Self; 5] = [
        Self::Studio,
        Self::PixelFire,
        Self::Classic,
        Self::Neon,
        Self::Phosphor,
    ];
    fn label(self) -> &'static str {
        match self {
            Self::Studio => "Studio",
            Self::PixelFire => "Pixel fire",
            Self::Classic => "Classic LEDs",
            Self::Neon => "Neon matrix",
            Self::Phosphor => "Phosphor dots",
        }
    }
}
impl Spectrum {
    fn apply_look(&mut self, look: Look) {
        self.appearance = Appearance::default();
        self.style = TraceStyle::Bars;
        self.bar_gap = 15.0;
        self.contrast = 1.0;
        self.palette = Palette::Theme;
        self.grid = look == Look::Studio;
        self.labels = look == Look::Studio;
        self.peak_hold = look != Look::Studio;
        if look != Look::Studio {
            self.appearance.finish = Finish::Cells;
            self.appearance.coloring = Coloring::Height;
            self.appearance.black_background = true;
            self.hold_seconds = 0.15;
            self.peak_falloff = 28.0;
            self.infinite_hold = false;
            self.bar_gap = 8.0;
            self.appearance.glow = 0.08;
        }
        match look {
            Look::Studio => {}
            Look::PixelFire => {
                self.palette = Palette::PixelFire;
                self.appearance.cell_size = 2.8;
                self.appearance.cell_gap = 0.24;
                self.appearance.peak_style = PeakStyle::Blocks;
            }
            Look::Classic => {
                self.palette = Palette::ClassicLed;
                self.appearance.cell_size = 5.0;
                self.appearance.cell_gap = 0.28;
                self.appearance.unlit = 0.025;
            }
            Look::Neon => {
                self.palette = Palette::NeonNights;
                self.appearance.coloring = Coloring::Frequency;
                self.appearance.cell_size = 6.0;
                self.appearance.glow = 0.35;
                self.appearance.peak_style = PeakStyle::Dots;
            }
            Look::Phosphor => {
                self.palette = Palette::Phosphor;
                self.appearance.finish = Finish::Dots;
                self.appearance.cell_size = 5.0;
                self.appearance.glow = 0.18;
                self.appearance.unlit = 0.035;
                self.appearance.white_peaks = false;
                self.appearance.peak_style = PeakStyle::Dots;
            }
        }
        self.reset_peaks();
    }
    pub(super) fn look_controls(&mut self, ui: &mut egui::Ui) {
        property_row(ui, "Look", |ui| {
            properties_combo("spectrum-look",ui.available_width(),Look::ALL.len())
                .selected_text("Choose a starting look…").show_ui(ui,|ui| {
                    for look in Look::ALL { if ui.selectable_label(false,look.label()).clicked() {self.apply_look(look);ui.close();} }
                }).response.help_text("Spectrum looks\nStudio, Pixel fire, Classic LEDs, Neon matrix and Phosphor dots are appearance starting points. They set drawing style, colors, guides and peak timing. Frequency range, measured levels, gain, smoothing, reference and macro assignments are retained.");
        });
    }
}
impl Appearance {
    pub(super) fn controls(&mut self, ui: &mut egui::Ui, bars: bool) {
        if bars {
            property_row(ui, "Bar texture", |ui| {
                properties_combo("spectrum-finish", ui.available_width(), 3)
                    .selected_text(self.finish.label())
                    .show_ui(ui, |ui| {
                        for finish in [Finish::Solid, Finish::Cells, Finish::Dots] {
                            ui.selectable_value(&mut self.finish, finish, finish.label());
                        }
                    });
            });
            if self.finish != Finish::Solid {
                ui.add(crate::parameter::Parameter::new(&mut self.cell_size,1.5..=14.0,4.0).key("appearance.cell_size").bounds(1.0..=32.0).text("Cell size px"))
                    .help_text("Size of the square cells or dots in logical screen pixels. Bars can contain several cells across their width. This changes the raster texture, not the number of measured frequency bands. Dense views automatically limit cell count to keep rendering bounded.");
                ui.add(crate::parameter::Parameter::new(&mut self.cell_gap,0.0..=0.65,0.22).key("appearance.cell_gap").bounds(0.0..=0.85).text("Cell gap"))
                    .help_text("Dark spacing inside the pixel grid. Larger gaps give separated LEDs; zero joins neighboring cells.");
                ui.add(crate::parameter::Parameter::new(&mut self.unlit,0.0..=0.15,0.0).key("appearance.unlit").bounds(0.0..=0.3).text("Unlit cells"))
                    .help_text("Faint background cells above the live bars, like an inactive LED matrix. Zero hides them entirely.");
            }
        }
        property_row(ui, "Color direction", |ui| {
            properties_combo("spectrum-coloring",ui.available_width(),4).selected_text(self.coloring.label()).show_ui(ui,|ui| {for mode in [Coloring::Level,Coloring::Height,Coloring::Frequency,Coloring::Uniform] {ui.selectable_value(&mut self.coloring,mode,mode.label());}}).response.help_text("Bar level gives each band its level color. Vertical gradient runs bottom to top through every bar. Across frequencies runs bass to treble. Single color uses the palette’s bright endpoint.");
        });
        property_row(ui, "Reverse colors", |ui| {
            ui.checkbox(&mut self.reverse, "").help_text("Reverse the selected gradient along its color direction. Bar heights and signal processing are unchanged.");
        });
        ui.add(crate::parameter::Parameter::new(&mut self.brightness,0.1..=2.0,1.0).key("appearance.brightness").bounds(0.0..=4.0).text("Brightness"))
            .help_text("Visual brightness only. Values above one brighten toward white; measured levels and bar heights stay the same.");
        ui.add(crate::parameter::Parameter::new(&mut self.glow,0.0..=1.0,0.0).key("appearance.glow").text("Glow"))
            .help_text("A restrained halo around bars and traces. Pixel gaps remain visible; glow does not affect analysis or peak measurements.");
        if !crate::modules::flat_inspector(ui) {
            property_row(ui, "Black background", |ui| {
                ui.checkbox(&mut self.black_background, "").help_text(
                    "Use a black spectrum canvas independently of the application GUI theme.",
                );
            });
        }
    }
    pub(super) fn peak_controls(&mut self, ui: &mut egui::Ui, bars: bool) {
        if bars {
            property_row(ui, "Peak style", |ui| {
                properties_combo("spectrum-peak-style", ui.available_width(), 3)
                    .selected_text(self.peak_style.label())
                    .show_ui(ui, |ui| {
                        for style in [PeakStyle::Line, PeakStyle::Blocks, PeakStyle::Dots] {
                            ui.selectable_value(&mut self.peak_style, style, style.label());
                        }
                    });
            });
        }
        property_row(ui, "White peaks", |ui| {
            ui.checkbox(&mut self.white_peaks,"").help_text("Bright neutral markers float above the bars, in the classic analyzer style. Disable to use the selected gradient color.");
        });
    }
    pub(super) fn color(
        &self,
        palette: Palette,
        level: f32,
        height: f32,
        frequency: f32,
        contrast: f32,
        theme: &AppTheme,
    ) -> egui::Color32 {
        let mut t = match self.coloring {
            Coloring::Level => level,
            Coloring::Height => height,
            Coloring::Frequency => frequency,
            Coloring::Uniform => 1.0,
        };
        if self.reverse {
            t = 1.0 - t;
        }
        let color = if self.coloring == Coloring::Level {
            if palette == Palette::Theme {
                mix(theme.accent_alt, theme.accent, t)
            } else {
                palette.color_with_contrast(t, contrast, theme)
            }
        } else {
            palette.gradient(theme).color(t)
        };
        let brightness = self.brightness.clamp(0.0, 4.0);
        if brightness <= 1.0 {
            color.gamma_multiply(brightness)
        } else {
            mix(
                color,
                egui::Color32::WHITE,
                ((brightness - 1.0) * 0.45).min(1.0),
            )
        }
    }
    pub(super) fn draw_peak(
        &self,
        painter: &egui::Painter,
        x: f32,
        y: f32,
        width: f32,
        color: egui::Color32,
    ) {
        match self.peak_style {
            PeakStyle::Line => {
                painter.line_segment(
                    [Pos2::new(x - width * 0.5, y), Pos2::new(x + width * 0.5, y)],
                    Stroke::new(1.0, color),
                );
            }
            PeakStyle::Blocks => {
                let h = self.cell_size.clamp(1.0, 5.0);
                painter.rect_filled(
                    Rect::from_center_size(Pos2::new(x, y), Vec2::new(width, h)),
                    0.0,
                    color,
                );
            }
            PeakStyle::Dots => {
                painter.circle_filled(Pos2::new(x, y), self.cell_size.clamp(1.0, 5.0) * 0.4, color);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn looks_keep_analysis_and_references_and_old_projects_keep_original_style() {
        let mut spectrum = Spectrum::default();
        spectrum.data.settings.gain = 12.0;
        spectrum.data.settings.min_hz = 40.0;
        spectrum.data.levels.fill(-30.0);
        let processing = spectrum.data.settings.clone();
        for look in Look::ALL {
            spectrum.apply_look(look);
            assert_eq!(spectrum.data.settings, processing);
            assert_eq!(spectrum.data.levels, [-30.0; BANDS]);
            let saved = spectrum.settings_snapshot();
            let decoded: super::super::SpectrumSettings =
                serde_json::from_slice(&serde_json::to_vec(&saved).unwrap()).unwrap();
            assert_eq!(saved, decoded);
            assert!(crate::workspace::valid_settings(&saved));
        }
        let mut legacy = serde_json::to_value(Spectrum::default().settings_snapshot()).unwrap();
        legacy.as_object_mut().unwrap().remove("appearance");
        let decoded: super::super::SpectrumSettings = serde_json::from_value(legacy).unwrap();
        assert_eq!(decoded.appearance, Appearance::default());
        assert_eq!(decoded.style, TraceStyle::Bars);
        let mut invalid = decoded;
        invalid.appearance.cell_size = 0.0;
        assert!(!crate::workspace::valid_settings(&invalid));
    }
    #[test]
    fn every_look_draws_bounded_colored_geometry_without_changing_levels_or_measurements() {
        let context = egui::Context::default();
        let now = Instant::now();
        let frame = AnalysisFrame {
            sequence: 1,
            sample_rate: 48000,
            fft_size: 512,
            bins: vec![
                crate::analysis::FrequencyBin {
                    magnitude: 0.1,
                    ..Default::default()
                };
                257
            ]
            .into(),
            ..Default::default()
        };
        let mut spectrum = Spectrum::default();
        spectrum.data.settings.smoothing = 0.0;
        for look in Look::ALL {
            spectrum.apply_look(look);
            for size in [Vec2::new(180.0, 90.0), Vec2::new(1500.0, 500.0)] {
                let mut output = context.run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
                        ..Default::default()
                    },
                    |ui| {
                        spectrum.draw(
                            ui,
                            Rect::from_min_size(Pos2::ZERO, size),
                            &frame,
                            &AppTheme::default(),
                            now,
                            ModuleRenderState {
                                live: true,
                                frozen: false,
                            },
                        );
                    },
                );
                assert!(
                    spectrum
                        .data
                        .levels
                        .iter()
                        .all(|v| (*v + 20.0).abs() < 0.01)
                );
                assert_eq!(
                    Spectrum::measurement_at(
                        &spectrum.data.settings,
                        &spectrum.data.levels,
                        0.5,
                        48000
                    )
                    .dbfs,
                    -20.0
                );
                let meshes =
                    context.tessellate(std::mem::take(&mut output.shapes), output.pixels_per_point);
                let mut vertices = 0;
                for mesh in meshes {
                    if let egui::epaint::Primitive::Mesh(mesh) = mesh.primitive {
                        vertices += mesh.vertices.len();
                        assert!(mesh.vertices.iter().all(|v| v.pos.is_finite()));
                    }
                }
                assert!(vertices < 180_000, "unbounded spectrum mesh: {vertices}");
                output.textures_delta.clear();
            }
        }
        let color = Appearance {
            coloring: Coloring::Height,
            ..Default::default()
        };
        let theme = AppTheme::default();
        assert_ne!(
            color.color(Palette::PixelFire, 0.75, 0.05, 0.5, 1.0, &theme),
            color.color(Palette::PixelFire, 0.75, 0.75, 0.5, 1.0, &theme)
        );
    }
}
