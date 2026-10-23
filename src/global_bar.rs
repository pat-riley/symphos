use crate::{
    analysis::{AnalysisFrame, ChannelMode},
    audio::AudioStatus,
    help::{self, HoverHelp},
    theme::AppTheme,
};
use eframe::egui::{self, Align, FontId, Layout, Pos2, Rect, RichText, Sense, Stroke, Vec2};

pub struct GlobalBar {
    pub stereo: bool,
    pub channel: ChannelMode,
    held_peaks: [f32; 3],
    clip_latched: [bool; 3],
    hold_epoch: Option<u64>,
}

impl Default for GlobalBar {
    fn default() -> Self {
        Self {
            stereo: true,
            channel: ChannelMode::StereoMix,
            held_peaks: [0.0; 3],
            clip_latched: [false; 3],
            hold_epoch: None,
        }
    }
}

pub struct BarInfo<'a> {
    pub frame: &'a AnalysisFrame,
    pub status: &'a AudioStatus,
    pub theme: &'a AppTheme,
    pub fps: f32,
    pub source_label: &'a str,
}

impl GlobalBar {
    pub(crate) fn reset_meter_hold(&mut self) {
        self.held_peaks.fill(0.0);
        self.clip_latched.fill(false);
    }

    fn update_meter_hold(&mut self, info: &BarInfo<'_>) {
        if !matches!(info.status, AudioStatus::Streaming) {
            self.reset_meter_hold();
            self.hold_epoch = None;
            return;
        }
        if self.hold_epoch != Some(info.frame.capture_epoch) {
            self.reset_meter_hold();
            self.hold_epoch = Some(info.frame.capture_epoch);
        }
        for (index, peak) in [info.frame.peak[0], info.frame.peak[1], info.frame.mix_peak]
            .into_iter()
            .enumerate()
        {
            self.held_peaks[index] = self.held_peaks[index].max(peak);
            self.clip_latched[index] |= peak >= 1.0;
        }
    }

    /// Help, compact performance controls, and identified master capture meters.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        help_open: &mut bool,
        target_fps: &mut u32,
        info: BarInfo<'_>,
    ) {
        self.update_meter_hold(&info);
        ui.spacing_mut().item_spacing = Vec2::new(7.0, 4.0);
        ui.spacing_mut().button_padding = Vec2::new(6.0, 4.0);
        ui.spacing_mut().interact_size.y = 26.0;
        ui.style_mut().override_font_id = Some(FontId::proportional(12.0));
        let bounds = ui.available_rect_before_wrap();
        let compact = bounds.width() < 900.0;
        let help_rect = Rect::from_center_size(
            Pos2::new(bounds.left() + 13.0, bounds.center().y),
            Vec2::splat(26.0),
        );
        help::toggle(
            &mut ui.new_child(
                egui::UiBuilder::new()
                    .id_salt("help-toggle")
                    .max_rect(help_rect),
            ),
            help_open,
        );

        let meter_width = if bounds.width() < 440.0 {
            120.0
        } else if compact {
            160.0
        } else {
            220.0
        };
        let audio_bounds = Rect::from_center_size(
            Pos2::new(
                bounds.right() - (meter_width + 82.0) * 0.5,
                bounds.center().y,
            ),
            Vec2::new(meter_width + 82.0, 30.0),
        );
        let stats_bounds = Rect::from_min_max(
            Pos2::new(help_rect.right() + 10.0, bounds.center().y - 15.0),
            Pos2::new(audio_bounds.left() - 12.0, bounds.center().y + 15.0),
        );
        let mut stats = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("performance")
                .max_rect(stats_bounds),
        );
        stats.with_layout(Layout::left_to_right(Align::Center), |ui| {
            let _response = fps_menu(ui, target_fps, &info);
            #[cfg(test)]
            ui.data_mut(|d| d.insert_temp(egui::Id::new("test-footer-fps"), _response.rect));
            if stats_bounds.width() >= 150.0 {
                ui.label(
                    RichText::new(format!("{:.1} kHz", info.frame.sample_rate as f32 / 1000.0))
                        .size(11.0)
                        .color(info.theme.muted),
                )
                .help_text("Sample rate of the master audio capture.");
            }
        });
        let mut audio = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("capture-meters")
                .max_rect(audio_bounds),
        );
        audio.with_layout(Layout::right_to_left(Align::Center), |ui| {
            self.meters(ui, &info, meter_width);
            ui.label(RichText::new(info.source_label).size(11.0).color(info.theme.muted))
                .help_text("Master capture levels before Signals inserts and track gain. Each visualizer can select its driving signal independently.");
        });
    }

    fn meters(&self, ui: &mut egui::Ui, info: &BarInfo<'_>, width: f32) {
        let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 30.0), Sense::hover());
        let live = matches!(info.status, AudioStatus::Streaming);
        let indicator = match info.status {
            AudioStatus::Streaming => info.theme.accent,
            AudioStatus::Error(_) => info.theme.error,
            AudioStatus::Connecting | AudioStatus::Paused => info.theme.warning,
            AudioStatus::Idle => info.theme.muted,
        };
        let painter = ui.painter_at(rect);
        painter.circle_filled(rect.left_center() + Vec2::new(4.0, 0.0), 3.0, indicator);
        let levels = if self.stereo {
            [
                ("L", info.frame.rms[0], info.frame.peak[0], 0),
                ("R", info.frame.rms[1], info.frame.peak[1], 1),
            ]
        } else {
            let (label, rms, peak, hold_index) = match self.channel {
                ChannelMode::StereoMix => ("M", info.frame.mix_rms, info.frame.mix_peak, 2),
                ChannelMode::Left => ("L", info.frame.rms[0], info.frame.peak[0], 0),
                ChannelMode::Right => ("R", info.frame.rms[1], info.frame.peak[1], 1),
            };
            [
                (label, rms, peak, hold_index),
                (label, rms, peak, hold_index),
            ]
        };
        for (lane, &(label, rms, peak, hold_index)) in levels
            .iter()
            .take(if self.stereo { 2 } else { 1 })
            .enumerate()
        {
            let center_y = if self.stereo {
                rect.top() + 8.0 + lane as f32 * 14.0
            } else {
                rect.center().y
            };
            let color = if self.clip_latched[hold_index] && live {
                info.theme.error
            } else if lane == 0 {
                info.theme.accent
            } else {
                info.theme.accent_alt
            };
            painter.text(
                Pos2::new(rect.left() + 14.0, center_y),
                egui::Align2::LEFT_CENTER,
                label,
                FontId::monospace(10.0),
                color,
            );
            let track = Rect::from_min_size(
                Pos2::new(rect.left() + 28.0, center_y - 3.0),
                Vec2::new(rect.width() - 83.0, 6.0),
            );
            painter.rect_filled(track, 2.0, info.theme.card);
            let level_db = |amplitude: f32| 20.0 * amplitude.max(1.0e-7).log10();
            let fraction = |amplitude: f32| {
                if live {
                    ((level_db(amplitude) + 60.0) / 60.0).clamp(0.0, 1.0)
                } else {
                    0.0
                }
            };
            painter.rect_filled(
                Rect::from_min_size(
                    track.min,
                    Vec2::new(track.width() * fraction(rms), track.height()),
                ),
                2.0,
                color,
            );
            if live {
                let x = track.left() + track.width() * fraction(peak);
                painter.line_segment(
                    [
                        Pos2::new(x, track.top() - 1.0),
                        Pos2::new(x, track.bottom() + 1.0),
                    ],
                    Stroke::new(1.0, info.theme.foreground),
                );
                let held_x = track.left() + track.width() * fraction(self.held_peaks[hold_index]);
                painter.line_segment(
                    [
                        Pos2::new(held_x, track.top() - 2.0),
                        Pos2::new(held_x, track.bottom() + 2.0),
                    ],
                    Stroke::new(
                        1.5,
                        if self.clip_latched[hold_index] {
                            info.theme.error
                        } else {
                            info.theme.warning
                        },
                    ),
                );
            }
            painter.text(
                Pos2::new(rect.right(), center_y),
                egui::Align2::RIGHT_CENTER,
                if live {
                    if self.clip_latched[hold_index] {
                        "CLIP".into()
                    } else {
                        format!("{:.1}", level_db(self.held_peaks[hold_index]))
                    }
                } else {
                    "—".into()
                },
                FontId::monospace(10.0),
                color,
            );
        }
        response.help_text_with(|| format!("{} · {} levels: RMS bars, live peak markers, retained peak markers, and latched clip indicators. Reset hold in Settings → Analysis & Meters. Capture: {}. {} dropped samples.",
            info.source_label, if self.stereo { "Stereo L/R" } else { self.channel.label() }, info.status.label(), info.frame.dropped_samples));
    }
}

fn fps_menu(ui: &mut egui::Ui, target: &mut u32, info: &BarInfo<'_>) -> egui::Response {
    let label = if info.fps.is_finite() && info.fps > 0.0 {
        format!("{:.0} FPS", info.fps)
    } else {
        "— FPS".into()
    };
    let base = ui.visuals().panel_fill;
    let text = info.theme.foreground;
    let accent = info.theme.accent;
    let config = egui::containers::menu::MenuConfig::new().style(egui::style::StyleModifier::new(
        move |style| crate::dropdown::popup_style(style, base, text, accent),
    ));
    let (response, _) = egui::containers::menu::MenuButton::from_button(
        egui::Button::new(RichText::new(label).size(11.0).color(info.theme.muted))
            .frame(false)
            .min_size(Vec2::new(68.0, 26.0)),
    )
    .config(config)
    .ui(ui, |ui| {
        ui.weak("Target frame rate");
        for rate in [30, 60, 90, 120] {
            if ui
                .selectable_label(*target == rate, format!("{rate} FPS"))
                .clicked()
            {
                *target = rate;
                ui.close();
            }
        }
    });
    let arrow = response.rect.right_center() - Vec2::new(5.0, 0.0);
    ui.painter().add(egui::Shape::line(
        vec![
            arrow + Vec2::new(-2.0, -1.0),
            arrow + Vec2::new(0.0, 1.0),
            arrow + Vec2::new(2.0, -1.0),
        ],
        Stroke::new(1.0, info.theme.muted),
    ));
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
        .help_text(format!("Measured display frame rate · target {target} FPS. Click to change the global display target. Audio analysis and recording frame rates are unchanged."))
}

pub(crate) fn channel_selector(
    ui: &mut egui::Ui,
    id: &str,
    channel: &mut ChannelMode,
) -> egui::Response {
    crate::dropdown::ComboBox::from_id_salt(id)
        .selected_text(channel.label())
        .show_ui(ui, |ui| {
            for value in ChannelMode::ALL {
                ui.selectable_value(channel, value, value.label());
            }
        })
        .response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_bar_fits_minimum_width_and_keeps_help_left_and_levels_right() {
        for width in [320.0, 400.0, 540.0, 740.0, 900.0, 1000.0, 1416.0] {
            let context = egui::Context::default();
            let mut bar = GlobalBar::default();
            let mut help = false;
            let mut target = 60;
            let bounds = Rect::from_min_size(Pos2::new(12.0, 600.0), Vec2::new(width, 40.0));
            let mut actual = Rect::NOTHING;
            let mut output = context.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        Pos2::ZERO,
                        Vec2::new(width + 24.0, 700.0),
                    )),
                    ..Default::default()
                },
                |ui| {
                    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(bounds));
                    bar.show(
                        &mut child,
                        &mut help,
                        &mut target,
                        BarInfo {
                            frame: &AnalysisFrame::default(),
                            status: &AudioStatus::Idle,
                            theme: &AppTheme::default(),
                            fps: 60.0,
                            source_label: "System audio",
                        },
                    );
                    actual = child.min_rect();
                },
            );
            output.textures_delta.clear();
            assert!(
                bounds.expand(0.5).contains_rect(actual),
                "bar overflow: {actual:?}"
            );
            let labels: Vec<_> = output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) => Some((text.galley.job.text.as_str(), text.pos)),
                    _ => None,
                })
                .collect();
            let x = |name| labels.iter().find(|(label, _)| *label == name).unwrap().1.x;
            let help = output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Circle(c) if (c.radius - 9.5).abs() < 0.01 => Some(c.center),
                    _ => None,
                })
                .unwrap();
            assert!((help.y - bounds.center().y).abs() < 0.1);
            assert!(help.x < x("60 FPS") && x("60 FPS") < bounds.center().x);
            assert!(x("System audio") > x("60 FPS"));
            assert!(x("System audio") < x("L"));
            assert!(
                !labels
                    .iter()
                    .any(|(text, _)| ["Macros", "Signals"].contains(text))
            );
            assert!(
                !labels
                    .iter()
                    .any(|(label, _)| ["FFT", "Rate"].contains(label))
            );
            assert!(x("L") > bounds.center().x && x("R") > bounds.center().x);
            assert!(!labels.iter().any(|(label, _)| {
                [
                    "Live",
                    "FFT window",
                    "Frequency-analysis channel",
                    "Single-view channel",
                ]
                .contains(label)
            }));
        }
    }

    #[test]
    fn peak_and_clip_hold_are_independent_and_reset_with_capture_state() {
        let mut bar = GlobalBar::default();
        let theme = AppTheme::default();
        let mut frame = AnalysisFrame {
            capture_epoch: 4,
            peak: [0.8, 1.1],
            mix_peak: 0.7,
            ..AnalysisFrame::default()
        };
        bar.update_meter_hold(&BarInfo {
            frame: &frame,
            status: &AudioStatus::Streaming,
            theme: &theme,
            fps: 60.0,
            source_label: "System audio",
        });
        assert_eq!(bar.held_peaks, [0.8, 1.1, 0.7]);
        assert_eq!(bar.clip_latched, [false, true, false]);

        frame.peak = [0.2, 0.3];
        frame.mix_peak = 0.1;
        bar.update_meter_hold(&BarInfo {
            frame: &frame,
            status: &AudioStatus::Streaming,
            theme: &theme,
            fps: 60.0,
            source_label: "System audio",
        });
        assert_eq!(bar.held_peaks, [0.8, 1.1, 0.7]);
        assert_eq!(bar.clip_latched, [false, true, false]);

        frame.capture_epoch = 5;
        bar.update_meter_hold(&BarInfo {
            frame: &frame,
            status: &AudioStatus::Streaming,
            theme: &theme,
            fps: 60.0,
            source_label: "System audio",
        });
        assert_eq!(bar.held_peaks, [0.2, 0.3, 0.1]);
        assert_eq!(bar.clip_latched, [false; 3]);

        bar.update_meter_hold(&BarInfo {
            frame: &frame,
            status: &AudioStatus::Idle,
            theme: &theme,
            fps: 60.0,
            source_label: "System audio",
        });
        assert_eq!(bar.held_peaks, [0.0; 3]);
        assert_eq!(bar.clip_latched, [false; 3]);
        assert_eq!(bar.hold_epoch, None);
    }
}
