use eframe::egui::{self, Pos2, Rect, Stroke, Vec2};

use super::{label, mix};
use crate::{
    analysis::{AnalysisFrame, ChannelMode},
    theme::AppTheme,
};

pub struct Waveform {
    time_ms: f32,
    amplitude: f32,
    stereo: bool,
    channel: ChannelMode,
}

impl Default for Waveform {
    fn default() -> Self {
        Self {
            time_ms: 40.0,
            amplitude: 1.0,
            stereo: true,
            channel: ChannelMode::StereoMix,
        }
    }
}

impl Waveform {
    pub fn controls(&mut self, ui: &mut egui::Ui, frame: &AnalysisFrame) {
        let max_ms = frame.fft_size as f32 / frame.sample_rate.max(1) as f32 * 1000.0;
        self.time_ms = self.time_ms.min(max_ms);
        ui.add(egui::Slider::new(&mut self.time_ms, 1.0..=max_ms.max(1.0)).text("Window ms"));
        ui.add(
            egui::Slider::new(&mut self.amplitude, 0.1..=10.0)
                .logarithmic(true)
                .text("Amplitude"),
        );
        ui.checkbox(&mut self.stereo, "Separate stereo channels");
        if !self.stereo {
            egui::ComboBox::from_id_salt("wave-channel")
                .selected_text(self.channel.label())
                .show_ui(ui, |ui| {
                    for mode in ChannelMode::ALL {
                        ui.selectable_value(&mut self.channel, mode, mode.label());
                    }
                });
        }
        ui.small("Time window is limited by the shared FFT capture window.");
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
        let plot = rect.shrink2(Vec2::new(10.0, 22.0));
        let max_ms = frame.fft_size as f32 / frame.sample_rate.max(1) as f32 * 1000.0;
        let time_ms = self.time_ms.min(max_ms);
        let len = frame.waveform_left.len().min(frame.waveform_right.len());
        let count = ((len as f32 * time_ms / max_ms).ceil() as usize).clamp(2, len.max(2));
        for lane in 0..if self.stereo { 2 } else { 1 } {
            let lane_rect = if self.stereo {
                Rect::from_min_max(
                    Pos2::new(plot.left(), plot.top() + lane as f32 * plot.height() / 2.0),
                    Pos2::new(
                        plot.right(),
                        plot.top() + (lane + 1) as f32 * plot.height() / 2.0,
                    ),
                )
            } else {
                plot
            };
            painter.line_segment(
                [
                    Pos2::new(plot.left(), lane_rect.center().y),
                    Pos2::new(plot.right(), lane_rect.center().y),
                ],
                Stroke::new(0.5, mix(theme.background, theme.muted, 0.5)),
            );
            if live && len >= 2 {
                let points = (len.saturating_sub(count)..len)
                    .enumerate()
                    .map(|(i, index)| {
                        let left = frame.waveform_left[index];
                        let right = frame.waveform_right[index];
                        let sample = if self.stereo {
                            if lane == 0 { left } else { right }
                        } else {
                            match self.channel {
                                ChannelMode::Left => left,
                                ChannelMode::Right => right,
                                ChannelMode::StereoMix => (left + right) / 2.0,
                            }
                        };
                        Pos2::new(
                            plot.left() + i as f32 / (count - 1) as f32 * plot.width(),
                            lane_rect.center().y
                                - (sample * self.amplitude).clamp(-1.0, 1.0)
                                    * lane_rect.height()
                                    * 0.45,
                        )
                    })
                    .collect();
                painter.add(egui::Shape::line(
                    points,
                    Stroke::new(
                        1.2,
                        if lane == 0 {
                            theme.accent
                        } else {
                            theme.accent_alt
                        },
                    ),
                ));
            }
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
        label(
            &painter,
            rect.left_bottom() + Vec2::new(10.0, -16.0),
            format!("−{time_ms:.1} ms"),
            theme.muted,
        );
        painter.text(
            rect.right_bottom() - Vec2::new(10.0, 5.0),
            egui::Align2::RIGHT_BOTTOM,
            "now",
            egui::FontId::monospace(10.0),
            theme.muted,
        );
    }
}
