use std::time::Instant;

use eframe::egui::{self, Pos2, Rect, Sense, Stroke, Vec2};

use super::{
    frequency::{BANDS, FrequencyData},
    frequency_label, label, mix, settings_panel,
};
use crate::help::HoverHelp;
use crate::{analysis::AnalysisFrame, theme::AppTheme};

pub struct Spectrum {
    data: FrequencyData,
    peak_hold: bool,
    peaks: [f32; BANDS],
}

impl Default for Spectrum {
    fn default() -> Self {
        Self {
            data: FrequencyData::default(),
            peak_hold: false,
            peaks: [-120.0; BANDS],
        }
    }
}

impl Spectrum {
    pub fn clear(&mut self) {
        self.data.clear();
        self.peaks.fill(-120.0);
    }

    pub fn controls(&mut self, ui: &mut egui::Ui) {
        settings_panel(ui, "Frequency Range", true, |ui| {
            self.data.settings.range_controls(ui)
        });
        settings_panel(ui, "Signal Response", false, |ui| {
            self.data.settings.response_controls(ui)
        });
        settings_panel(ui, "Appearance", true, |ui| {
            self.data.settings.level_controls(ui);
            ui.checkbox(&mut self.peak_hold, "Peak hold")
                .help_text("Keep a marker at the highest observed level for each frequency band.");
            if ui
                .small_button("Reset peaks")
                .help_text("Clear peak-hold markers and begin measuring new peaks.")
                .clicked()
            {
                self.peaks.fill(-120.0);
            }
        });
    }

    pub fn draw(
        &mut self,
        ui: &mut egui::Ui,
        rect: Rect,
        frame: &AnalysisFrame,
        theme: &AppTheme,
        now: Instant,
        live: bool,
    ) {
        if live {
            let (changed, reset) = self.data.update(frame, now);
            if reset {
                self.peaks.fill(-120.0);
            }
            if changed {
                for (peak, level) in self.peaks.iter_mut().zip(self.data.levels) {
                    *peak = peak.max(level);
                }
            }
        }
        let painter = ui.painter_at(rect);
        let plot = Rect::from_min_max(
            rect.min + Vec2::new(32.0, 12.0),
            rect.max - Vec2::new(12.0, 24.0),
        );
        let settings = &self.data.settings;
        for step in 0..=3 {
            let t = step as f32 / 3.0;
            let y = plot.bottom() - t * plot.height();
            painter.line_segment(
                [Pos2::new(plot.left(), y), Pos2::new(plot.right(), y)],
                Stroke::new(0.5, mix(theme.background, theme.muted, 0.35)),
            );
            label(
                &painter,
                Pos2::new(rect.left() + 3.0, y - 5.0),
                format!(
                    "{:.0}",
                    settings.floor + t * (settings.ceiling - settings.floor)
                ),
                theme.muted,
            );
        }
        for (i, db) in self.data.levels.iter().enumerate() {
            let x = plot.left() + i as f32 / BANDS as f32 * plot.width();
            let width = (plot.width() / BANDS as f32 - 1.0).max(0.5);
            let intensity = if live { settings.intensity(*db) } else { 0.0 };
            painter.rect_filled(
                Rect::from_min_max(
                    Pos2::new(x, plot.bottom() - intensity * plot.height()),
                    Pos2::new(x + width, plot.bottom()),
                ),
                0.5,
                mix(theme.accent_alt, theme.accent, intensity),
            );
            if self.peak_hold && live {
                let y = plot.bottom() - settings.intensity(self.peaks[i]) * plot.height();
                painter.line_segment(
                    [Pos2::new(x, y), Pos2::new(x + width, y)],
                    Stroke::new(1.0, theme.foreground),
                );
            }
        }
        for step in 0..=4 {
            let t = step as f32 / 4.0;
            label(
                &painter,
                Pos2::new(plot.left() + t * (plot.width() - 25.0), plot.bottom() + 7.0),
                frequency_label(settings.frequency(t, frame.sample_rate)),
                theme.muted,
            );
        }
        let response = ui.interact(plot, ui.id().with("spectrum-hover"), Sense::hover());
        if let Some(pointer) = response.hover_pos() {
            let t = ((pointer.x - plot.left()) / plot.width()).clamp(0.0, 0.999);
            let index = (t * BANDS as f32) as usize;
            response.help_text(format!(
                "{} Hz · {:.1} dBFS",
                frequency_label(settings.frequency(t, frame.sample_rate)),
                self.data.levels[index]
            ));
        }
    }
}
