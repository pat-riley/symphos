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
        match self.kind {
            ModuleKind::Waterfall => self.waterfall.controls(ui),
            ModuleKind::Spectrum => self.spectrum.controls(ui),
            ModuleKind::Waveform => self.waveform.controls(ui, frame),
            ModuleKind::Spectrogram => self.spectrogram.controls(ui),
        }
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
