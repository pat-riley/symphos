mod camera_gizmo;
mod frequency;
mod spectrogram;
mod spectrum;
mod waterfall;
mod waveform;

use std::time::Instant;

use eframe::egui::{self, Color32, FontId, Pos2, Rect};

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
            });
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
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Palette {
    #[default]
    Theme,
    Ember,
    Ocean,
}

impl Palette {
    fn controls(&mut self, ui: &mut egui::Ui) {
        egui::ComboBox::from_id_salt("palette")
            .selected_text(self.label())
            .show_ui(ui, |ui| {
                for value in [Self::Theme, Self::Ember, Self::Ocean] {
                    ui.selectable_value(self, value, value.label());
                }
            });
    }

    fn label(self) -> &'static str {
        match self {
            Self::Theme => "Theme colors",
            Self::Ember => "Ember",
            Self::Ocean => "Ocean",
        }
    }

    fn color(self, value: f32, theme: &AppTheme) -> Color32 {
        let (low, high) = match self {
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
