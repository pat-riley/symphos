mod camera_gizmo;
mod frequency;
mod spectrogram;
mod spectrum;
mod waterfall;
mod waveform;

use std::time::Instant;

use eframe::egui::{self, Color32, FontId, Pos2, Rect};

use crate::help::HoverHelp;
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
                "Audio amplitude over a short time window. Choose separate stereo channels or a combined signal in settings."
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
}

impl ModulePane {
    pub fn new(kind: ModuleKind) -> Self {
        Self {
            kind,
            waterfall: Waterfall::default(),
            spectrum: Spectrum::default(),
            waveform: Waveform::default(),
            spectrogram: Spectrogram::default(),
        }
    }

    pub fn reset(&mut self) {
        self.waterfall.clear();
        self.spectrum.clear();
        self.spectrogram.clear();
    }

    pub fn controls(&mut self, ui: &mut egui::Ui, frame: &AnalysisFrame) {
        ui.push_id(self.kind.label(), |ui| match self.kind {
            ModuleKind::Waterfall => self.waterfall.controls(ui),
            ModuleKind::Spectrum => self.spectrum.controls(ui),
            ModuleKind::Waveform => self.waveform.controls(ui, frame),
            ModuleKind::Spectrogram => self.spectrogram.controls(ui),
        });
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

pub(crate) fn settings_panel(
    ui: &mut egui::Ui,
    title: &str,
    open: bool,
    body: impl FnOnce(&mut egui::Ui),
) {
    ui.push_id(title, |ui| {
        ui.visuals_mut().collapsing_header_frame = true;
        egui::CollapsingHeader::new(egui::RichText::new(title).strong())
            .id_salt(title)
            .default_open(open)
            .show_background(true)
            .show(ui, |ui| {
                ui.add_space(3.0);
                body(ui);
                ui.add_space(5.0);
            })
            .header_response
            .help_text(format!(
                "Show or hide {title} settings. Other panels can stay open."
            ));
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
    fn module_switching_preserves_independent_section_states() {
        let context = egui::Context::default();
        let time = std::cell::Cell::new(0.0);
        let mut pane = ModulePane::new(ModuleKind::Spectrum);
        let render = |pane: &mut ModulePane, index: usize, events| {
            time.set(time.get() + 0.05);
            let mut output = context.run_ui(
                egui::RawInput {
                    time: Some(time.get()),
                    events,
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(300.0, 1400.0))),
                    ..Default::default()
                },
                |ui| {
                    ui.push_id(index, |ui| pane.controls(ui, &AnalysisFrame::default()));
                },
            );
            output.textures_delta.clear();
            output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) => Some((text.galley.job.text.clone(), text.pos)),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        let toggle = |pane: &mut ModulePane| {
            let labels = render(pane, 0, vec![]);
            let position = labels
                .iter()
                .find(|(text, _)| text == "Frequency Range")
                .expect("frequency header")
                .1
                + egui::vec2(5.0, 5.0);
            for pressed in [true, false] {
                render(
                    pane,
                    0,
                    vec![
                        egui::Event::PointerMoved(position),
                        egui::Event::PointerButton {
                            pos: position,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                );
            }
        };
        let range_visible = |pane: &mut ModulePane, index| {
            time.set(time.get() + 1.0);
            render(pane, index, vec![])
                .iter()
                .any(|(text, _)| text == "Low Hz")
        };
        assert!(range_visible(&mut pane, 0));
        toggle(&mut pane);
        assert!(!range_visible(&mut pane, 0));
        pane.kind = ModuleKind::Waterfall;
        assert!(!range_visible(&mut pane, 0));
        toggle(&mut pane);
        assert!(range_visible(&mut pane, 0));
        pane.kind = ModuleKind::Spectrum;
        assert!(!range_visible(&mut pane, 0));
        assert!(range_visible(&mut pane, 1));
        pane.kind = ModuleKind::Waterfall;
        assert!(range_visible(&mut pane, 0));
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
