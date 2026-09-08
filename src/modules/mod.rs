mod camera_gizmo;
mod frequency;
mod spectrogram;
mod spectrum;
mod waterfall;
mod waveform;

use std::time::Instant;

use eframe::egui::{self, Color32, FontId, Pos2, Rect};

use crate::help::HoverHelp;
use crate::icons::Icon;
use crate::{analysis::AnalysisFrame, theme::AppTheme};
use spectrogram::Spectrogram;
use spectrum::Spectrum;
use waterfall::Waterfall;
use waveform::Waveform;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModuleKind {
    Waterfall,
    Spectrum,
    Waveform,
    Spectrogram,
}

impl ModuleKind {
    pub const ALL: [Self; 4] = [
        Self::Waterfall,
        Self::Spectrum,
        Self::Waveform,
        Self::Spectrogram,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Waterfall => "3D Waterfall",
            Self::Spectrum => "Frequency spectrum",
            Self::Waveform => "Waveform",
            Self::Spectrogram => "Spectrogram",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Waterfall => {
                "A 3D frequency history: X is frequency, Y is time, and Z is signal level. Drag to orbit; right-drag or Shift-drag to pan."
            }
            Self::Spectrum => {
                "Current signal level by frequency. Hover over a band to inspect its frequency and level."
            }
            Self::Waveform => {
                "Audio amplitude over a short time window. Use the bottom-bar stereo/mix icon to switch between separate channels and a combined signal."
            }
            Self::Spectrogram => {
                "Frequency over time, with color showing signal level. Newest audio appears on the right."
            }
        }
    }
}

pub struct ModulePane {
    pub kind: ModuleKind,
    waterfall: Waterfall,
    spectrum: Spectrum,
    waveform: Waveform,
    spectrogram: Spectrogram,
    active_sections: [usize; 4],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsSection {
    Camera,
    History,
    Geometry,
    Frequency,
    Response,
    Appearance,
    TimeWindow,
    Amplitude,
}

impl SettingsSection {
    pub fn title(self) -> &'static str {
        match self {
            Self::Camera => "Camera",
            Self::History => "Time & History",
            Self::Geometry => "Geometry",
            Self::Frequency => "Frequency Range",
            Self::Response => "Signal Response",
            Self::Appearance => "Appearance",
            Self::TimeWindow => "Time Window",
            Self::Amplitude => "Amplitude",
        }
    }
    pub fn icon(self) -> Icon {
        match self {
            Self::Camera => Icon::Camera,
            Self::History | Self::TimeWindow => Icon::Time,
            Self::Geometry => Icon::Geometry,
            Self::Frequency => Icon::Frequency,
            Self::Response => Icon::Response,
            Self::Appearance => Icon::Appearance,
            Self::Amplitude => Icon::Waveform,
        }
    }
}

impl ModulePane {
    pub fn new(kind: ModuleKind) -> Self {
        Self {
            kind,
            waterfall: Waterfall::default(),
            spectrum: Spectrum::default(),
            waveform: Waveform::default(),
            spectrogram: Spectrogram::default(),
            active_sections: [0; 4],
        }
    }

    pub fn reset(&mut self) {
        self.waterfall.clear();
        self.spectrum.clear();
        self.spectrogram.clear();
    }

    pub fn controls(&mut self, ui: &mut egui::Ui, frame: &AnalysisFrame) {
        let section = self.active_section();
        ui.push_id(self.kind.label(), |ui| {
            ui.data_mut(|data| data.insert_temp(ui.id().with("active-module-section"), section));
            match self.kind {
                ModuleKind::Waterfall => self.waterfall.controls(ui),
                ModuleKind::Spectrum => self.spectrum.controls(ui),
                ModuleKind::Waveform => self.waveform.controls(ui, frame),
                ModuleKind::Spectrogram => self.spectrogram.controls(ui),
            }
        });
    }

    pub fn sections(&self) -> &'static [SettingsSection] {
        use SettingsSection::*;
        match self.kind {
            ModuleKind::Waterfall => &[Camera, History, Geometry, Frequency, Response, Appearance],
            ModuleKind::Spectrum => &[Frequency, Response, Appearance],
            ModuleKind::Spectrogram => &[History, Frequency, Response, Appearance],
            ModuleKind::Waveform => &[TimeWindow, Amplitude],
        }
    }

    pub fn active_section(&self) -> SettingsSection {
        self.sections()[self.active_sections[self.kind as usize]]
    }

    pub fn select_section(&mut self, section: SettingsSection) {
        if let Some(index) = self
            .sections()
            .iter()
            .position(|candidate| *candidate == section)
        {
            self.active_sections[self.kind as usize] = index;
        }
    }

    pub fn set_channel_view(&mut self, stereo: bool, channel: crate::analysis::ChannelMode) {
        self.waveform.set_channel_view(stereo, channel);
    }

    pub fn draw(
        &mut self,
        ui: &mut egui::Ui,
        rect: Rect,
        frame: &AnalysisFrame,
        theme: &AppTheme,
        live: bool,
    ) {
        let now = Instant::now();
        ui.painter().rect_filled(rect, 4.0, theme.background);
        ui.interact(rect, ui.id().with("module-help"), egui::Sense::hover())
            .help_text(self.kind.description());
        match self.kind {
            ModuleKind::Waterfall => self.waterfall.draw(ui, rect, frame, theme, now, live),
            ModuleKind::Spectrum => self.spectrum.draw(ui, rect, frame, theme, now, live),
            ModuleKind::Waveform => self.waveform.draw(ui, rect, frame, theme, live),
            ModuleKind::Spectrogram => self.spectrogram.draw(ui, rect, frame, theme, now, live),
        }
        if !live {
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "Waiting for audio",
                FontId::proportional(14.0),
                theme.muted,
            );
        }
    }
}

pub(crate) fn settings_panel(ui: &mut egui::Ui, title: &str, body: impl FnOnce(&mut egui::Ui)) {
    let selected =
        ui.data(|data| data.get_temp::<SettingsSection>(ui.id().with("active-module-section")));
    if selected.is_some_and(|section| section.title() != title) {
        return;
    }
    ui.push_id(title, |ui| {
        ui.strong(title);
        ui.add_space(6.0);
        body(ui);
    });
}

fn label(painter: &egui::Painter, position: Pos2, text: impl ToString, color: Color32) {
    painter.text(
        position,
        egui::Align2::LEFT_TOP,
        text,
        FontId::monospace(10.0),
        color,
    );
}

fn mix(a: Color32, b: Color32, amount: f32) -> Color32 {
    let channel = |left: u8, right: u8| {
        (left as f32 + (right as f32 - left as f32) * amount.clamp(0.0, 1.0)) as u8
    };
    Color32::from_rgb(
        channel(a.r(), b.r()),
        channel(a.g(), b.g()),
        channel(a.b(), b.b()),
    )
}

fn frequency_label(hz: f32) -> String {
    if hz >= 1000.0 {
        format!("{:.1}k", hz / 1000.0)
    } else {
        format!("{hz:.0}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_tabs_show_one_section_and_preserve_per_module_and_pane_selection() {
        let context = egui::Context::default();
        let mut pane = ModulePane::new(ModuleKind::Waterfall);
        let render = |pane: &mut ModulePane| {
            let mut output = context.run_ui(egui::RawInput::default(), |ui| {
                pane.controls(ui, &AnalysisFrame::default());
            });
            output.textures_delta.clear();
            output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) => Some(text.galley.job.text.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        let labels = render(&mut pane);
        assert!(labels.iter().any(|label| label == "Camera"));
        assert!(!labels.iter().any(|label| label == "History s"));
        pane.select_section(SettingsSection::Geometry);
        let labels = render(&mut pane);
        assert!(labels.iter().any(|label| label == "Length X"));
        assert!(
            !labels
                .iter()
                .any(|label| label == "Camera" || label == "History s")
        );
        pane.kind = ModuleKind::Spectrum;
        assert_eq!(pane.active_section(), SettingsSection::Frequency);
        pane.select_section(SettingsSection::Appearance);
        pane.kind = ModuleKind::Waterfall;
        assert_eq!(pane.active_section(), SettingsSection::Geometry);
        pane.kind = ModuleKind::Spectrum;
        assert_eq!(pane.active_section(), SettingsSection::Appearance);
        let other = ModulePane::new(ModuleKind::Spectrum);
        assert_eq!(other.active_section(), SettingsSection::Frequency);
        pane.kind = ModuleKind::Waveform;
        assert_eq!(
            pane.sections(),
            &[SettingsSection::TimeWindow, SettingsSection::Amplitude]
        );
    }

    #[test]
    fn global_channel_view_reaches_every_waveform_pane() {
        let context = egui::Context::default();
        for kind in ModuleKind::ALL {
            let mut pane = ModulePane::new(kind);
            pane.set_channel_view(false, crate::analysis::ChannelMode::StereoMix);
            pane.kind = ModuleKind::Waveform;
            let mut output = context.run_ui(egui::RawInput::default(), |ui| {
                pane.draw(
                    ui,
                    Rect::from_min_size(Pos2::ZERO, egui::vec2(500.0, 300.0)),
                    &AnalysisFrame::default(),
                    &AppTheme::default(),
                    false,
                );
            });
            output.textures_delta.clear();
            assert!(output.shapes.iter().any(|shape| matches!(&shape.shape,
                egui::Shape::Text(text) if text.galley.job.text == "Stereo mix")));
            assert!(!output.shapes.iter().any(|shape| matches!(&shape.shape,
                egui::Shape::Text(text) if text.galley.job.text == "L" || text.galley.job.text == "R")));
        }
    }

    #[test]
    fn heatmap_spans_cool_to_warm_without_theme_tinting() {
        let theme = AppTheme::default();
        let palette = Palette::Heatmap;
        let quiet = palette.color(0.0, &theme);
        let middle = palette.color(0.5, &theme);
        let loud = palette.color(1.0, &theme);
        assert!(quiet.b() > quiet.r() && quiet.b() > quiet.g());
        assert!(middle.g() > middle.r() && middle.g() > middle.b());
        assert!(loud.r() > loud.g() && loud.r() > loud.b());
        assert_eq!(palette.color(-1.0, &theme), quiet);
        assert_eq!(palette.color(2.0, &theme), loud);
        let other_theme = AppTheme {
            background: Color32::WHITE,
            foreground: Color32::BLACK,
            ..theme.clone()
        };
        let mut colors = std::collections::HashSet::new();
        for step in 0..=255 {
            let level = step as f32 / 255.0;
            let color = palette.color(level, &theme);
            colors.insert(color.to_array());
            assert_eq!(color, palette.color(level, &other_theme));
            if step > 0 {
                let previous = palette.color((step - 1) as f32 / 255.0, &theme);
                for (a, b) in color.to_array().iter().zip(previous.to_array()) {
                    assert!(a.abs_diff(b) <= 6, "smooth gradient between color stops");
                }
            }
        }
        assert!(colors.len() > 250);
    }

    #[test]
    fn heatmap_contrast_preserves_endpoints_midpoint_and_other_palettes() {
        let theme = AppTheme::default();
        for contrast in [0.25, 1.0, 3.0] {
            for t in [0.0, 0.5, 1.0] {
                assert_eq!(
                    Palette::Heatmap.color_with_contrast(t, contrast, &theme),
                    Palette::Heatmap.color(t, &theme)
                );
            }
            for palette in [Palette::Theme, Palette::Ember, Palette::Ocean] {
                assert_eq!(
                    palette.color_with_contrast(0.3, contrast, &theme),
                    palette.color(0.3, &theme)
                );
            }
        }
        let cool = Palette::Heatmap.color_with_contrast(0.25, 3.0, &theme);
        let warm = Palette::Heatmap.color_with_contrast(0.75, 3.0, &theme);
        assert!(cool.b() > cool.g());
        assert!(warm.r() > warm.g());
        for step in 0..=100 {
            let t = step as f32 / 100.0;
            assert_eq!(
                Palette::Heatmap.color_with_contrast(t, 1.0, &theme),
                Palette::Heatmap.color(t, &theme)
            );
        }
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Palette {
    #[default]
    Theme,
    Ember,
    Ocean,
    Heatmap,
}

impl Palette {
    fn contrast_controls(self, ui: &mut egui::Ui, contrast: &mut f32) {
        if self == Self::Heatmap {
            ui.add(egui::Slider::new(contrast, 0.25..=3.0).text("Contrast"))
                .help_text("1 is neutral. Higher contrast separates quiet blues from loud reds; lower contrast brings colors toward the middle. Changes color only, not height, levels, or retained audio.");
        }
    }

    fn color_with_contrast(self, value: f32, contrast: f32, theme: &AppTheme) -> Color32 {
        let value = if self == Self::Heatmap {
            let t = value.clamp(0.0, 1.0);
            let a = t.powf(contrast);
            a / (a + (1.0 - t).powf(contrast))
        } else {
            value
        };
        self.color(value, theme)
    }

    fn controls(&mut self, ui: &mut egui::Ui) {
        egui::ComboBox::from_id_salt("palette")
            .selected_text(self.label())
            .show_ui(ui, |ui| {
                for value in [Self::Theme, Self::Ember, Self::Ocean, Self::Heatmap] {
                    ui.selectable_value(self, value, value.label()).help_text(
                        if value == Self::Heatmap {
                            "Full-spectrum heatmap: cool blue and cyan for quiet levels, green through yellow and orange to red for loud peaks. Floor and Ceiling set the level range."
                        } else {
                            "Choose the colors used to represent quiet and loud signal levels."
                        },
                    );
                }
            })
            .response
            .help_text("Choose a signal color palette: current theme, Ember, Ocean, or a full-spectrum Heatmap from cool quiet levels to warm loud peaks.");
    }

    fn label(self) -> &'static str {
        match self {
            Self::Theme => "Theme colors",
            Self::Ember => "Ember",
            Self::Ocean => "Ocean",
            Self::Heatmap => "Heatmap",
        }
    }

    fn color(self, value: f32, theme: &AppTheme) -> Color32 {
        let (low, high) = match self {
            Self::Heatmap => {
                // Map the same normalized level used for waterfall height to a
                // full cool-to-warm spectrum, independent of the desktop theme.
                const STOPS: [Color32; 7] = [
                    Color32::from_rgb(32, 40, 160),
                    Color32::from_rgb(36, 100, 240),
                    Color32::from_rgb(0, 200, 230),
                    Color32::from_rgb(45, 205, 115),
                    Color32::from_rgb(245, 224, 55),
                    Color32::from_rgb(255, 140, 35),
                    Color32::from_rgb(240, 45, 35),
                ];
                let position = value.clamp(0.0, 1.0) * (STOPS.len() - 1) as f32;
                let index = (position as usize).min(STOPS.len() - 2);
                return mix(STOPS[index], STOPS[index + 1], position - index as f32);
            }
            Self::Theme => (theme.accent_alt, theme.accent),
            Self::Ember => (
                Color32::from_rgb(140, 44, 70),
                Color32::from_rgb(255, 181, 70),
            ),
            Self::Ocean => (
                Color32::from_rgb(45, 72, 172),
                Color32::from_rgb(80, 225, 214),
            ),
        };
        if value < 0.4 {
            mix(theme.background, low, value / 0.4)
        } else if value < 0.85 {
            mix(low, high, (value - 0.4) / 0.45)
        } else {
            mix(high, theme.foreground, (value - 0.85) / 0.15)
        }
    }
}
