use std::time::Instant;

use eframe::egui::{self, Color32, Pos2, Rect, Stroke, Vec2};

use super::{Palette, TraceStyle, label, mix, properties_combo, settings_panel};
use crate::help::HoverHelp;
use crate::{analysis::AnalysisFrame, theme::AppTheme};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
enum WaveformChannel {
    Left,
    Right,
    Mid,
    Side,
}

impl WaveformChannel {
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
enum WaveformColorMode {
    Static,
    MultiBand,
    ColorMap,
}

impl WaveformColorMode {
    const ALL: [Self; 3] = [Self::Static, Self::MultiBand, Self::ColorMap];

    const fn label(self) -> &'static str {
        match self {
            Self::Static => "Static",
            Self::MultiBand => "Multi-band",
            Self::ColorMap => "Color map",
        }
    }
}

pub struct Waveform {
    speed_ms: f32,
    amplitude: f32,
    auto_scale: bool,
    trigger: bool,
    rising: bool,
    trigger_level: f32,
    style: TraceStyle,
    thickness: f32,
    fill_opacity: f32,
    centerline: bool,
    grid: bool,
    labels: bool,
    two_channels: bool,
    channels: [WaveformChannel; 2],
    color_mode: WaveformColorMode,
    palette: Palette,
    contrast: f32,
    peak_history: bool,
    looped: bool,
    loop_cursor: usize,
    last_sequence: Option<u64>,
    last_update: Option<Instant>,
}

module_settings!(Waveform, WaveformSettings, {
speed_ms: f32 => speed_ms,
amplitude: f32 => amplitude,
auto_scale: bool => auto_scale,
trigger: bool => trigger,
rising: bool => rising,
trigger_level: f32 => trigger_level,
style: TraceStyle => style,
thickness: f32 => thickness,
fill_opacity: f32 => fill_opacity,
centerline: bool => centerline,
grid: bool => grid,
labels: bool => labels,
two_channels: bool => two_channels,
channels: [WaveformChannel; 2] => channels,
color_mode: WaveformColorMode => color_mode,
palette: Palette => palette,
contrast: f32 => contrast,
peak_history: bool => peak_history,
looped: bool => looped,
});

impl Default for Waveform {
    fn default() -> Self {
        Self {
            speed_ms: 250.0,
            amplitude: 1.0,
            auto_scale: false,
            trigger: false,
            rising: true,
            trigger_level: 0.0,
            style: TraceStyle::Line,
            thickness: 1.2,
            fill_opacity: 0.3,
            centerline: true,
            grid: false,
            labels: true,
            two_channels: true,
            channels: [WaveformChannel::Left, WaveformChannel::Right],
            color_mode: WaveformColorMode::Static,
            palette: Palette::Theme,
            contrast: 1.0,
            peak_history: false,
            looped: false,
            loop_cursor: 0,
            last_sequence: None,
            last_update: None,
        }
    }
}

impl Waveform {
    pub(super) const FACTORY_PRESETS: [&'static str; 3] =
        ["Stereo Scroll", "Triggered Mid", "Multiband Loop"];

    pub(super) fn factory_settings(index: usize) -> Option<WaveformSettings> {
        let mut module = Self::default();
        match index {
            0 => {}
            1 => {
                module.two_channels = false;
                module.channels[0] = WaveformChannel::Mid;
                module.speed_ms = 80.0;
                module.trigger = true;
                module.auto_scale = true;
                module.grid = true;
            }
            2 => {
                module.speed_ms = 500.0;
                module.looped = true;
                module.color_mode = WaveformColorMode::MultiBand;
                module.peak_history = true;
                module.grid = true;
            }
            _ => return None,
        }
        Some(module.settings_snapshot())
    }

    pub fn reset_settings(&mut self) {
        *self = Self::default();
    }

    pub fn clear(&mut self) {
        self.loop_cursor = 0;
        self.last_sequence = None;
        self.last_update = None;
    }

    pub fn controls(&mut self, ui: &mut egui::Ui) {
        settings_panel(ui, "Channels", |ui| {
            super::settings_group(ui, "Layout", |ui| {
                properties_combo("waveform-channel-count", 176.0, 2)
                    .selected_text(if self.two_channels {
                        "Two channels"
                    } else {
                        "One channel"
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.two_channels, false, "One channel")
                            .help_text("Use the full pane height for one selected signal.");
                        ui.selectable_value(&mut self.two_channels, true, "Two channels")
                            .help_text("Show any two selected signals in aligned lanes.");
                    })
                    .response
                    .help_text("Choose a single signal or an independent pair of waveform lanes.");
                waveform_channel_selector(
                    ui,
                    "waveform-channel-a",
                    "Channel 1",
                    &mut self.channels[0],
                );
                ui.add_enabled_ui(self.two_channels, |ui| {
                    waveform_channel_selector(
                        ui,
                        "waveform-channel-b",
                        "Channel 2",
                        &mut self.channels[1],
                    );
                });
            });
        });
        settings_panel(ui, "Time Window", |ui| {
            super::settings_group(ui, "Speed & motion", |ui| {
                ui.add(crate::parameter::Parameter::new(&mut self.speed_ms, 10.0..=500.0, 250.0).logarithmic(true).text("Across ms"))
                .help_text("Time required for audio to cross the pane. Smaller values move faster. This is independent of the shared FFT size, and long views preserve peaks when reduced to screen pixels.");
                properties_combo("waveform-motion", 176.0, 2)
                    .selected_text(if self.looped { "Loop" } else { "Scroll" })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.looped, false, "Scroll")
                            .help_text("Keep the newest audio at the right edge and scroll older audio left.");
                        ui.selectable_value(&mut self.looped, true, "Loop")
                            .help_text("Keep existing audio at fixed horizontal positions while a write head wraps across the pane.");
                    })
                    .response
                    .help_text("Choose a continuously scrolling trace or a static loop that is overwritten in place.");
            });
            super::settings_group(ui, "Trigger", |ui| {
                ui.add_enabled_ui(!self.looped, |ui| {
                    ui.checkbox(&mut self.trigger, "Stabilize repeating signals")
                    .help_text("Align the left edge to a recent threshold crossing in Channel 1 and keep both lanes aligned. Falls back to the latest window when no crossing is found.");
                });
                ui.add_enabled_ui(self.trigger && !self.looped, |ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.rising, true, "Rising").help_text("Trigger when the signal crosses the threshold upward.");
                    ui.selectable_value(&mut self.rising, false, "Falling").help_text("Trigger when the signal crosses the threshold downward.");
                });
                ui.add(crate::parameter::Parameter::new(&mut self.trigger_level, -1.0..=1.0, 0.0).text("Threshold"))
                    .help_text("Trigger threshold in original sample amplitude, before display scaling; zero is the centerline.");
            });
            });
        });
        settings_panel(ui, "Amplitude", |ui| {
            super::settings_group(ui, "Scaling", |ui| {
                ui.checkbox(&mut self.auto_scale, "Auto scale")
                .help_text("Fit the visible signal to the lane height. Both stereo lanes share one gain so their relative levels remain accurate. Does not change audio volume.");
                ui.add_enabled_ui(!self.auto_scale, |ui| {
                    ui.add(
                        crate::parameter::Parameter::new(&mut self.amplitude, 0.1..=10.0, 1.0)
                            .bounds(0.01..=100.0)
                            .logarithmic(true)
                            .text("Amplitude"),
                    )
                    .help_text(
                        "Manual vertical magnification. Retained while Auto scale is enabled.",
                    );
                });
            });
        });
        settings_panel(ui, "Appearance", |ui| {
            super::settings_group(ui, "Trace", |ui| {
                self.style.controls(ui, false);
                ui.add(
                    crate::parameter::Parameter::new(&mut self.thickness, 0.5..=4.0, 1.2)
                        .bounds(0.1..=20.0)
                        .text("Line px"),
                )
                .help_text("Outline thickness in screen pixels.");
                if self.style == TraceStyle::Filled {
                    ui.add(
                        crate::parameter::Parameter::new(&mut self.fill_opacity, 0.05..=1.0, 0.3)
                            .bounds(0.0..=1.0)
                            .text("Fill opacity"),
                    )
                    .help_text("Opacity of the area between the signal and centerline.");
                }
            });
            super::settings_group(ui, "Color", |ui| {
                properties_combo("waveform-color-mode", 176.0, 3)
                    .selected_text(self.color_mode.label())
                    .show_ui(ui, |ui| {
                        for mode in WaveformColorMode::ALL {
                            ui.selectable_value(&mut self.color_mode, mode, mode.label())
                                .help_text(match mode {
                                    WaveformColorMode::Static => "Use one theme color per waveform lane.",
                                    WaveformColorMode::MultiBand => "Color each part of the trace from its low-, mid-, and high-frequency energy.",
                                    WaveformColorMode::ColorMap => "Map the selected palette to instantaneous waveform level.",
                                });
                        }
                    })
                    .response
                    .help_text("Choose whether color identifies the lane, frequency balance, or signal level.");
                if self.color_mode == WaveformColorMode::ColorMap {
                    self.palette.controls(ui);
                    self.palette.contrast_controls(ui, &mut self.contrast);
                }
                ui.checkbox(&mut self.peak_history, "Multi-band peak history")
                    .help_text("Overlay the recent low-, mid-, and high-frequency peak envelopes behind the waveform.");
            });
            super::settings_group(ui, "Guides", |ui| {
                ui.checkbox(&mut self.centerline, "Centerline")
                    .help_text("Show the zero-amplitude reference in each lane.");
                ui.checkbox(&mut self.grid, "Grid")
                    .help_text("Show time divisions and half-scale amplitude guides.");
                ui.checkbox(&mut self.labels, "Axis & channel labels")
                    .help_text("Show channel names and the visible time range.");
            });
        });
    }

    fn sample(&self, frame: &AnalysisFrame, index: usize, lane: usize) -> f32 {
        let left = frame.waveform_left[index];
        let right = frame.waveform_right[index];
        match self.channels[lane] {
            WaveformChannel::Left => left,
            WaveformChannel::Right => right,
            WaveformChannel::Mid => (left + right) * 0.5,
            WaveformChannel::Side => (left - right) * 0.5,
        }
    }

    fn window(&self, frame: &AnalysisFrame) -> (std::ops::Range<usize>, bool) {
        let len = frame.waveform_left.len().min(frame.waveform_right.len());
        let count = ((self.speed_ms * frame.sample_rate.max(1) as f32 / 1000.0).ceil() as usize)
            .max(2)
            .min(len);
        let latest = len.saturating_sub(count);
        if self.trigger && !self.looped {
            for start in (1..=latest).rev() {
                let a = self.sample(frame, start - 1, 0);
                let b = self.sample(frame, start, 0);
                if crossing(a, b, self.trigger_level, self.rising) {
                    return (start..start + count, true);
                }
            }
        }
        (latest..len, false)
    }

    fn display_gain(&self, frame: &AnalysisFrame, window: std::ops::Range<usize>) -> f32 {
        let lanes = if self.two_channels { 2 } else { 1 };
        if self.auto_scale {
            let peak = (0..lanes)
                .flat_map(|lane| window.clone().map(move |index| (lane, index)))
                .map(|(lane, index)| self.sample(frame, index, lane).abs())
                .fold(0.0_f32, f32::max);
            // Bound amplification of silence/noise, and retain a little headroom.
            (0.95 / peak.max(0.001)).clamp(0.1, 100.0)
        } else {
            self.amplitude
        }
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
        let painter = ui.painter_at(rect);
        let plot = rect.shrink2(Vec2::new(10.0, if self.labels { 22.0 } else { 10.0 }));
        if !plot.is_positive() {
            return;
        }
        let (window, triggered) = self.window(frame);
        let gain = self.display_gain(frame, window.clone());
        self.update_loop_cursor(frame, window.len(), now, live);
        let lanes = if self.two_channels { 2 } else { 1 };
        for lane in 0..lanes {
            let lane_rect = Rect::from_min_max(
                Pos2::new(
                    plot.left(),
                    plot.top() + lane as f32 * plot.height() / lanes as f32,
                ),
                Pos2::new(
                    plot.right(),
                    plot.top() + (lane + 1) as f32 * plot.height() / lanes as f32,
                ),
            );
            let guide = Stroke::new(0.5, mix(theme.background, theme.muted, 0.5));
            if self.centerline {
                painter.line_segment([lane_rect.left_center(), lane_rect.right_center()], guide);
            }
            if self.grid {
                for t in [0.25, 0.5, 0.75] {
                    let x = plot.left() + t * plot.width();
                    painter.line_segment(
                        [
                            Pos2::new(x, lane_rect.top()),
                            Pos2::new(x, lane_rect.bottom()),
                        ],
                        guide,
                    );
                }
                for amplitude in [-0.5, 0.5] {
                    let y = lane_rect.center().y - amplitude * lane_rect.height() * 0.45;
                    painter.line_segment(
                        [Pos2::new(plot.left(), y), Pos2::new(plot.right(), y)],
                        guide,
                    );
                }
            }
            if live && window.len() >= 2 {
                let chronological: Vec<_> = window
                    .clone()
                    .map(|index| self.sample(frame, index, lane))
                    .collect();
                let chronological_bands = split_bands(&chronological, frame.sample_rate);
                let samples = if self.looped {
                    rotate_for_loop(&chronological, self.loop_cursor)
                } else {
                    chronological
                };
                let bands = if self.looped {
                    rotate_for_loop(&chronological_bands, self.loop_cursor)
                } else {
                    chronological_bands
                };
                if self.peak_history {
                    paint_peak_history(&painter, &bands, lane_rect, plot, gain, theme);
                }
                let reduced = envelope(&samples, plot.width().ceil() as usize);
                let points: Vec<_> = reduced
                    .iter()
                    .map(|&(i, sample)| {
                        (
                            Pos2::new(
                                plot.left() + i as f32 / (samples.len() - 1) as f32 * plot.width(),
                                lane_rect.center().y
                                    - (sample * gain).clamp(-1.0, 1.0) * lane_rect.height() * 0.45,
                            ),
                            self.trace_color(sample, bands[i], gain, lane, theme),
                        )
                    })
                    .collect();
                let ranges = trace_ranges(&reduced, self.looped.then_some(self.loop_cursor));
                for range in ranges {
                    let trace = &points[range];
                    if trace.len() < 2 {
                        continue;
                    }
                    if self.style == TraceStyle::Filled {
                        paint_colored_fill(
                            &painter,
                            trace,
                            lane_rect.center().y,
                            self.fill_opacity,
                        );
                    }
                    if self.color_mode == WaveformColorMode::Static {
                        painter.add(egui::Shape::line(
                            trace.iter().map(|(point, _)| *point).collect(),
                            Stroke::new(self.thickness, trace[0].1),
                        ));
                    } else {
                        for pair in trace.windows(2) {
                            painter.line_segment(
                                [pair[0].0, pair[1].0],
                                Stroke::new(self.thickness, mix(pair[0].1, pair[1].1, 0.5)),
                            );
                        }
                    }
                }
            }
            if self.labels {
                label(
                    &painter,
                    lane_rect.left_top(),
                    self.channels[lane].short_label(),
                    theme.muted,
                );
            }
        }
        if self.looped && live && window.len() >= 2 {
            let x =
                plot.left() + self.loop_cursor as f32 / window.len().max(1) as f32 * plot.width();
            painter.line_segment(
                [Pos2::new(x, plot.top()), Pos2::new(x, plot.bottom())],
                Stroke::new(1.0, theme.foreground.gamma_multiply(0.65)),
            );
        }
        if self.labels {
            let duration = window.len() as f32 / frame.sample_rate.max(1) as f32 * 1000.0;
            label(
                &painter,
                rect.left_bottom() + Vec2::new(10.0, -16.0),
                format!("−{duration:.1} ms"),
                theme.muted,
            );
            painter.text(
                rect.right_bottom() - Vec2::new(10.0, 5.0),
                egui::Align2::RIGHT_BOTTOM,
                if self.looped {
                    "loop"
                } else if triggered {
                    "triggered"
                } else if self.trigger {
                    "waiting for trigger · live"
                } else {
                    "scrolling"
                },
                egui::FontId::monospace(10.0),
                theme.muted,
            );
        }
    }

    fn update_loop_cursor(
        &mut self,
        frame: &AnalysisFrame,
        samples: usize,
        now: Instant,
        live: bool,
    ) {
        if !self.looped || samples == 0 {
            self.loop_cursor = 0;
            self.last_sequence = Some(frame.sequence);
            self.last_update = Some(now);
            return;
        }
        match self.last_sequence {
            None => {
                self.last_sequence = Some(frame.sequence);
                self.last_update = Some(now);
            }
            Some(sequence) if sequence == frame.sequence => {}
            Some(_) => {
                if live {
                    let elapsed = self.last_update.map_or(0.0, |previous| {
                        now.saturating_duration_since(previous).as_secs_f64()
                    });
                    let advanced = (elapsed * f64::from(frame.sample_rate.max(1))).round() as usize;
                    self.loop_cursor = (self.loop_cursor + advanced.max(1)) % samples;
                }
                self.last_sequence = Some(frame.sequence);
                self.last_update = Some(now);
            }
        }
    }

    fn trace_color(
        &self,
        sample: f32,
        bands: [f32; 3],
        gain: f32,
        lane: usize,
        theme: &AppTheme,
    ) -> Color32 {
        match self.color_mode {
            WaveformColorMode::Static => {
                if lane == 0 {
                    theme.accent
                } else {
                    theme.accent_alt
                }
            }
            WaveformColorMode::MultiBand => multiband_color(bands, theme),
            WaveformColorMode::ColorMap => self.palette.color_with_contrast(
                (sample * gain).abs().clamp(0.0, 1.0),
                self.contrast,
                theme,
            ),
        }
    }
}

fn waveform_channel_selector(
    ui: &mut egui::Ui,
    id: &str,
    label_text: &str,
    channel: &mut WaveformChannel,
) {
    ui.label(label_text);
    properties_combo(id, 176.0, WaveformChannel::ALL.len())
        .selected_text(channel.label())
        .show_ui(ui, |ui| {
            for candidate in WaveformChannel::ALL {
                ui.selectable_value(channel, candidate, candidate.label())
                    .help_text(match candidate {
                        WaveformChannel::Left => "Original left input channel.",
                        WaveformChannel::Right => "Original right input channel.",
                        WaveformChannel::Mid => "Mono-compatible center: (Left + Right) / 2.",
                        WaveformChannel::Side => "Stereo difference: (Left − Right) / 2.",
                    });
            }
        })
        .response
        .help_text("Select the signal shown in this waveform lane.");
}

fn rotate_for_loop<T: Clone>(values: &[T], cursor: usize) -> Vec<T> {
    if values.is_empty() {
        return Vec::new();
    }
    let cursor = cursor % values.len();
    let split = values.len() - cursor;
    values[split..]
        .iter()
        .chain(&values[..split])
        .cloned()
        .collect()
}

fn trace_ranges(
    points: &[(usize, f32)],
    loop_cursor: Option<usize>,
) -> [std::ops::Range<usize>; 2] {
    let Some(cursor) = loop_cursor.filter(|cursor| *cursor > 0) else {
        return [0..points.len(), points.len()..points.len()];
    };
    let split = points.partition_point(|(index, _)| *index < cursor);
    [0..split, split..points.len()]
}

/// Split a waveform into low (<200 Hz), mid (roughly 200 Hz–2 kHz), and high
/// (>2 kHz) components, then apply a short release envelope so color does not
/// flicker at the audio sample rate. These bands are display-only and never
/// alter the captured audio or shared FFT analysis.
fn split_bands(samples: &[f32], sample_rate: u32) -> Vec<[f32; 3]> {
    let rate = sample_rate.max(1) as f32;
    let coefficient = |frequency: f32| 1.0 - (-std::f32::consts::TAU * frequency / rate).exp();
    let low_coefficient = coefficient(200.0);
    let mid_coefficient = coefficient(2_000.0);
    let release = (-1.0 / (rate * 0.012)).exp();
    let mut low_pass = 0.0_f32;
    let mut mid_pass = 0.0_f32;
    let mut envelope = [0.0_f32; 3];
    samples
        .iter()
        .map(|&sample| {
            low_pass += (sample - low_pass) * low_coefficient;
            mid_pass += (sample - mid_pass) * mid_coefficient;
            let components = [low_pass, mid_pass - low_pass, sample - mid_pass];
            for band in 0..3 {
                envelope[band] = components[band].abs().max(envelope[band] * release);
            }
            envelope
        })
        .collect()
}

fn multiband_colors(theme: &AppTheme) -> [Color32; 3] {
    [
        Color32::from_rgb(232, 91, 71),
        theme.accent,
        Color32::from_rgb(72, 151, 232),
    ]
}

fn multiband_color(bands: [f32; 3], theme: &AppTheme) -> Color32 {
    let colors = multiband_colors(theme);
    let sum = bands.iter().sum::<f32>();
    if sum <= 1.0e-7 {
        return theme.muted;
    }
    let weighted = |components: [u8; 3]| {
        bands
            .iter()
            .zip(components)
            .map(|(&weight, component)| weight * component as f32)
            .sum::<f32>()
            / sum
    };
    Color32::from_rgb(
        weighted(colors.map(|color| color.r())).round() as u8,
        weighted(colors.map(|color| color.g())).round() as u8,
        weighted(colors.map(|color| color.b())).round() as u8,
    )
}

fn paint_peak_history(
    painter: &egui::Painter,
    bands: &[[f32; 3]],
    lane: Rect,
    plot: Rect,
    gain: f32,
    theme: &AppTheme,
) {
    if bands.len() < 2 {
        return;
    }
    let pixels = plot.width().ceil().max(1.0) as usize;
    let stride = bands.len().div_ceil(pixels).max(1);
    for band in (0..3).rev() {
        let points: Vec<_> = bands
            .chunks(stride)
            .enumerate()
            .map(|(chunk, values)| {
                let index = (chunk * stride + values.len() / 2).min(bands.len() - 1);
                let peak = values
                    .iter()
                    .map(|levels| levels[band])
                    .fold(0.0_f32, f32::max);
                Pos2::new(
                    plot.left() + index as f32 / (bands.len() - 1) as f32 * plot.width(),
                    lane.center().y - (peak * gain).clamp(0.0, 1.0) * lane.height() * 0.45,
                )
            })
            .collect();
        super::fill_trace(
            painter,
            &points,
            lane.center().y,
            multiband_colors(theme)[band].gamma_multiply(0.16),
        );
    }
}

fn paint_colored_fill(
    painter: &egui::Painter,
    points: &[(Pos2, Color32)],
    baseline: f32,
    opacity: f32,
) {
    let mut mesh = egui::Mesh::default();
    for pair in points.windows(2) {
        let (a, color_a) = pair[0];
        let (b, color_b) = pair[1];
        let color_a = color_a.gamma_multiply(opacity);
        let color_b = color_b.gamma_multiply(opacity);
        let base = mesh.vertices.len() as u32;
        if (a.y - baseline) * (b.y - baseline) < 0.0 {
            let t = (baseline - a.y) / (b.y - a.y);
            let crossing = Pos2::new(egui::lerp(a.x..=b.x, t), baseline);
            let crossing_color = mix(color_a, color_b, t);
            for (point, color) in [
                (a, color_a),
                (crossing, crossing_color),
                (Pos2::new(a.x, baseline), color_a),
                (b, color_b),
                (Pos2::new(b.x, baseline), color_b),
            ] {
                mesh.colored_vertex(point, color);
            }
            mesh.add_triangle(base, base + 1, base + 2);
            mesh.add_triangle(base + 1, base + 3, base + 4);
        } else {
            for (point, color) in [
                (a, color_a),
                (b, color_b),
                (Pos2::new(b.x, baseline), color_b),
                (Pos2::new(a.x, baseline), color_a),
            ] {
                mesh.colored_vertex(point, color);
            }
            mesh.add_triangle(base, base + 1, base + 2);
            mesh.add_triangle(base, base + 2, base + 3);
        }
    }
    painter.add(egui::Shape::mesh(mesh));
}

fn crossing(a: f32, b: f32, level: f32, rising: bool) -> bool {
    if rising {
        a < level && b >= level
    } else {
        a > level && b <= level
    }
}

/// Keep each pixel bucket's extrema in time order instead of skipping transients.
fn envelope(samples: &[f32], pixels: usize) -> Vec<(usize, f32)> {
    let stride = samples.len().div_ceil(pixels.max(1)).max(1);
    let mut result = Vec::with_capacity(pixels.min(samples.len()) * 2 + 2);
    for (chunk_index, chunk) in samples.chunks(stride).enumerate() {
        let start = chunk_index * stride;
        let mut low = 0;
        let mut high = 0;
        for i in 1..chunk.len() {
            if chunk[i] < chunk[low] {
                low = i;
            }
            if chunk[i] > chunk[high] {
                high = i;
            }
        }
        for i in if low <= high {
            [low, high]
        } else {
            [high, low]
        } {
            if result.last().is_none_or(|&(index, _)| index != start + i) {
                result.push((start + i, chunk[i]));
            }
        }
    }
    if let Some(&last) = samples.last()
        && result.last().is_none_or(|&(i, _)| i + 1 != samples.len())
    {
        result.push((samples.len() - 1, last));
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_settings_includes_channel_color_and_motion_but_not_runtime_cursor() {
        let mut source = Waveform {
            speed_ms: 125.0,
            amplitude: 2.0,
            channels: [WaveformChannel::Mid, WaveformChannel::Side],
            color_mode: WaveformColorMode::MultiBand,
            peak_history: true,
            looped: true,
            loop_cursor: 41,
            ..Waveform::default()
        };
        let settings = source.settings_snapshot();
        source.speed_ms = 10.0;
        assert_eq!(source.speed_ms, 10.0);
        let mut target = Waveform {
            loop_cursor: 7,
            ..Waveform::default()
        };
        target.apply_settings(&settings);
        assert_eq!(target.speed_ms, 125.0);
        assert_eq!(target.amplitude, 2.0);
        assert_eq!(
            target.channels,
            [WaveformChannel::Mid, WaveformChannel::Side]
        );
        assert_eq!(target.color_mode, WaveformColorMode::MultiBand);
        assert!(target.peak_history && target.looped);
        assert_eq!(target.loop_cursor, 7);
    }

    #[test]
    fn auto_scale_uses_one_bounded_gain_and_keeps_manual_amplitude() {
        let mut waveform = Waveform {
            amplitude: 3.0,
            auto_scale: true,
            ..Waveform::default()
        };
        let mut frame = AnalysisFrame {
            waveform_left: vec![0.1; 512],
            waveform_right: vec![0.2; 512],
            ..AnalysisFrame::default()
        };
        assert!((waveform.display_gain(&frame, 0..512) - 4.75).abs() < 0.001);
        waveform.auto_scale = false;
        assert_eq!(waveform.display_gain(&frame, 0..512), 3.0);
        waveform.auto_scale = true;
        frame.waveform_left.fill(0.0);
        frame.waveform_right.fill(0.0);
        assert_eq!(waveform.display_gain(&frame, 0..512), 100.0);
    }

    #[test]
    fn line_and_filled_styles_render_finite_geometry_with_all_guides_hidden() {
        let context = egui::Context::default();
        let frame = AnalysisFrame {
            waveform_left: (0..24_000).map(|i| (i as f32 * 0.2).sin()).collect(),
            waveform_right: vec![0.0; 24_000],
            ..AnalysisFrame::default()
        };
        for style in [TraceStyle::Line, TraceStyle::Filled] {
            let mut waveform = Waveform {
                style,
                speed_ms: 250.0,
                labels: false,
                centerline: false,
                grid: false,
                ..Waveform::default()
            };
            let mut output = context.run_ui(egui::RawInput::default(), |ui| {
                waveform.draw(
                    ui,
                    Rect::from_min_size(Pos2::ZERO, egui::vec2(200.0, 120.0)),
                    &frame,
                    &AppTheme::default(),
                    Instant::now(),
                    true,
                );
            });
            output.textures_delta.clear();
            assert!(
                !output
                    .shapes
                    .iter()
                    .any(|s| matches!(&s.shape, egui::Shape::Text(_)))
            );
            let mut count = 0;
            for primitive in context.tessellate(output.shapes, output.pixels_per_point) {
                if let egui::epaint::Primitive::Mesh(mesh) = primitive.primitive {
                    count += mesh.vertices.len();
                    assert!(mesh.vertices.iter().all(|v| v.pos.is_finite()));
                }
            }
            assert!(count > 0 && count < 20_000);
        }
    }

    #[test]
    fn every_color_mode_and_peak_history_render_finite_colored_geometry() {
        let context = egui::Context::default();
        let frame = AnalysisFrame {
            sequence: 2,
            waveform_left: (0..24_000)
                .map(|sample| {
                    let time = sample as f32 / 48_000.0;
                    (std::f32::consts::TAU * 90.0 * time).sin() * 0.5
                        + (std::f32::consts::TAU * 4_000.0 * time).sin() * 0.2
                })
                .collect(),
            waveform_right: (0..24_000)
                .map(|sample| (sample as f32 * 0.17).sin() * 0.4)
                .collect(),
            ..AnalysisFrame::default()
        };
        for color_mode in WaveformColorMode::ALL {
            let mut waveform = Waveform {
                color_mode,
                palette: Palette::Heatmap,
                peak_history: true,
                looped: true,
                labels: false,
                ..Waveform::default()
            };
            let mut output = context.run_ui(egui::RawInput::default(), |ui| {
                waveform.draw(
                    ui,
                    Rect::from_min_size(Pos2::ZERO, egui::vec2(320.0, 180.0)),
                    &frame,
                    &AppTheme::default(),
                    Instant::now(),
                    true,
                );
            });
            output.textures_delta.clear();
            let meshes: Vec<_> = context
                .tessellate(output.shapes, output.pixels_per_point)
                .into_iter()
                .filter_map(|primitive| match primitive.primitive {
                    egui::epaint::Primitive::Mesh(mesh) => Some(mesh),
                    _ => None,
                })
                .collect();
            assert!(!meshes.is_empty());
            assert!(
                meshes
                    .iter()
                    .flat_map(|mesh| &mesh.vertices)
                    .all(|vertex| vertex.pos.is_finite())
            );
            let colors: std::collections::HashSet<_> = meshes
                .iter()
                .flat_map(|mesh| &mesh.vertices)
                .map(|vertex| vertex.color.to_array())
                .collect();
            assert!(
                colors.len() >= 3,
                "{color_mode:?} should render band overlays"
            );
        }
    }

    #[test]
    fn time_window_does_not_depend_on_fft_and_trigger_keeps_stereo_aligned() {
        let mut frame = AnalysisFrame {
            sample_rate: 48_000,
            fft_size: 512,
            waveform_left: (0..24_000)
                .map(|i| ((i as f32 - 0.5) * std::f32::consts::TAU / 48.0).sin())
                .collect(),
            waveform_right: vec![0.0; 24_000],
            ..AnalysisFrame::default()
        };
        let mut waveform = Waveform {
            speed_ms: 250.0,
            ..Waveform::default()
        };
        let (range, triggered) = waveform.window(&frame);
        assert_eq!(range, 12_000..24_000);
        assert!(!triggered);
        frame.fft_size = 16_384;
        assert_eq!(waveform.window(&frame).0, range);
        waveform.trigger = true;
        let (range, triggered) = waveform.window(&frame);
        assert!(triggered);
        assert_eq!(range.len(), 12_000);
        assert!(crossing(
            frame.waveform_left[range.start - 1],
            frame.waveform_left[range.start],
            0.0,
            true
        ));
        waveform.rising = false;
        let (range, triggered) = waveform.window(&frame);
        assert!(triggered);
        assert!(crossing(
            frame.waveform_left[range.start - 1],
            frame.waveform_left[range.start],
            0.0,
            false
        ));
    }

    #[test]
    fn trigger_falls_back_for_silence_and_empty_input() {
        let mut frame = AnalysisFrame::default();
        let waveform = Waveform {
            trigger: true,
            ..Waveform::default()
        };
        assert!(!waveform.window(&frame).1);
        frame.waveform_left.clear();
        assert_eq!(waveform.window(&frame), (0..0, false));
    }

    #[test]
    fn downsampling_preserves_both_transients_and_time_order() {
        let mut samples = vec![0.0; 1000];
        samples[417] = 1.0;
        samples[419] = -1.0;
        let points = envelope(&samples, 100);
        assert!(points.contains(&(417, 1.0)));
        assert!(points.contains(&(419, -1.0)));
        assert!(points.windows(2).all(|p| p[0].0 < p[1].0));
        assert!(points.len() <= 202);
    }

    #[test]
    fn reset_restores_waveform_settings_and_motion_state() {
        let mut waveform = Waveform {
            two_channels: false,
            channels: [WaveformChannel::Side, WaveformChannel::Mid],
            speed_ms: 200.0,
            loop_cursor: 19,
            last_sequence: Some(4),
            ..Waveform::default()
        };
        waveform.reset_settings();
        assert!(waveform.two_channels);
        assert_eq!(
            waveform.channels,
            [WaveformChannel::Left, WaveformChannel::Right]
        );
        assert_eq!(waveform.speed_ms, 250.0);
        assert_eq!(waveform.loop_cursor, 0);
        assert_eq!(waveform.last_sequence, None);
    }

    #[test]
    fn channels_return_left_right_mid_and_side_with_standard_scaling() {
        let frame = AnalysisFrame {
            waveform_left: vec![0.8],
            waveform_right: vec![0.2],
            ..AnalysisFrame::default()
        };
        let mut waveform = Waveform {
            channels: [WaveformChannel::Left, WaveformChannel::Right],
            ..Waveform::default()
        };
        assert_eq!(waveform.sample(&frame, 0, 0), 0.8);
        assert_eq!(waveform.sample(&frame, 0, 1), 0.2);
        waveform.channels = [WaveformChannel::Mid, WaveformChannel::Side];
        assert!((waveform.sample(&frame, 0, 0) - 0.5).abs() < f32::EPSILON);
        assert!((waveform.sample(&frame, 0, 1) - 0.3).abs() < f32::EPSILON);
    }

    #[test]
    fn multiband_split_and_colors_distinguish_low_and_high_tones() {
        let rate = 48_000;
        let tone = |frequency: f32| {
            (0..4_800)
                .map(|sample| {
                    (std::f32::consts::TAU * frequency * sample as f32 / rate as f32).sin()
                })
                .collect::<Vec<_>>()
        };
        let low = split_bands(&tone(80.0), rate);
        let high = split_bands(&tone(8_000.0), rate);
        let low = low.last().unwrap();
        let high = high.last().unwrap();
        assert!(low[0] > low[2]);
        assert!(high[2] > high[0]);
        assert_ne!(
            multiband_color(*low, &AppTheme::default()),
            multiband_color(*high, &AppTheme::default())
        );
    }

    #[test]
    fn loop_rotation_holds_existing_samples_in_place_and_splits_at_write_head() {
        let before = vec![0, 1, 2, 3, 4, 5];
        let after = vec![2, 3, 4, 5, 6, 7];
        assert_eq!(rotate_for_loop(&before, 0), before);
        assert_eq!(rotate_for_loop(&after, 2), vec![6, 7, 2, 3, 4, 5]);
        let reduced: Vec<_> = (0..6).map(|index| (index, 0.0)).collect();
        assert_eq!(trace_ranges(&reduced, Some(2)), [0..2, 2..6]);
    }

    #[test]
    fn loop_cursor_advances_only_for_new_live_frames_and_clear_resets_it() {
        let start = Instant::now();
        let mut waveform = Waveform {
            looped: true,
            ..Waveform::default()
        };
        let mut frame = AnalysisFrame {
            sequence: 1,
            sample_rate: 1_000,
            ..AnalysisFrame::default()
        };
        waveform.update_loop_cursor(&frame, 500, start, true);
        assert_eq!(waveform.loop_cursor, 0);
        waveform.update_loop_cursor(
            &frame,
            500,
            start + std::time::Duration::from_millis(10),
            true,
        );
        assert_eq!(waveform.loop_cursor, 0);
        frame.sequence = 2;
        waveform.update_loop_cursor(
            &frame,
            500,
            start + std::time::Duration::from_millis(20),
            true,
        );
        assert_eq!(waveform.loop_cursor, 20);
        frame.sequence = 3;
        waveform.update_loop_cursor(
            &frame,
            500,
            start + std::time::Duration::from_millis(40),
            false,
        );
        assert_eq!(waveform.loop_cursor, 20);
        waveform.clear();
        assert_eq!(waveform.loop_cursor, 0);
        assert_eq!(waveform.last_sequence, None);
    }
}
