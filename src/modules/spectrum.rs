use std::time::{Duration, Instant};

use eframe::egui::{self, Pos2, Rect, Sense, Stroke, Vec2};

use super::{
    ModuleRenderState, Palette, TraceStyle,
    frequency::{BANDS, FrequencyData, FrequencySettings},
    label, mix, settings_panel,
};
use crate::help::HoverHelp;
use crate::{analysis::AnalysisFrame, theme::AppTheme};

pub struct Spectrum {
    data: FrequencyData,
    applied_settings: Option<FrequencySettings>,
    reference: Option<ReferenceSpectrum>,
    pinned: Option<FrequencyPin>,
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

struct ReferenceSpectrum {
    magnitudes: Vec<f32>,
    sample_rate: u32,
    fft_size: usize,
    levels: [f32; BANDS],
    processed_settings: FrequencySettings,
    processed_sample_rate: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct FrequencyPin {
    frequency_hz: f32,
    dbfs: f32,
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
            reference: None,
            pinned: None,
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
    pub(super) const FACTORY_PRESETS: [&'static str; 3] =
        ["Balanced Bars", "Smooth Line", "Peak Inspector"];

    pub(super) fn factory_settings(index: usize) -> Option<SpectrumSettings> {
        let mut module = Self::default();
        match index {
            0 => {}
            1 => {
                module.style = TraceStyle::Line;
                module.data.settings.smoothing = 0.86;
                module.data.settings.frequency_smoothing = 2;
                module.peak_hold = true;
                module.palette = Palette::Ocean;
            }
            2 => {
                module.style = TraceStyle::Filled;
                module.data.settings.smoothing = 0.35;
                module.peak_hold = true;
                module.infinite_hold = true;
                module.palette = Palette::Heatmap;
                module.contrast = 1.35;
            }
            _ => return None,
        }
        Some(module.settings_snapshot())
    }

    pub fn reset_settings(&mut self) {
        let mut data = std::mem::take(&mut self.data);
        let reference = self.reference.take();
        data.settings = FrequencySettings::default();
        *self = Self {
            data,
            reference,
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
        self.pinned = None;
    }

    pub fn clear_cursor(&mut self) {
        self.pinned = None;
    }

    fn capture_reference(&mut self, frame: &AnalysisFrame) {
        if frame.bins.is_empty() {
            return;
        }
        let magnitudes: Vec<_> = frame.bins.iter().map(|bin| bin.magnitude).collect();
        let levels = FrequencyData::snapshot_levels(
            &self.data.settings,
            &magnitudes,
            frame.sample_rate,
            frame.fft_size,
            frame.sample_rate,
        );
        self.reference = Some(ReferenceSpectrum {
            magnitudes,
            sample_rate: frame.sample_rate,
            fft_size: frame.fft_size,
            levels,
            processed_settings: self.data.settings.clone(),
            processed_sample_rate: frame.sample_rate,
        });
    }

    fn refresh_reference(&mut self, display_sample_rate: u32) {
        let Some(reference) = &mut self.reference else {
            return;
        };
        if !reference
            .processed_settings
            .same_processing(&self.data.settings)
            || reference.processed_sample_rate != display_sample_rate
        {
            reference.levels = FrequencyData::snapshot_levels(
                &self.data.settings,
                &reference.magnitudes,
                reference.sample_rate,
                reference.fft_size,
                display_sample_rate,
            );
            reference.processed_settings = self.data.settings.clone();
            reference.processed_sample_rate = display_sample_rate;
        }
    }

    fn measurement_at(
        settings: &FrequencySettings,
        levels: &[f32; BANDS],
        fraction: f32,
        sample_rate: u32,
    ) -> FrequencyPin {
        let fraction = fraction.clamp(0.0, 0.999);
        let index = (fraction * BANDS as f32) as usize;
        FrequencyPin {
            frequency_hz: settings.frequency(fraction, sample_rate),
            dbfs: levels[index],
        }
    }

    pub fn controls(&mut self, ui: &mut egui::Ui, frame: &AnalysisFrame) {
        settings_panel(ui, "Frequency Range", |ui| {
            self.data.settings.range_controls(ui)
        });
        settings_panel(ui, "Signal Response", |ui| {
            self.data.settings.response_controls(ui);
            super::settings_group(ui, "Reference", |ui| {
                let capture = if self.reference.is_some() {
                    "Replace live reference"
                } else {
                    "Capture live reference"
                };
                if ui
                    .add_enabled(!frame.bins.is_empty(), egui::Button::new(capture))
                    .help_text("Capture the current spectrum as a fixed comparison while live analysis continues.")
                    .clicked()
                {
                    self.capture_reference(frame);
                }
                if ui
                    .add_enabled(
                        self.reference.is_some(),
                        egui::Button::new("Clear reference"),
                    )
                    .help_text("Remove only the captured comparison trace.")
                    .clicked()
                {
                    self.reference = None;
                }
            });
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
                if ui
                    .add_enabled(
                        self.pinned.is_some(),
                        egui::Button::new("Clear pinned measurement"),
                    )
                    .help_text("Remove the pane-local measurement crosshair.")
                    .clicked()
                {
                    self.pinned = None;
                }
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
        render_state: ModuleRenderState,
    ) {
        let ModuleRenderState { live, frozen } = render_state;
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
        self.refresh_reference(frame.sample_rate);
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
        if let Some(reference) = &self.reference {
            let points: Vec<_> = reference
                .levels
                .iter()
                .enumerate()
                .map(|(index, db)| {
                    Pos2::new(
                        plot.left() + (index as f32 + 0.5) * band_width,
                        plot.bottom() - settings.intensity(*db) * plot.height(),
                    )
                })
                .collect();
            painter.add(egui::Shape::line(
                points,
                Stroke::new(1.5, theme.foreground.gamma_multiply(0.65)),
            ));
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
        if frozen && let Some(pin) = self.pinned {
            let fraction = settings.fraction_for_frequency(pin.frequency_hz, frame.sample_rate);
            let x = plot.left() + fraction * plot.width();
            let y = plot.bottom() - settings.intensity(pin.dbfs) * plot.height();
            let stroke = Stroke::new(1.0, theme.warning);
            painter.line_segment(
                [Pos2::new(x, plot.top()), Pos2::new(x, plot.bottom())],
                stroke,
            );
            painter.line_segment(
                [Pos2::new(plot.left(), y), Pos2::new(plot.right(), y)],
                stroke,
            );
            label(
                &painter,
                Pos2::new(
                    (x + 5.0).min(plot.right() - 80.0),
                    (y - 16.0).max(plot.top()),
                ),
                format!(
                    "{} · {:.1} dBFS",
                    settings.label_frequency(pin.frequency_hz),
                    pin.dbfs
                ),
                theme.warning,
            );
        }
        let response = ui.interact(plot, ui.id().with("spectrum-measure"), Sense::click());
        if frozen
            && response.clicked()
            && let Some(pointer) = response.interact_pointer_pos()
        {
            let fraction = ((pointer.x - plot.left()) / plot.width()).clamp(0.0, 0.999);
            self.pinned = Some(Self::measurement_at(
                settings,
                &self.data.levels,
                fraction,
                frame.sample_rate,
            ));
        }
        if let Some(pointer) = response.hover_pos() {
            let t = ((pointer.x - plot.left()) / plot.width()).clamp(0.0, 0.999);
            let index = (t * BANDS as f32) as usize;
            let action = if frozen { " · click to pin" } else { "" };
            response.help_text(format!(
                "{} · {:.1} dBFS{action}",
                settings.label_frequency(settings.frequency(t, frame.sample_rate)),
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
                    ModuleRenderState {
                        live: true,
                        frozen: false,
                    },
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
            let mut controls = context.run_ui(egui::RawInput::default(), |ui| {
                spectrum.controls(ui, &AnalysisFrame::default())
            });
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
                    ModuleRenderState {
                        live: false,
                        frozen: false,
                    },
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

    #[test]
    fn live_reference_reprocesses_raw_bins_and_is_not_copied_as_a_setting() {
        let frame = AnalysisFrame {
            sequence: 7,
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
        let mut source = Spectrum::default();
        source.capture_reference(&frame);
        assert!(
            source
                .reference
                .as_ref()
                .unwrap()
                .levels
                .iter()
                .all(|level| (*level + 20.0).abs() < 0.01)
        );

        source.data.settings.gain = 6.0;
        source.refresh_reference(frame.sample_rate);
        assert!(
            source
                .reference
                .as_ref()
                .unwrap()
                .levels
                .iter()
                .all(|level| (*level + 14.0).abs() < 0.01)
        );

        let mut target = Spectrum::default();
        target.apply_settings(&source.settings_snapshot());
        assert!(target.reference.is_none());
        source.reset_settings();
        assert!(source.reference.is_some());
    }

    #[test]
    fn pinned_measurement_uses_the_selected_frequency_band_and_clears() {
        let mut spectrum = Spectrum::default();
        spectrum.data.levels[48] = -17.5;
        spectrum.pinned = Some(Spectrum::measurement_at(
            &spectrum.data.settings,
            &spectrum.data.levels,
            0.5,
            48_000,
        ));
        let pin = spectrum.pinned.unwrap();
        assert!((pin.frequency_hz - spectrum.data.settings.frequency(0.5, 48_000)).abs() < 0.01);
        assert_eq!(pin.dbfs, -17.5);
        spectrum.clear_cursor();
        assert!(spectrum.pinned.is_none());
    }
}
