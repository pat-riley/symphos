use eframe::egui::{self, Pos2, Rect, Stroke, Vec2};

use super::{TraceStyle, label, mix, settings_panel};
use crate::help::HoverHelp;
use crate::{
    analysis::{AnalysisFrame, ChannelMode},
    theme::AppTheme,
};

pub struct Waveform {
    time_ms: f32,
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
    stereo: bool,
    channel: ChannelMode,
}

module_settings!(Waveform, WaveformSettings, {
time_ms: f32 => time_ms,
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
});

impl Default for Waveform {
    fn default() -> Self {
        Self {
            time_ms: 40.0,
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
            stereo: true,
            channel: ChannelMode::StereoMix,
        }
    }
}

impl Waveform {
    pub fn reset_settings(&mut self) {
        *self = Self {
            stereo: self.stereo,
            channel: self.channel,
            ..Self::default()
        };
    }

    pub fn controls(&mut self, ui: &mut egui::Ui) {
        settings_panel(ui, "Time Window", |ui| {
            ui.add(crate::parameter::Parameter::new(&mut self.time_ms, 1.0..=250.0, 40.0).logarithmic(true).text("Window ms"))
                .help_text("Visible time window, independent of the shared FFT size. Long views preserve peaks when reduced to screen pixels.");
            ui.separator();
            ui.strong("Trigger");
            ui.checkbox(&mut self.trigger, "Stabilize repeating signals")
                .help_text("Align the left edge to a recent threshold crossing. Stereo uses the left channel and keeps both channels aligned. Falls back to the latest window when no crossing is found.");
            ui.add_enabled_ui(self.trigger, |ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.rising, true, "Rising").help_text("Trigger when the signal crosses the threshold upward.");
                    ui.selectable_value(&mut self.rising, false, "Falling").help_text("Trigger when the signal crosses the threshold downward.");
                });
                ui.add(crate::parameter::Parameter::new(&mut self.trigger_level, -1.0..=1.0, 0.0).text("Threshold"))
                    .help_text("Trigger threshold in original sample amplitude, before display scaling; zero is the centerline.");
            });
        });
        settings_panel(ui, "Amplitude", |ui| {
            ui.checkbox(&mut self.auto_scale, "Auto scale")
                .help_text("Fit the visible signal to the lane height. Both stereo lanes share one gain so their relative levels remain accurate. Does not change audio volume.");
            ui.add_enabled_ui(!self.auto_scale, |ui| {
                ui.add(
                    crate::parameter::Parameter::new(&mut self.amplitude, 0.1..=10.0, 1.0)
                        .bounds(0.01..=100.0)
                        .logarithmic(true)
                        .text("Amplitude"),
                )
                .help_text("Manual vertical magnification. Retained while Auto scale is enabled.");
            });
        });
        settings_panel(ui, "Appearance", |ui| {
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
            ui.separator();
            ui.strong("Guides");
            ui.checkbox(&mut self.centerline, "Centerline")
                .help_text("Show the zero-amplitude reference in each lane.");
            ui.checkbox(&mut self.grid, "Grid")
                .help_text("Show time divisions and half-scale amplitude guides.");
            ui.checkbox(&mut self.labels, "Axis & channel labels")
                .help_text("Show channel names and the visible time range.");
        });
    }

    pub fn set_channel_view(&mut self, stereo: bool, channel: ChannelMode) {
        self.stereo = stereo;
        self.channel = channel;
    }

    fn sample(&self, frame: &AnalysisFrame, index: usize, lane: usize) -> f32 {
        let left = frame.waveform_left[index];
        let right = frame.waveform_right[index];
        if self.stereo {
            if lane == 0 { left } else { right }
        } else {
            match self.channel {
                ChannelMode::Left => left,
                ChannelMode::Right => right,
                ChannelMode::StereoMix => (left + right) * 0.5,
            }
        }
    }

    fn window(&self, frame: &AnalysisFrame) -> (std::ops::Range<usize>, bool) {
        let len = frame.waveform_left.len().min(frame.waveform_right.len());
        let count = ((self.time_ms * frame.sample_rate.max(1) as f32 / 1000.0).ceil() as usize)
            .max(2)
            .min(len);
        let latest = len.saturating_sub(count);
        if self.trigger {
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
        let lanes = if self.stereo { 2 } else { 1 };
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
        live: bool,
    ) {
        let painter = ui.painter_at(rect);
        let plot = rect.shrink2(Vec2::new(10.0, if self.labels { 22.0 } else { 10.0 }));
        if !plot.is_positive() {
            return;
        }
        let (window, triggered) = self.window(frame);
        let gain = self.display_gain(frame, window.clone());
        let lanes = if self.stereo { 2 } else { 1 };
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
                let samples: Vec<_> = window
                    .clone()
                    .map(|index| self.sample(frame, index, lane))
                    .collect();
                let points: Vec<_> = envelope(&samples, plot.width().ceil() as usize)
                    .into_iter()
                    .map(|(i, sample)| {
                        Pos2::new(
                            plot.left() + i as f32 / (samples.len() - 1) as f32 * plot.width(),
                            lane_rect.center().y
                                - (sample * gain).clamp(-1.0, 1.0) * lane_rect.height() * 0.45,
                        )
                    })
                    .collect();
                let color = if lane == 0 {
                    theme.accent
                } else {
                    theme.accent_alt
                };
                if self.style == TraceStyle::Filled {
                    super::fill_trace(
                        &painter,
                        &points,
                        lane_rect.center().y,
                        color.gamma_multiply(self.fill_opacity),
                    );
                }
                painter.add(egui::Shape::line(
                    points,
                    Stroke::new(self.thickness, color),
                ));
            }
            if self.labels {
                label(
                    &painter,
                    lane_rect.left_top(),
                    if self.stereo {
                        if lane == 0 { "L" } else { "R" }
                    } else {
                        self.channel.label()
                    },
                    theme.muted,
                );
            }
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
                if triggered {
                    "triggered"
                } else if self.trigger {
                    "waiting for trigger · live"
                } else {
                    "now"
                },
                egui::FontId::monospace(10.0),
                theme.muted,
            );
        }
    }
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
    fn copy_settings_preserves_target_global_channel_and_is_an_independent_snapshot() {
        let mut source = Waveform {
            time_ms: 125.0,
            amplitude: 2.0,
            ..Waveform::default()
        };
        let settings = source.settings_snapshot();
        source.time_ms = 10.0;
        assert_eq!(source.time_ms, 10.0);
        let mut target = Waveform {
            stereo: false,
            channel: ChannelMode::Right,
            ..Waveform::default()
        };
        target.apply_settings(&settings);
        assert_eq!(target.time_ms, 125.0);
        assert_eq!(target.amplitude, 2.0);
        assert!(!target.stereo);
        assert_eq!(target.channel, ChannelMode::Right);
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
                time_ms: 250.0,
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
            time_ms: 250.0,
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
    fn reset_preserves_global_channel_selection() {
        let mut waveform = Waveform {
            stereo: false,
            channel: ChannelMode::Right,
            time_ms: 200.0,
            ..Waveform::default()
        };
        waveform.reset_settings();
        assert!(!waveform.stereo);
        assert_eq!(waveform.channel, ChannelMode::Right);
        assert_eq!(waveform.time_ms, 40.0);
    }
}
