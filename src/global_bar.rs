use crate::{
    analysis::{AnalysisFrame, AnalysisSettings, ChannelMode, FFT_SIZES, WindowFunction},
    audio::AudioStatus,
    help::{self, HoverHelp},
    icons::{self, Icon},
    theme::AppTheme,
};
use eframe::egui::{self, Align, FontId, Layout, Pos2, Rect, RichText, Sense, Stroke, Vec2};

pub struct GlobalBar {
    pub stereo: bool,
    pub channel: ChannelMode,
}

impl Default for GlobalBar {
    fn default() -> Self {
        Self {
            stereo: true,
            channel: ChannelMode::StereoMix,
        }
    }
}

pub struct BarInfo<'a> {
    pub frame: &'a AnalysisFrame,
    pub status: &'a AudioStatus,
    pub theme: &'a AppTheme,
    pub fps: f32,
}

impl GlobalBar {
    /// Returns whether shared signal interpretation changed and histories need clearing.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        settings: &mut AnalysisSettings,
        help_open: &mut bool,
        inspector: &mut bool,
        info: BarInfo<'_>,
    ) -> bool {
        let before = (settings.fft_size, settings.window, settings.channel_mode);
        ui.spacing_mut().item_spacing = Vec2::new(7.0, 4.0);
        ui.spacing_mut().button_padding = Vec2::new(6.0, 4.0);
        ui.spacing_mut().interact_size.y = 26.0;
        ui.style_mut().override_font_id = Some(FontId::proportional(12.0));
        ui.horizontal_centered(|ui| {
            help::toggle(ui, help_open);
            ui.separator();
            ui.label("FFT");
            egui::ComboBox::from_id_salt("global-fft").width(72.0)
                .selected_text(settings.fft_size.to_string()).show_ui(ui, |ui| {
                    for size in FFT_SIZES {
                        ui.selectable_value(&mut settings.fft_size, size, size.to_string())
                            .help_text("Larger FFT sizes give finer frequency detail and a longer analysis window. Changing this clears all histories.");
                    }
                }).response.help_text("Shared FFT sample count. Larger sizes give finer frequency detail and a longer capture window.");
            ui.label("Rate");
            egui::ComboBox::from_id_salt("global-rate").width(66.0)
                .selected_text(format!("{} Hz", settings.analysis_fps)).show_ui(ui, |ui| {
                    for rate in [30, 60, 90, 120] {
                        ui.selectable_value(&mut settings.analysis_fps, rate, format!("{rate} Hz"))
                            .help_text("Target analysis and display update rate. Higher rates use more CPU/GPU resources.");
                    }
                }).response.help_text("Shared target analysis/display update rate. Does not change history duration.");
            if icons::button(ui, if self.stereo { Icon::Stereo } else { Icon::Mix }, !self.stereo,
                if self.stereo { "Stereo view: separate L/R meters and waveform lanes. Click for a mixed single-channel view. Frequency analysis is unchanged." }
                else { "Mixed view: combined meters and waveform. Click for separate L/R channels. Choose a left/right-only override in Shared options." }).clicked() {
                self.stereo = !self.stereo;
            }
            let options = icons::button(ui, Icon::Settings, false,
                "Shared options: FFT window, analysis channel, single-view channel, diagnostics, and capture status.");
            egui::Popup::menu(&options).show(|ui| {
                ui.set_min_width(245.0);
                ui.strong("SHARED OPTIONS");
                ui.label("FFT window");
                egui::ComboBox::from_id_salt("global-window").selected_text(settings.window.label()).show_ui(ui, |ui| {
                    for window in WindowFunction::ALL {
                        ui.selectable_value(&mut settings.window, window, window.label());
                    }
                }).response.help_text("Window applied before the FFT. Changing it clears all histories.");
                ui.label("Frequency-analysis channel");
                channel_selector(ui, "analysis-channel", &mut settings.channel_mode)
                    .help_text("Shared signal analyzed by frequency modules: stereo mix, left, or right. Changing this clears histories.");
                ui.label("Single-view channel");
                channel_selector(ui, "view-channel", &mut self.channel)
                    .help_text("Signal used by meters and waveform panes when mixed view is active: stereo mix, left, or right. Does not change the FFT signal.");
                ui.separator();
                ui.checkbox(inspector, "FFT inspector / diagnostics")
                    .help_text("Open raw frequency bins and detailed audio-analysis statistics.");
                ui.label(format!("Capture: {}", info.status.label()));
                ui.label(format!("{} Hz · {} channels", info.frame.sample_rate, info.frame.channels));
                ui.label(format!("Dropped samples: {}", info.frame.dropped_samples));
                ui.label(format!("Theme: {}", info.theme.name));
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                self.meters(ui, &info);
                ui.label(RichText::new(format!("{:.1} kHz", info.frame.sample_rate as f32 / 1000.0)).small().color(info.theme.muted))
                    .help_text("Sample rate of the selected audio source.");
                ui.label(RichText::new(format!("{:.0} FPS", info.fps)).small().color(info.theme.muted))
                    .help_text("Measured display frame rate.");
            });
        });
        before != (settings.fft_size, settings.window, settings.channel_mode)
    }

    fn meters(&self, ui: &mut egui::Ui, info: &BarInfo<'_>) {
        let (rect, response) = ui.allocate_exact_size(Vec2::new(220.0, 30.0), Sense::hover());
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
                ("L", info.frame.rms[0], info.frame.peak[0]),
                ("R", info.frame.rms[1], info.frame.peak[1]),
            ]
        } else {
            let (label, rms, peak) = match self.channel {
                ChannelMode::StereoMix => ("M", info.frame.mix_rms, info.frame.mix_peak),
                ChannelMode::Left => ("L", info.frame.rms[0], info.frame.peak[0]),
                ChannelMode::Right => ("R", info.frame.rms[1], info.frame.peak[1]),
            };
            [(label, rms, peak), (label, rms, peak)]
        };
        for (lane, &(label, rms, peak)) in levels
            .iter()
            .take(if self.stereo { 2 } else { 1 })
            .enumerate()
        {
            let center_y = if self.stereo {
                rect.top() + 8.0 + lane as f32 * 14.0
            } else {
                rect.center().y
            };
            let color = if peak >= 1.0 && live {
                info.theme.warning
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
                Vec2::new(137.0, 6.0),
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
            }
            painter.text(
                Pos2::new(rect.right(), center_y),
                egui::Align2::RIGHT_CENTER,
                if live {
                    format!("{:.1}", level_db(peak))
                } else {
                    "—".into()
                },
                FontId::monospace(10.0),
                color,
            );
        }
        response.help_text_with(|| format!("{} levels: RMS bars with peak markers and peak dBFS readouts. Capture: {}. {} dropped samples.",
            if self.stereo { "Stereo L/R" } else { self.channel.label() }, info.status.label(), info.frame.dropped_samples));
    }
}

fn channel_selector(ui: &mut egui::Ui, id: &str, channel: &mut ChannelMode) -> egui::Response {
    egui::ComboBox::from_id_salt(id)
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
        for width in [1000.0, 1416.0] {
            let context = egui::Context::default();
            let mut bar = GlobalBar::default();
            let mut settings = AnalysisSettings::default();
            let mut help = false;
            let mut inspector = false;
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
                    assert!(!bar.show(
                        &mut child,
                        &mut settings,
                        &mut help,
                        &mut inspector,
                        BarInfo {
                            frame: &AnalysisFrame::default(),
                            status: &AudioStatus::Idle,
                            theme: &AppTheme::default(),
                            fps: 60.0,
                        }
                    ));
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
            assert!(x("? Help") < x("FFT"));
            assert!(x("FFT") < x("Rate"));
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
    fn stereo_icon_switches_meters_without_changing_fft_or_requesting_history_reset() {
        let context = egui::Context::default();
        let mut bar = GlobalBar::default();
        let mut settings = AnalysisSettings::default();
        let mut help = false;
        let mut inspector = false;
        let mut render = |bar: &mut GlobalBar, events| {
            let mut output = context.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1100.0, 100.0))),
                    events,
                    ..Default::default()
                },
                |ui| {
                    assert!(!bar.show(
                        ui,
                        &mut settings,
                        &mut help,
                        &mut inspector,
                        BarInfo {
                            frame: &AnalysisFrame::default(),
                            status: &AudioStatus::Streaming,
                            theme: &AppTheme::default(),
                            fps: 60.0,
                        }
                    ));
                },
            );
            output.textures_delta.clear();
            output
        };
        let output = render(&mut bar, vec![]);
        let position = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Circle(circle) if (circle.radius - 5.0).abs() < 0.001 => {
                    Some(circle.center)
                }
                _ => None,
            })
            .expect("stereo icon");
        for pressed in [true, false] {
            render(
                &mut bar,
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
        assert!(!bar.stereo);
        let output = render(&mut bar, vec![]);
        assert!(output.shapes.iter().any(
            |shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.job.text == "M")
        ));
        assert_eq!(settings.fft_size, 4096);
        assert_eq!(settings.channel_mode, ChannelMode::StereoMix);
    }
}
