use std::ops::Range;

use eframe::egui::{self, Color32, FontId, Pos2, Rect, Stroke, Vec2};

use super::{mix, properties_combo, settings_panel};
use crate::help::HoverHelp;
use crate::{analysis::AnalysisFrame, theme::AppTheme};

const LOW_CROSSOVER_HZ: f32 = 200.0;
const HIGH_CROSSOVER_HZ: f32 = 2_000.0;
const MIN_FOLLOW_HZ: f32 = 30.0;
const MAX_FOLLOW_HZ: f32 = 2_000.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
enum ScopeChannel {
    Left,
    Right,
    Mid,
    Side,
}

impl ScopeChannel {
    const ALL: [Self; 4] = [Self::Left, Self::Right, Self::Mid, Self::Side];

    const fn label(self) -> &'static str {
        match self {
            Self::Left => "Left",
            Self::Right => "Right",
            Self::Mid => "Mid",
            Self::Side => "Side",
        }
    }

    const fn short_label(self) -> &'static str {
        match self {
            Self::Left => "L",
            Self::Right => "R",
            Self::Mid => "M",
            Self::Side => "S",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
enum SyncMode {
    Pitch,
    Trigger,
    Free,
}

impl SyncMode {
    const ALL: [Self; 3] = [Self::Pitch, Self::Trigger, Self::Free];

    const fn label(self) -> &'static str {
        match self {
            Self::Pitch => "Follow pitch",
            Self::Trigger => "Threshold trigger",
            Self::Free => "Free run",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
enum CycleMode {
    Single,
    Multi,
}

impl CycleMode {
    const ALL: [Self; 2] = [Self::Single, Self::Multi];

    const fn label(self) -> &'static str {
        match self {
            Self::Single => "Single cycle",
            Self::Multi => "Multiple cycles",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
enum ScopeColorMode {
    Static,
    Rgb,
    MultiBand,
}

impl ScopeColorMode {
    const ALL: [Self; 3] = [Self::Static, Self::Rgb, Self::MultiBand];

    const fn label(self) -> &'static str {
        match self {
            Self::Static => "Static",
            Self::Rgb => "RGB",
            Self::MultiBand => "Multi-band",
        }
    }
}

pub struct Oscilloscope {
    channel: ScopeChannel,
    sync_mode: SyncMode,
    cycle_mode: CycleMode,
    cycles: u32,
    timebase_ms: f32,
    trigger_level: f32,
    rising: bool,
    auto_scale: bool,
    amplitude: f32,
    color_mode: ScopeColorMode,
    thickness: f32,
    afterglow_traces: u32,
    afterglow_opacity: f32,
    centerline: bool,
    grid: bool,
    labels: bool,
}

module_settings!(Oscilloscope, OscilloscopeSettings, {
channel: ScopeChannel => channel,
sync_mode: SyncMode => sync_mode,
cycle_mode: CycleMode => cycle_mode,
cycles: u32 => cycles,
timebase_ms: f32 => timebase_ms,
trigger_level: f32 => trigger_level,
rising: bool => rising,
auto_scale: bool => auto_scale,
amplitude: f32 => amplitude,
color_mode: ScopeColorMode => color_mode,
thickness: f32 => thickness,
afterglow_traces: u32 => afterglow_traces,
afterglow_opacity: f32 => afterglow_opacity,
centerline: bool => centerline,
grid: bool => grid,
labels: bool => labels,
});

impl Default for Oscilloscope {
    fn default() -> Self {
        Self {
            channel: ScopeChannel::Mid,
            sync_mode: SyncMode::Pitch,
            cycle_mode: CycleMode::Multi,
            cycles: 3,
            timebase_ms: 20.0,
            trigger_level: 0.0,
            rising: true,
            auto_scale: true,
            amplitude: 1.0,
            color_mode: ScopeColorMode::Static,
            thickness: 1.4,
            afterglow_traces: 2,
            afterglow_opacity: 0.16,
            centerline: true,
            grid: true,
            labels: true,
        }
    }
}

impl Oscilloscope {
    pub(super) const FACTORY_PRESETS: [&'static str; 3] =
        ["Pitch Lock", "Single Cycle", "Triggered RGB"];

    pub(super) fn factory_settings(index: usize) -> Option<OscilloscopeSettings> {
        let mut module = Self::default();
        match index {
            0 => {}
            1 => {
                module.cycle_mode = CycleMode::Single;
                module.afterglow_traces = 4;
                module.afterglow_opacity = 0.22;
            }
            2 => {
                module.channel = ScopeChannel::Left;
                module.sync_mode = SyncMode::Trigger;
                module.timebase_ms = 10.0;
                module.color_mode = ScopeColorMode::Rgb;
                module.afterglow_traces = 3;
            }
            _ => return None,
        }
        Some(module.settings_snapshot())
    }

    pub fn clear(&mut self) {}

    pub fn reset_settings(&mut self) {
        *self = Self::default();
    }

    pub fn controls(&mut self, ui: &mut egui::Ui) {
        settings_panel(ui, "Channels", |ui| {
            super::settings_group(ui, "Signal", |ui| {
                properties_combo("oscilloscope-channel", 176.0, ScopeChannel::ALL.len())
                    .selected_text(self.channel.label())
                    .show_ui(ui, |ui| {
                        for channel in ScopeChannel::ALL {
                            ui.selectable_value(&mut self.channel, channel, channel.label())
                                .help_text(match channel {
                                    ScopeChannel::Left => "Original left input channel.",
                                    ScopeChannel::Right => "Original right input channel.",
                                    ScopeChannel::Mid => {
                                        "Mono-compatible center: (Left + Right) / 2."
                                    }
                                    ScopeChannel::Side => "Stereo difference: (Left − Right) / 2.",
                                });
                        }
                    })
                    .response
                    .help_text("Choose the signal whose repeating shape is inspected.");
            });
        });

        settings_panel(ui, "Synchronization", |ui| {
            super::settings_group(ui, "Lock", |ui| {
                properties_combo("oscilloscope-sync", 176.0, SyncMode::ALL.len())
                    .selected_text(self.sync_mode.label())
                    .show_ui(ui, |ui| {
                        for mode in SyncMode::ALL {
                            ui.selectable_value(&mut self.sync_mode, mode, mode.label())
                                .help_text(match mode {
                                    SyncMode::Pitch => "Estimate a 30 Hz–2 kHz period locally and align the view to a rising zero crossing.",
                                    SyncMode::Trigger => "Align the manual time window to the selected threshold and slope.",
                                    SyncMode::Free => "Show the newest manual time window without stabilization.",
                                });
                        }
                    })
                    .response
                    .help_text("Choose pitch-period locking, a conventional threshold trigger, or an unstabilized view.");
            });
            super::settings_group(ui, "Trigger", |ui| {
                ui.add_enabled_ui(self.sync_mode == SyncMode::Trigger, |ui| {
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut self.rising, true, "Rising")
                            .help_text("Start when the signal crosses the threshold upward.");
                        ui.selectable_value(&mut self.rising, false, "Falling")
                            .help_text("Start when the signal crosses the threshold downward.");
                    });
                    ui.add(
                        crate::parameter::Parameter::new(&mut self.trigger_level, -1.0..=1.0, 0.0)
                            .text("Threshold"),
                    )
                    .help_text(
                        "Trigger level in original sample amplitude before display scaling.",
                    );
                });
                if self.sync_mode == SyncMode::Pitch {
                    ui.small("Pitch follow uses a local 30 Hz–2 kHz autocorrelation estimate and a rising zero crossing.")
                        .help_text("This analysis is display-only and uses the selected channel's recent full-rate samples. It does not alter shared FFT analysis.");
                }
            });
        });

        settings_panel(ui, "Time Window", |ui| {
            super::settings_group(ui, "Cycles", |ui| {
                ui.add_enabled_ui(self.sync_mode == SyncMode::Pitch, |ui| {
                    properties_combo("oscilloscope-cycle-mode", 176.0, CycleMode::ALL.len())
                        .selected_text(self.cycle_mode.label())
                        .show_ui(ui, |ui| {
                            for mode in CycleMode::ALL {
                                ui.selectable_value(&mut self.cycle_mode, mode, mode.label())
                                    .help_text(match mode {
                                        CycleMode::Single => "Fit one detected pitch period across the pane.",
                                        CycleMode::Multi => "Fit the selected number of detected periods across the pane.",
                                    });
                            }
                        })
                        .response
                        .help_text("Choose a single detected cycle or several consecutive cycles.");
                    ui.add_enabled_ui(self.cycle_mode == CycleMode::Multi, |ui| {
                        ui.add(
                            crate::parameter::Parameter::new(&mut self.cycles, 2..=8, 3)
                                .bounds(2.0..=16.0)
                                .text("Cycles"),
                        )
                        .help_text("Number of pitch periods visible at once in Multiple cycles mode.");
                    });
                });
            });
            super::settings_group(ui, "Manual timebase", |ui| {
                ui.add_enabled_ui(self.sync_mode != SyncMode::Pitch, |ui| {
                    ui.add(
                        crate::parameter::Parameter::new(
                            &mut self.timebase_ms,
                            1.0..=100.0,
                            20.0,
                        )
                        .bounds(0.1..=500.0)
                        .logarithmic(true)
                        .text("Across ms"),
                    )
                    .help_text("Time spanned across the pane in Trigger and Free run modes; independent of FFT size.");
                });
            });
        });

        settings_panel(ui, "Amplitude", |ui| {
            super::settings_group(ui, "Scaling", |ui| {
                ui.checkbox(&mut self.auto_scale, "Auto scale")
                    .help_text("Fit the current trace with a small amount of headroom. Does not change audio volume.");
                ui.add_enabled_ui(!self.auto_scale, |ui| {
                    ui.add(
                        crate::parameter::Parameter::new(&mut self.amplitude, 0.1..=10.0, 1.0)
                            .bounds(0.01..=100.0)
                            .logarithmic(true)
                            .text("Amplitude"),
                    )
                    .help_text(
                        "Manual vertical magnification retained while Auto scale is enabled.",
                    );
                });
            });
        });

        settings_panel(ui, "Appearance", |ui| {
            super::settings_group(ui, "Trace", |ui| {
                ui.add(
                    crate::parameter::Parameter::new(&mut self.thickness, 0.5..=4.0, 1.4)
                        .bounds(0.1..=20.0)
                        .text("Line px"),
                )
                .help_text("Current trace thickness in screen pixels.");
                properties_combo(
                    "oscilloscope-color-mode",
                    176.0,
                    ScopeColorMode::ALL.len(),
                )
                .selected_text(self.color_mode.label())
                .show_ui(ui, |ui| {
                    for mode in ScopeColorMode::ALL {
                        ui.selectable_value(&mut self.color_mode, mode, mode.label())
                            .help_text(match mode {
                                ScopeColorMode::Static => "Use the current theme accent for the complete trace.",
                                ScopeColorMode::Rgb => "Blend red, green, and blue from local low-, mid-, and high-frequency energy.",
                                ScopeColorMode::MultiBand => "Overlay separate low, mid, and high wave shapes in red, green, and blue.",
                            });
                    }
                })
                .response
                .help_text("Choose one trace color, frequency-balanced RGB, or three filtered overlays.");
            });
            super::settings_group(ui, "Persistence", |ui| {
                ui.add(
                        crate::parameter::Parameter::new(&mut self.afterglow_traces, 0..=8, 2)
                            .bounds(0.0..=16.0)
                    .text("Older traces"),
                )
                .help_text("Overlay preceding windows at the same scale. Pitch-locked cycles align; manual windows may drift.");
                ui.add_enabled_ui(self.afterglow_traces > 0, |ui| {
                    ui.add(
                        crate::parameter::Parameter::new(
                            &mut self.afterglow_opacity,
                            0.03..=0.5,
                            0.16,
                        )
                        .bounds(0.0..=1.0)
                        .text("Afterglow"),
                    )
                    .help_text(
                        "Opacity of the newest historical trace; earlier traces fade further.",
                    );
                });
            });
            super::settings_group(ui, "Guides", |ui| {
                ui.checkbox(&mut self.centerline, "Centerline")
                    .help_text("Show the zero-amplitude reference.");
                ui.checkbox(&mut self.grid, "Grid")
                    .help_text("Show time divisions and half-scale amplitude guides.");
                ui.checkbox(&mut self.labels, "Status & axis labels")
                    .help_text(
                        "Show channel, time span, synchronization, pitch, and amplitude labels.",
                    );
            });
        });
    }

    pub fn draw(
        &self,
        ui: &mut egui::Ui,
        rect: Rect,
        frame: &AnalysisFrame,
        theme: &AppTheme,
        live: bool,
    ) {
        let painter = ui.painter_at(rect);
        let plot = rect.shrink2(Vec2::new(10.0, if self.labels { 22.0 } else { 10.0 }));
        if !plot.is_positive() {
            return;
        }
        paint_guides(&painter, plot, self.centerline, self.grid, theme);

        let signal = selected_signal(frame, self.channel);
        let selection = self.select_window(&signal, frame.sample_rate);
        if live && selection.range.len() >= 2 {
            let bands = split_bands(&signal, frame.sample_rate);
            let gain = self.display_gain(&signal, selection.range.clone());
            let layer = match self.color_mode {
                ScopeColorMode::Static => PlotLayer::Static,
                ScopeColorMode::Rgb => PlotLayer::Rgb,
                ScopeColorMode::MultiBand => PlotLayer::Band(0),
            };
            let afterglow = self.afterglow_traces.min(16) as usize;
            let span = selection.range.len().saturating_sub(1).max(1);
            for age in (1..=afterglow).rev() {
                let offset = age.saturating_mul(span);
                if selection.range.start < offset {
                    continue;
                }
                let start = selection.range.start - offset;
                let older = start..start + selection.range.len();
                let opacity = self.afterglow_opacity / age as f32;
                self.paint_layers(&painter, plot, &bands, older, layer, gain, opacity, theme);
            }
            self.paint_layers(
                &painter,
                plot,
                &bands,
                selection.range.clone(),
                layer,
                gain,
                1.0,
                theme,
            );
        }
        if self.labels {
            paint_labels(
                &painter,
                rect,
                frame.sample_rate,
                self.channel,
                self.sync_mode,
                &selection,
                self.auto_scale,
                theme,
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn paint_layers(
        &self,
        painter: &egui::Painter,
        plot: Rect,
        bands: &[BandSample],
        range: Range<usize>,
        layer: PlotLayer,
        gain: f32,
        opacity: f32,
        theme: &AppTheme,
    ) {
        if self.color_mode == ScopeColorMode::MultiBand {
            for band in 0..3 {
                paint_trace(
                    painter,
                    plot,
                    bands,
                    range.clone(),
                    PlotLayer::Band(band),
                    gain,
                    self.thickness,
                    opacity,
                    theme,
                );
            }
        } else {
            paint_trace(
                painter,
                plot,
                bands,
                range,
                layer,
                gain,
                self.thickness,
                opacity,
                theme,
            );
        }
    }

    fn display_gain(&self, signal: &[f32], range: Range<usize>) -> f32 {
        if self.auto_scale {
            let peak = signal[range]
                .iter()
                .map(|sample| sample.abs())
                .fold(0.0_f32, f32::max);
            (0.92 / peak.max(0.001)).clamp(0.1, 100.0)
        } else {
            self.amplitude
        }
    }

    fn select_window(&self, signal: &[f32], sample_rate: u32) -> ScopeSelection {
        let rate = sample_rate.max(1) as f32;
        let pitch = (self.sync_mode == SyncMode::Pitch)
            .then(|| estimate_period(signal, sample_rate))
            .flatten();
        let count = if let Some(estimate) = pitch {
            let cycles = if self.cycle_mode == CycleMode::Single {
                1.0
            } else {
                self.cycles.clamp(2, 16) as f32
            };
            (estimate.period_samples * cycles).round() as usize + 1
        } else {
            (self.timebase_ms * rate / 1_000.0).round() as usize
        }
        .clamp(2, signal.len().max(2))
        .min(signal.len());
        if count < 2 {
            return ScopeSelection {
                range: 0..signal.len(),
                pitch,
                triggered: false,
            };
        }
        let latest_start = signal.len() - count;
        let trigger = match self.sync_mode {
            SyncMode::Pitch => Some((0.0, true)),
            SyncMode::Trigger => Some((self.trigger_level, self.rising)),
            SyncMode::Free => None,
        };
        let start = trigger
            .and_then(|(level, rising)| find_crossing_before(signal, latest_start, level, rising))
            .unwrap_or(latest_start);
        ScopeSelection {
            range: start..start + count,
            pitch,
            triggered: trigger.is_some() && start != latest_start,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct PitchEstimate {
    period_samples: f32,
    frequency_hz: f32,
    confidence: f32,
}

#[derive(Clone, Debug)]
struct ScopeSelection {
    range: Range<usize>,
    pitch: Option<PitchEstimate>,
    triggered: bool,
}

fn selected_signal(frame: &AnalysisFrame, channel: ScopeChannel) -> Vec<f32> {
    frame
        .waveform_left
        .iter()
        .zip(&frame.waveform_right)
        .map(|(&left, &right)| match channel {
            ScopeChannel::Left => left,
            ScopeChannel::Right => right,
            ScopeChannel::Mid => (left + right) * 0.5,
            ScopeChannel::Side => (left - right) * 0.5,
        })
        .collect()
}

fn estimate_period(samples: &[f32], sample_rate: u32) -> Option<PitchEstimate> {
    let sample_rate = sample_rate.max(1) as usize;
    let step = sample_rate.div_ceil(12_000).max(1);
    let effective_rate = sample_rate as f32 / step as f32;
    let raw_start = samples.len().saturating_sub(step * 2_048);
    let mut reduced: Vec<f32> = samples[raw_start..]
        .chunks(step)
        .map(|chunk| chunk.iter().sum::<f32>() / chunk.len() as f32)
        .collect();
    if reduced.len() < 16 {
        return None;
    }
    let mean = reduced.iter().sum::<f32>() / reduced.len() as f32;
    for sample in &mut reduced {
        *sample -= mean;
    }
    let rms =
        (reduced.iter().map(|sample| sample * sample).sum::<f32>() / reduced.len() as f32).sqrt();
    if rms < 1.0e-4 {
        return None;
    }

    let min_lag = (effective_rate / MAX_FOLLOW_HZ).floor().max(2.0) as usize;
    let max_lag = (effective_rate / MIN_FOLLOW_HZ).ceil() as usize;
    let max_lag = max_lag.min(reduced.len() / 3);
    if min_lag + 2 >= max_lag {
        return None;
    }
    let mut correlations = vec![f32::NEG_INFINITY; max_lag + 1];
    for (lag, slot) in correlations
        .iter_mut()
        .enumerate()
        .take(max_lag + 1)
        .skip(min_lag)
    {
        let (mut cross, mut left_energy, mut right_energy) = (0.0_f64, 0.0_f64, 0.0_f64);
        for index in lag..reduced.len() {
            let left = f64::from(reduced[index]);
            let right = f64::from(reduced[index - lag]);
            cross += left * right;
            left_energy += left * left;
            right_energy += right * right;
        }
        let denominator = (left_energy * right_energy).sqrt();
        if denominator > 1.0e-15 {
            *slot = (cross / denominator) as f32;
        }
    }
    let (best_lag, &best) = correlations[min_lag..=max_lag]
        .iter()
        .enumerate()
        .max_by(|left, right| left.1.total_cmp(right.1))
        .map(|(index, value)| (index + min_lag, value))?;
    if best < 0.55 {
        return None;
    }
    let threshold = (best * 0.9).max(0.55);
    let lag = (min_lag + 1..max_lag)
        .find(|&lag| {
            correlations[lag] >= threshold
                && correlations[lag] >= correlations[lag - 1]
                && correlations[lag] >= correlations[lag + 1]
        })
        .unwrap_or(best_lag);
    let (before, peak, after) = (
        correlations[lag - 1],
        correlations[lag],
        correlations[lag + 1],
    );
    let curvature = before - 2.0 * peak + after;
    let offset = if curvature.abs() > 1.0e-6 {
        (0.5 * (before - after) / curvature).clamp(-0.5, 0.5)
    } else {
        0.0
    };
    let period_samples = (lag as f32 + offset) * step as f32;
    Some(PitchEstimate {
        period_samples,
        frequency_hz: sample_rate as f32 / period_samples,
        confidence: peak.clamp(0.0, 1.0),
    })
}

fn find_crossing_before(
    samples: &[f32],
    latest_start: usize,
    level: f32,
    rising: bool,
) -> Option<usize> {
    (1..=latest_start).rev().find(|&index| {
        if rising {
            samples[index - 1] < level && samples[index] >= level
        } else {
            samples[index - 1] > level && samples[index] <= level
        }
    })
}

#[derive(Clone, Copy, Debug, Default)]
struct BandSample {
    full: f32,
    components: [f32; 3],
    envelopes: [f32; 3],
}

fn split_bands(samples: &[f32], sample_rate: u32) -> Vec<BandSample> {
    let rate = sample_rate.max(1) as f32;
    let coefficient = |frequency: f32| 1.0 - (-std::f32::consts::TAU * frequency / rate).exp();
    let low_coefficient = coefficient(LOW_CROSSOVER_HZ);
    let mid_coefficient = coefficient(HIGH_CROSSOVER_HZ);
    let release = (-1.0 / (rate * 0.012)).exp();
    let mut low_pass = 0.0_f32;
    let mut mid_pass = 0.0_f32;
    let mut envelopes = [0.0_f32; 3];
    samples
        .iter()
        .map(|&sample| {
            low_pass += (sample - low_pass) * low_coefficient;
            mid_pass += (sample - mid_pass) * mid_coefficient;
            let components = [low_pass, mid_pass - low_pass, sample - mid_pass];
            for band in 0..3 {
                envelopes[band] = components[band].abs().max(envelopes[band] * release);
            }
            BandSample {
                full: sample,
                components,
                envelopes,
            }
        })
        .collect()
}

#[derive(Clone, Copy)]
enum PlotLayer {
    Static,
    Rgb,
    Band(usize),
}

fn layer_value(sample: &BandSample, layer: PlotLayer) -> f32 {
    match layer {
        PlotLayer::Static | PlotLayer::Rgb => sample.full,
        PlotLayer::Band(band) => sample.components[band],
    }
}

fn band_colors() -> [Color32; 3] {
    [
        Color32::from_rgb(245, 74, 70),
        Color32::from_rgb(106, 224, 88),
        Color32::from_rgb(76, 126, 255),
    ]
}

fn rgb_color(envelopes: [f32; 3], theme: &AppTheme) -> Color32 {
    let sum = envelopes.iter().sum::<f32>();
    if sum <= 1.0e-7 {
        return theme.muted;
    }
    let colors = band_colors();
    let weighted = |component: fn(&Color32) -> u8| {
        envelopes
            .iter()
            .zip(colors.iter())
            .map(|(&weight, color)| weight * component(color) as f32)
            .sum::<f32>()
            / sum
    };
    Color32::from_rgb(
        weighted(Color32::r).round() as u8,
        weighted(Color32::g).round() as u8,
        weighted(Color32::b).round() as u8,
    )
}

#[allow(clippy::too_many_arguments)]
fn paint_trace(
    painter: &egui::Painter,
    plot: Rect,
    samples: &[BandSample],
    range: Range<usize>,
    layer: PlotLayer,
    gain: f32,
    thickness: f32,
    opacity: f32,
    theme: &AppTheme,
) {
    if range.len() < 2 {
        return;
    }
    let reduced = reduce_trace(samples, range.clone(), layer, plot.width().ceil() as usize);
    let points: Vec<_> = reduced
        .iter()
        .map(|&(index, sample)| {
            let x = plot.left()
                + (index - range.start) as f32 / (range.len() - 1) as f32 * plot.width();
            let y = plot.center().y - (sample * gain).clamp(-1.0, 1.0) * plot.height() * 0.46;
            let color = match layer {
                PlotLayer::Static => theme.accent,
                PlotLayer::Rgb => rgb_color(samples[index].envelopes, theme),
                PlotLayer::Band(band) => band_colors()[band],
            }
            .gamma_multiply(opacity);
            (Pos2::new(x, y), color)
        })
        .collect();
    match layer {
        PlotLayer::Static | PlotLayer::Band(_) => {
            painter.add(egui::Shape::line(
                points.iter().map(|(point, _)| *point).collect(),
                Stroke::new(thickness, points[0].1),
            ));
        }
        PlotLayer::Rgb => {
            for pair in points.windows(2) {
                painter.line_segment(
                    [pair[0].0, pair[1].0],
                    Stroke::new(thickness, mix(pair[0].1, pair[1].1, 0.5)),
                );
            }
        }
    }
}

fn reduce_trace(
    samples: &[BandSample],
    range: Range<usize>,
    layer: PlotLayer,
    pixels: usize,
) -> Vec<(usize, f32)> {
    let stride = range.len().div_ceil(pixels.max(1)).max(1);
    let mut result = Vec::with_capacity(pixels.min(range.len()) * 2 + 1);
    for (chunk_index, chunk) in samples[range.clone()].chunks(stride).enumerate() {
        let start = range.start + chunk_index * stride;
        let mut low = 0;
        let mut high = 0;
        for index in 1..chunk.len() {
            if layer_value(&chunk[index], layer) < layer_value(&chunk[low], layer) {
                low = index;
            }
            if layer_value(&chunk[index], layer) > layer_value(&chunk[high], layer) {
                high = index;
            }
        }
        for index in if low <= high {
            [low, high]
        } else {
            [high, low]
        } {
            let absolute = start + index;
            if result.last().is_none_or(|&(last, _)| last != absolute) {
                result.push((absolute, layer_value(&samples[absolute], layer)));
            }
        }
    }
    if result
        .last()
        .is_none_or(|&(index, _)| index + 1 != range.end)
    {
        let index = range.end - 1;
        result.push((index, layer_value(&samples[index], layer)));
    }
    result
}

fn paint_guides(
    painter: &egui::Painter,
    plot: Rect,
    centerline: bool,
    grid: bool,
    theme: &AppTheme,
) {
    let guide = Stroke::new(0.7, mix(theme.background, theme.muted, 0.45));
    if centerline {
        painter.line_segment([plot.left_center(), plot.right_center()], guide);
    }
    if grid {
        for position in [0.25, 0.5, 0.75] {
            let x = plot.left() + position * plot.width();
            painter.line_segment(
                [Pos2::new(x, plot.top()), Pos2::new(x, plot.bottom())],
                guide,
            );
        }
        for amplitude in [-0.5, 0.5] {
            let y = plot.center().y - amplitude * plot.height() * 0.46;
            painter.line_segment(
                [Pos2::new(plot.left(), y), Pos2::new(plot.right(), y)],
                guide,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_labels(
    painter: &egui::Painter,
    rect: Rect,
    sample_rate: u32,
    channel: ScopeChannel,
    sync_mode: SyncMode,
    selection: &ScopeSelection,
    auto_scale: bool,
    theme: &AppTheme,
) {
    let duration_ms = selection.range.len() as f32 / sample_rate.max(1) as f32 * 1_000.0;
    painter.text(
        rect.left_top() + Vec2::new(10.0, 4.0),
        egui::Align2::LEFT_TOP,
        format!(
            "{} · {}",
            channel.short_label(),
            if auto_scale {
                "auto scale"
            } else {
                "manual scale"
            }
        ),
        FontId::monospace(10.0),
        theme.muted,
    );
    let status = match (sync_mode, selection.pitch) {
        (SyncMode::Pitch, Some(pitch)) => format!(
            "{:.1} Hz · {:.0}% lock",
            pitch.frequency_hz,
            pitch.confidence * 100.0
        ),
        (SyncMode::Pitch, None) => "pitch unlocked · manual fallback".into(),
        (SyncMode::Trigger, _) if selection.triggered => "triggered".into(),
        (SyncMode::Trigger, _) => "waiting for trigger · latest window".into(),
        (SyncMode::Free, _) => "free run".into(),
    };
    painter.text(
        rect.right_top() + Vec2::new(-10.0, 4.0),
        egui::Align2::RIGHT_TOP,
        status,
        FontId::monospace(10.0),
        theme.muted,
    );
    painter.text(
        rect.left_bottom() + Vec2::new(10.0, -5.0),
        egui::Align2::LEFT_BOTTOM,
        format!("{duration_ms:.2} ms across"),
        FontId::monospace(10.0),
        theme.muted,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(frequency: f32, sample_rate: u32, seconds: f32) -> Vec<f32> {
        (0..(sample_rate as f32 * seconds) as usize)
            .map(|index| {
                (std::f32::consts::TAU * frequency * index as f32 / sample_rate as f32).sin()
            })
            .collect()
    }

    #[test]
    fn pitch_follow_estimates_known_tones_and_rejects_silence() {
        for frequency in [55.0, 110.0, 440.0, 1_200.0] {
            let estimate = estimate_period(&sine(frequency, 48_000, 0.5), 48_000).unwrap();
            assert!((estimate.frequency_hz - frequency).abs() / frequency < 0.025);
            assert!(estimate.confidence > 0.8);
        }
        assert!(estimate_period(&vec![0.0; 24_000], 48_000).is_none());
    }

    #[test]
    fn channels_use_standard_mid_side_scaling() {
        let frame = AnalysisFrame {
            waveform_left: vec![1.0, -0.5],
            waveform_right: vec![-1.0, 0.5],
            ..AnalysisFrame::default()
        };
        assert_eq!(selected_signal(&frame, ScopeChannel::Left), [1.0, -0.5]);
        assert_eq!(selected_signal(&frame, ScopeChannel::Right), [-1.0, 0.5]);
        assert_eq!(selected_signal(&frame, ScopeChannel::Mid), [0.0, 0.0]);
        assert_eq!(selected_signal(&frame, ScopeChannel::Side), [1.0, -0.5]);
    }

    #[test]
    fn pitch_window_fits_requested_cycles_and_trigger_aligns() {
        let samples = sine(440.0, 48_000, 0.5);
        let scope = Oscilloscope {
            cycle_mode: CycleMode::Single,
            ..Oscilloscope::default()
        };
        let selection = scope.select_window(&samples, 48_000);
        assert!((selection.range.len() as f32 - 48_000.0 / 440.0).abs() < 3.0);
        assert!(samples[selection.range.start - 1] < 0.0);
        assert!(samples[selection.range.start] >= 0.0);

        let scope = Oscilloscope {
            sync_mode: SyncMode::Trigger,
            timebase_ms: 10.0,
            trigger_level: 0.5,
            rising: true,
            ..Oscilloscope::default()
        };
        let selection = scope.select_window(&samples, 48_000);
        assert!(selection.triggered);
        assert!(samples[selection.range.start - 1] < 0.5);
        assert!(samples[selection.range.start] >= 0.5);
    }

    #[test]
    fn all_sync_and_color_modes_render_finite_geometry_and_copy_settings() {
        let context = egui::Context::default();
        let wave = sine(220.0, 48_000, 0.5);
        let frame = AnalysisFrame {
            sequence: 1,
            waveform_left: wave.clone(),
            waveform_right: wave,
            ..AnalysisFrame::default()
        };
        for sync_mode in SyncMode::ALL {
            for color_mode in ScopeColorMode::ALL {
                let mut scope = Oscilloscope {
                    sync_mode,
                    color_mode,
                    ..Oscilloscope::default()
                };
                let settings = scope.settings_snapshot();
                scope.reset_settings();
                scope.apply_settings(&settings);
                assert_eq!(scope.settings_snapshot(), settings);
                let mut output = context.run_ui(egui::RawInput::default(), |ui| {
                    scope.draw(
                        ui,
                        Rect::from_min_size(Pos2::ZERO, Vec2::new(520.0, 300.0)),
                        &frame,
                        &AppTheme::default(),
                        true,
                    );
                });
                output.textures_delta.clear();
                let primitives = context.tessellate(output.shapes, output.pixels_per_point);
                let vertices: Vec<_> = primitives
                    .iter()
                    .flat_map(|primitive| match &primitive.primitive {
                        egui::epaint::Primitive::Mesh(mesh) => mesh.vertices.as_slice(),
                        _ => &[],
                    })
                    .collect();
                assert!(vertices.len() > 100);
                assert!(vertices.iter().all(|vertex| vertex.pos.is_finite()));
            }
        }
    }
}
