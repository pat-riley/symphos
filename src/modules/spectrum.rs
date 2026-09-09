use std::time::{Duration, Instant};

use eframe::egui::{self, Pos2, Rect, Sense, Stroke, Vec2};

use super::{
    Palette, TraceStyle,
    frequency::{BANDS, FrequencyData, FrequencySettings},
    frequency_label, label, mix, settings_panel,
};
use crate::help::HoverHelp;
use crate::{analysis::AnalysisFrame, theme::AppTheme};

pub struct Spectrum {
    data: FrequencyData,
    applied_settings: Option<FrequencySettings>,
    peak_hold: bool,
    hold_seconds: f32,
    peak_falloff: f32,
    infinite_hold: bool,
    peaks: [f32; BANDS],
    peak_times: [Option<Instant>; BANDS],
    last_peak_update: Option<Instant>,
    style: TraceStyle,
    bar_gap: f32,
    thickness: f32,
    fill_opacity: f32,
    palette: Palette,
    contrast: f32,
    grid: bool,
    labels: bool,
}

module_settings!(Spectrum, SpectrumSettings, {
frequency: super::frequency::FrequencySettings => data.settings,
peak_hold: bool => peak_hold,
hold_seconds: f32 => hold_seconds,
peak_falloff: f32 => peak_falloff,
infinite_hold: bool => infinite_hold,
style: TraceStyle => style,
bar_gap: f32 => bar_gap,
thickness: f32 => thickness,
fill_opacity: f32 => fill_opacity,
palette: Palette => palette,
contrast: f32 => contrast,
grid: bool => grid,
labels: bool => labels,
});

impl Default for Spectrum {
    fn default() -> Self {
        Self {
            data: FrequencyData::default(),
            applied_settings: None,
            peak_hold: false,
            hold_seconds: 1.0,
            peak_falloff: 18.0,
            infinite_hold: false,
            peaks: [-120.0; BANDS],
            peak_times: [None; BANDS],
            last_peak_update: None,
            style: TraceStyle::Bars,
            bar_gap: 15.0,
            thickness: 1.5,
            fill_opacity: 0.3,
            palette: Palette::Theme,
            contrast: 1.0,
            grid: true,
            labels: true,
        }
    }
}

impl Spectrum {
    pub fn reset_settings(&mut self) {
        let mut data = std::mem::take(&mut self.data);
        data.settings = FrequencySettings::default();
        *self = Self {
            data,
            ..Self::default()
        };
    }

    fn reset_peaks(&mut self) {
        self.peaks.fill(-120.0);
        self.peak_times.fill(None);
        self.last_peak_update = None;
    }

    pub fn clear(&mut self) {
        self.data.clear();
        self.reset_peaks();
        self.applied_settings = None;
    }

    pub fn controls(&mut self, ui: &mut egui::Ui) {
        settings_panel(ui, "Frequency Range", |ui| {
            self.data.settings.range_controls(ui)
        });
        settings_panel(ui, "Signal Response", |ui| {
            self.data.settings.response_controls(ui);
            super::settings_group(ui, "Peak markers", |ui| {
                if ui.checkbox(&mut self.peak_hold, "Peak hold")
                .help_text("Track recent maxima above the live trace. Starts a fresh measurement when enabled.").changed() {
                self.reset_peaks();
            }
                ui.add_enabled_ui(self.peak_hold, |ui| {
                ui.checkbox(&mut self.infinite_hold, "Hold indefinitely")
                    .help_text("Keep each band's highest level until Reset peaks. Disables timed release.");
                ui.add_enabled_ui(!self.infinite_hold, |ui| {
                    ui.add(crate::parameter::Parameter::new(&mut self.hold_seconds, 0.0..=10.0, 1.0).bounds(0.0..=60.0).text("Hold s"))
                        .help_text("Time to retain each peak before it starts falling. Zero begins release immediately.");
                    ui.add(crate::parameter::Parameter::new(&mut self.peak_falloff, 1.0..=96.0, 18.0).bounds(0.0..=240.0).text("Fall dB/s"))
                        .help_text("Peak-marker release speed, independent of the live spectrum's decay.");
                });
                if ui.small_button("Reset peaks").help_text("Clear only the peak markers, without resetting the trace or settings.").clicked() {
                    self.reset_peaks();
                }
            });
            });
        });
        settings_panel(ui, "Appearance", |ui| {
            super::settings_group(ui, "Trace", |ui| {
                self.style.controls(ui, true);
                if self.style == TraceStyle::Bars {
                    ui.add(
                        crate::parameter::Parameter::new(&mut self.bar_gap, 0.0..=90.0, 15.0)
                            .text("Gap %"),
                    )
                    .help_text(
                        "Space between bars as a percentage of each frequency band's width.",
                    );
                } else {
                    ui.add(
                        crate::parameter::Parameter::new(&mut self.thickness, 0.5..=4.0, 1.5)
                            .bounds(0.1..=20.0)
                            .text("Line px"),
                    )
                    .help_text("Trace outline thickness in screen pixels.");
                }
                if self.style == TraceStyle::Filled {
                    ui.add(
                        crate::parameter::Parameter::new(&mut self.fill_opacity, 0.05..=1.0, 0.3)
                            .bounds(0.0..=1.0)
                            .text("Fill opacity"),
                    )
                    .help_text("Opacity of the filled area beneath the spectrum.");
                }
            });
            super::settings_group(ui, "Color & level", |ui| {
                self.palette.controls(ui);
                self.palette.contrast_controls(ui, &mut self.contrast);
                self.data.settings.level_controls(ui);
            });
            super::settings_group(ui, "Guides", |ui| {
                ui.checkbox(&mut self.grid, "Grid")
                    .help_text("Show horizontal dB reference lines.");
                ui.checkbox(&mut self.labels, "Axis labels").help_text(
                    "Show dB and frequency labels. Frequency Range can replace Hz with note names.",
                );
            });
        });
    }

    fn update_peaks(&mut self, now: Instant) {
        let previous = self.last_peak_update.unwrap_or(now);
        for i in 0..BANDS {
            let level = self.data.levels[i];
            if level >= self.peaks[i] {
                self.peaks[i] = level;
                self.peak_times[i] = Some(now);
            } else if !self.infinite_hold {
                let held_until =
                    self.peak_times[i].unwrap_or(now) + Duration::from_secs_f32(self.hold_seconds);
                let elapsed = now
                    .saturating_duration_since(previous.max(held_until))
                    .as_secs_f32();
                self.peaks[i] = (self.peaks[i] - elapsed * self.peak_falloff).max(level);
            }
        }
        self.last_peak_update = Some(now);
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
            if self
                .applied_settings
                .as_ref()
                .is_some_and(|old| !old.same_processing(&self.data.settings))
            {
                self.data.clear();
                self.reset_peaks();
                // Re-map the retained frame immediately, including while paused.
                // Starting a fresh attack from silence would leave a frozen trace
                // artificially attenuated until capture resumes.
                let smoothing = self.data.settings.smoothing;
                self.data.settings.smoothing = 0.0;
                self.data.update(frame, now);
                self.data.settings.smoothing = smoothing;
            }
            self.applied_settings = Some(self.data.settings.clone());
            let (_, reset) = self.data.update(frame, now);
            if reset {
                self.reset_peaks();
            }
            if self.peak_hold {
                self.update_peaks(now);
            }
        }
        let painter = ui.painter_at(rect);
        let plot = Rect::from_min_max(
            rect.min + Vec2::new(if self.labels { 36.0 } else { 10.0 }, 12.0),
            rect.max - Vec2::new(12.0, if self.labels { 24.0 } else { 10.0 }),
        );
        if !plot.is_positive() {
            return;
        }
        let settings = &self.data.settings;
        for step in 0..=3 {
            let t = step as f32 / 3.0;
            let y = plot.bottom() - t * plot.height();
            if self.grid {
                painter.line_segment(
                    [Pos2::new(plot.left(), y), Pos2::new(plot.right(), y)],
                    Stroke::new(0.5, mix(theme.background, theme.muted, 0.35)),
                );
            }
            if self.labels {
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
        }
        let color = |intensity| {
            if self.palette == Palette::Theme {
                mix(theme.accent_alt, theme.accent, intensity)
            } else {
                self.palette
                    .color_with_contrast(intensity, self.contrast, theme)
            }
        };
        let band_width = plot.width() / BANDS as f32;
        let mut points = Vec::with_capacity(BANDS);
        let mut peak_points = Vec::with_capacity(BANDS);
        for (i, db) in self.data.levels.iter().enumerate() {
            let x = plot.left() + (i as f32 + 0.5) * band_width;
            let width = band_width * (1.0 - self.bar_gap / 100.0);
            let intensity = if live { settings.intensity(*db) } else { 0.0 };
            let point = Pos2::new(x, plot.bottom() - intensity * plot.height());
            points.push(point);
            if self.style == TraceStyle::Bars {
                painter.rect_filled(
                    Rect::from_min_max(
                        Pos2::new(x - width * 0.5, point.y),
                        Pos2::new(x + width * 0.5, plot.bottom()),
                    ),
                    0.5,
                    color(intensity),
                );
            }
            if self.peak_hold && live {
                let y = plot.bottom() - settings.intensity(self.peaks[i]) * plot.height();
                peak_points.push(Pos2::new(x, y));
                if self.style == TraceStyle::Bars {
                    painter.line_segment(
                        [Pos2::new(x - width * 0.5, y), Pos2::new(x + width * 0.5, y)],
                        Stroke::new(1.0, theme.foreground),
                    );
                }
            }
        }
        if self.style != TraceStyle::Bars {
            // Per-segment color follows actual dB intensity for every palette.
            for pair in points.windows(2) {
                let intensity = (plot.bottom() - (pair[0].y + pair[1].y) * 0.5) / plot.height();
                if self.style == TraceStyle::Filled {
                    super::fill_trace(
                        &painter,
                        pair,
                        plot.bottom(),
                        color(intensity).gamma_multiply(self.fill_opacity),
                    );
                }
                painter.line_segment(
                    [pair[0], pair[1]],
                    Stroke::new(self.thickness, color(intensity)),
                );
            }
            if peak_points.len() > 1 {
                painter.add(egui::Shape::line(
                    peak_points,
                    Stroke::new(1.0, theme.foreground),
                ));
            }
        }
        if self.labels {
            for step in 0..=4 {
                let t = step as f32 / 4.0;
                label(
                    &painter,
                    Pos2::new(plot.left() + t * (plot.width() - 25.0), plot.bottom() + 7.0),
                    settings.axis_label(t, frame.sample_rate),
                    theme.muted,
                );
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frozen_spectrum_reprocesses_gain_without_waiting_for_a_new_frame() {
        let context = egui::Context::default();
        let mut spectrum = Spectrum::default();
        let now = Instant::now();
        let frame = AnalysisFrame {
            sequence: 1,
            fft_size: 512,
            bins: vec![
                crate::analysis::FrequencyBin {
                    magnitude: 0.1,
                    ..Default::default()
                };
                257
            ],
            ..AnalysisFrame::default()
        };
        let render = |spectrum: &mut Spectrum| {
            let mut output = context.run_ui(egui::RawInput::default(), |ui| {
                spectrum.draw(
                    ui,
                    Rect::from_min_size(Pos2::ZERO, egui::vec2(500.0, 300.0)),
                    &frame,
                    &AppTheme::default(),
                    now,
                    true,
                );
            });
            output.textures_delta.clear();
        };
        render(&mut spectrum);
        spectrum.data.settings.gain = 6.0;
        render(&mut spectrum);
        assert!(
            spectrum
                .data
                .levels
                .iter()
                .all(|db| (*db + 14.0).abs() < 0.01)
        );
        spectrum.data.settings.floor = -100.0;
        spectrum.style = TraceStyle::Filled;
        render(&mut spectrum);
        assert!(
            spectrum
                .data
                .levels
                .iter()
                .all(|db| (*db + 14.0).abs() < 0.01)
        );
    }

    #[test]
    fn styles_show_only_relevant_controls_and_guides_can_be_fully_hidden() {
        let context = egui::Context::default();
        for style in [TraceStyle::Bars, TraceStyle::Line, TraceStyle::Filled] {
            let mut spectrum = Spectrum {
                style,
                grid: false,
                labels: false,
                ..Spectrum::default()
            };
            let mut controls =
                context.run_ui(egui::RawInput::default(), |ui| spectrum.controls(ui));
            controls.textures_delta.clear();
            let labels: Vec<_> = controls
                .shapes
                .iter()
                .filter_map(|s| match &s.shape {
                    egui::Shape::Text(text) => Some(text.galley.job.text.as_str()),
                    _ => None,
                })
                .collect();
            assert_eq!(labels.contains(&"Gap %"), style == TraceStyle::Bars);
            assert_eq!(labels.contains(&"Line px"), style != TraceStyle::Bars);
            assert_eq!(
                labels.contains(&"Fill opacity"),
                style == TraceStyle::Filled
            );
            let mut output = context.run_ui(egui::RawInput::default(), |ui| {
                spectrum.draw(
                    ui,
                    Rect::from_min_size(Pos2::ZERO, egui::vec2(150.0, 100.0)),
                    &AnalysisFrame::default(),
                    &AppTheme::default(),
                    Instant::now(),
                    false,
                );
            });
            output.textures_delta.clear();
            assert!(
                !output
                    .shapes
                    .iter()
                    .any(|s| matches!(&s.shape, egui::Shape::Text(_)))
            );
            for primitive in context.tessellate(output.shapes, output.pixels_per_point) {
                if let egui::epaint::Primitive::Mesh(mesh) = primitive.primitive {
                    assert!(mesh.vertices.iter().all(|v| v.pos.is_finite()));
                }
            }
        }
    }

    #[test]
    fn peaks_hold_then_release_at_the_requested_rate_and_never_below_trace() {
        let start = Instant::now();
        let mut spectrum = Spectrum {
            hold_seconds: 1.0,
            peak_falloff: 20.0,
            ..Spectrum::default()
        };
        spectrum.data.levels.fill(-10.0);
        spectrum.update_peaks(start);
        spectrum.data.levels.fill(-60.0);
        spectrum.update_peaks(start + Duration::from_millis(500));
        assert_eq!(spectrum.peaks[0], -10.0);
        spectrum.update_peaks(start + Duration::from_millis(1500));
        assert_eq!(spectrum.peaks[0], -20.0);
        spectrum.update_peaks(start + Duration::from_secs(10));
        assert_eq!(spectrum.peaks[0], -60.0);
    }

    #[test]
    fn infinite_hold_and_frozen_clock_do_not_decay_peaks() {
        let now = Instant::now();
        let mut spectrum = Spectrum {
            infinite_hold: true,
            ..Spectrum::default()
        };
        spectrum.data.levels.fill(-5.0);
        spectrum.update_peaks(now);
        spectrum.data.levels.fill(-80.0);
        spectrum.update_peaks(now + Duration::from_secs(60));
        assert_eq!(spectrum.peaks[0], -5.0);
        spectrum.infinite_hold = false;
        spectrum.update_peaks(now + Duration::from_secs(60));
        assert_eq!(spectrum.peaks[0], -5.0);
        spectrum.reset_peaks();
        assert!(spectrum.peaks.iter().all(|p| *p == -120.0));
    }
}
