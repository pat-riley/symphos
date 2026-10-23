//! Application preferences: a draggable modal with categories and immediate changes.
use super::*;
use crate::analysis::{FFT_SIZES, WindowFunction};
use crate::theme::ThemeChoice;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(super) enum Category {
    Display,
    #[default]
    Theme,
    Audio,
    Midi,
    Analysis,
    Help,
}
impl Category {
    const ALL: [Self; 6] = [
        Self::Display,
        Self::Theme,
        Self::Audio,
        Self::Midi,
        Self::Analysis,
        Self::Help,
    ];
    fn label(self) -> &'static str {
        match self {
            Self::Display => "Display & Layout",
            Self::Theme => "Theme & Colors",
            Self::Audio => "Audio",
            Self::Midi => "MIDI",
            Self::Analysis => "Analysis & Meters",
            Self::Help => "Help & About",
        }
    }
}
#[derive(Default)]
pub(super) struct SettingsWindow {
    pub open: bool,
    pub category: Category,
    position: Option<Pos2>,
}
impl SettingsWindow {
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        theme: &AppTheme,
        mut content: impl FnMut(&mut egui::Ui, Category),
    ) -> Rect {
        let screen = ctx.content_rect();
        let size = Vec2::new(
            760.0_f32.min(screen.width() - 24.0).max(300.0),
            510.0_f32.min(screen.height() - 24.0).max(260.0),
        );
        let mut position = self.position.unwrap_or(screen.center() - size * 0.5);
        position.x = position
            .x
            .clamp(screen.left(), (screen.right() - size.x).max(screen.left()));
        position.y = position
            .y
            .clamp(screen.top(), (screen.bottom() - size.y).max(screen.top()));
        let mut close = false;
        let response = egui::Modal::new(egui::Id::new("application-settings"))
            .area(
                egui::Area::new(egui::Id::new("application-settings"))
                    .kind(egui::UiKind::Modal)
                    .order(egui::Order::Foreground)
                    .fixed_pos(position)
                    .sense(Sense::hover()),
            )
            .frame(
                egui::Frame::new()
                    .fill(theme.panel)
                    .corner_radius(8)
                    .stroke(Stroke::new(1.0, theme.foreground.gamma_multiply(0.18)))
                    .inner_margin(0),
            )
            .backdrop_color(Color32::from_black_alpha(80))
            .show(ctx, |ui| {
                ui.set_min_size(size);
                ui.set_max_size(size);
                ui.spacing_mut().item_spacing = Vec2::new(8.0, 8.0);
                let rect = Rect::from_min_size(ui.cursor().min, size);
                let header = Rect::from_min_size(rect.min, Vec2::new(size.x, 40.0));
                ui.painter().rect_filled(
                    header,
                    egui::CornerRadius {
                        nw: 8,
                        ne: 8,
                        sw: 0,
                        se: 0,
                    },
                    theme.navigation,
                );
                let drag_rect = Rect::from_min_max(
                    header.min,
                    Pos2::new(header.right() - 42.0, header.bottom()),
                );
                let drag = ui
                    .interact(
                        drag_rect,
                        ui.id().with("settings-title-drag"),
                        Sense::drag(),
                    )
                    .on_hover_cursor(egui::CursorIcon::Grab);
                if drag.dragged() {
                    position += drag.drag_delta();
                }
                ui.painter().text(
                    header.center(),
                    egui::Align2::CENTER_CENTER,
                    "Settings",
                    FontId::proportional(16.0),
                    theme.foreground,
                );
                let close_rect = Rect::from_center_size(
                    header.right_center() - Vec2::new(22.0, 0.0),
                    Vec2::splat(26.0),
                );
                let mut close_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .id_salt("settings-close")
                        .max_rect(close_rect),
                );
                if icons::sized_button(
                    &mut close_ui,
                    Icon::Close,
                    false,
                    26.0,
                    "Close Settings · Escape",
                )
                .clicked()
                {
                    close = true;
                }
                let body = Rect::from_min_max(
                    header.left_bottom(),
                    rect.right_bottom() - Vec2::new(0.0, 28.0),
                );
                let nav =
                    Rect::from_min_max(body.min, Pos2::new(body.left() + 164.0, body.bottom()));
                ui.painter().rect_filled(nav, 0.0, theme.card);
                ui.painter().vline(
                    nav.right(),
                    nav.y_range(),
                    Stroke::new(1.0, theme.foreground.gamma_multiply(0.12)),
                );
                let mut nav_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .id_salt("settings-categories")
                        .max_rect(nav.shrink2(Vec2::new(6.0, 12.0))),
                );
                nav_ui.spacing_mut().item_spacing.y = 3.0;
                nav_ui.style_mut().override_font_id = Some(FontId::proportional(13.0));
                for category in Category::ALL {
                    if nav_ui
                        .add(
                            egui::Button::selectable(self.category == category, category.label())
                                .min_size(Vec2::new(nav_ui.available_width(), 36.0)),
                        )
                        .clicked()
                    {
                        self.category = category;
                    }
                }
                let panel = Rect::from_min_max(
                    Pos2::new(nav.right() + 20.0, body.top() + 16.0),
                    body.max - Vec2::new(20.0, 12.0),
                );
                let mut panel_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .id_salt("settings-content")
                        .max_rect(panel),
                );
                panel_ui.set_clip_rect(panel.intersect(ui.clip_rect()));
                egui::ScrollArea::vertical()
                    .id_salt(("settings-category-scroll", self.category))
                    .auto_shrink([false, false])
                    .max_height(panel.height())
                    .show(&mut panel_ui, |ui| {
                        ui.set_width(panel.width());
                        ui.style_mut().override_font_id = Some(FontId::proportional(14.0));
                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                        content(ui, self.category);
                    });
                ui.painter().text(
                    rect.right_bottom() - Vec2::new(16.0, 8.0),
                    egui::Align2::RIGHT_BOTTOM,
                    "Changes apply immediately",
                    FontId::proportional(11.0),
                    theme.muted,
                );
            });
        if close || response.should_close() {
            self.open = false;
        }
        self.position = Some(position);
        response.response.rect
    }
}
fn section(ui: &mut egui::Ui, label: &str) {
    if ui.cursor().min.y > ui.min_rect().top() + 10.0 {
        ui.add_space(12.0);
    }
    ui.strong(label);
    ui.separator();
    ui.add_space(3.0);
}
fn row(ui: &mut egui::Ui, label: &str, control: impl FnOnce(&mut egui::Ui)) {
    let width = ui.available_width();
    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(
            Vec2::new(width * 0.43, 28.0),
            Layout::left_to_right(Align::Center),
            |ui| {
                ui.label(label);
            },
        );
        ui.allocate_ui_with_layout(
            Vec2::new(width * 0.54, 28.0),
            Layout::left_to_right(Align::Center),
            control,
        );
    })
    .response
    .help_fallback(|| format!("{label}\n{}", help::control_description(label)));
}
impl SymphosApp {
    pub(super) fn show_settings(&mut self, ctx: &egui::Context, frame: &AnalysisFrame) {
        if !self.settings_window.open {
            return;
        }
        let mut dialog = std::mem::take(&mut self.settings_window);
        let theme = self.theme.clone();
        let mut dismiss = false;
        dialog.show(ctx, &theme, |ui, category| {
            dismiss |= self.settings_content(ui, category, frame);
        });
        if dismiss {
            dialog.open = false;
        }
        self.settings_window = dialog;
        self.apply_appearance(ctx);
    }
    fn settings_content(
        &mut self,
        ui: &mut egui::Ui,
        category: Category,
        frame: &AnalysisFrame,
    ) -> bool {
        let mut dismiss = false;
        match category {
            Category::Theme => {
                section(ui, "GUI theme");
                ui.label("GUI skins style navigation, sidebars, controls, and dialogs. Visualization palettes are independent.");
                let width = (ui.available_width() - 12.0) * 0.5;
                for choices in ThemeChoice::ALL.chunks(2) {
                    ui.horizontal(|ui| {
                        for &choice in choices {
                            let preview = choice.resolve(&self.automatic_theme);
                            let (rect, response) =
                                ui.allocate_exact_size(Vec2::new(width, 72.0), Sense::click());
                            let painter = ui.painter();
                            painter.rect_filled(rect, 5.0, preview.card);
                            painter.rect_stroke(
                                rect,
                                5.0,
                                Stroke::new(
                                    if self.appearance.theme == choice {
                                        2.0
                                    } else {
                                        1.0
                                    },
                                    if self.appearance.theme == choice {
                                        self.theme.accent
                                    } else {
                                        self.theme.muted.gamma_multiply(0.4)
                                    },
                                ),
                                egui::StrokeKind::Inside,
                            );
                            painter.text(
                                rect.left_top() + Vec2::new(12.0, 7.0),
                                egui::Align2::LEFT_TOP,
                                choice.label(),
                                FontId::proportional(14.0),
                                preview.foreground,
                            );
                            painter.text(
                                rect.left_top() + Vec2::new(12.0, 27.0),
                                egui::Align2::LEFT_TOP,
                                choice.description(),
                                FontId::proportional(11.0),
                                preview.muted,
                            );
                            for (i, color) in [
                                preview.background,
                                preview.panel,
                                preview.accent,
                                preview.accent_alt,
                            ]
                            .into_iter()
                            .enumerate()
                            {
                                painter.circle_filled(
                                    rect.left_bottom() + Vec2::new(18.0 + i as f32 * 22.0, -17.0),
                                    6.0,
                                    color,
                                );
                            }
                            response.widget_info(|| {
                                egui::WidgetInfo::labeled(
                                    egui::WidgetType::RadioButton,
                                    true,
                                    format!("{} theme", choice.label()),
                                )
                            });
                            if response.clicked() {
                                self.appearance.theme = choice;
                            }
                        }
                    });
                }
                ui.small(if cfg!(target_os = "linux") {
                    "Automatic follows your current Omarchy palette."
                } else {
                    "Automatic uses the default Symphos palette."
                });
                section(ui, "Visualization colors · independent");
                ui.label("Use the inspector's Appearance tab for gradients, custom color stops, saved palettes, and swatches.");
                if ui
                    .add_enabled(
                        self.tiles.is_some(),
                        egui::Button::new("Edit selected component colors"),
                    )
                    .clicked()
                {
                    self.panes[self.selected_pane]
                        .select_section(crate::modules::SettingsSection::Appearance);
                    self.sidebar_open = true;
                    dismiss = true;
                }
            }
            Category::Display => {
                section(ui, "Display performance");
                row(ui, "Target frame rate", |ui| {
                    crate::dropdown::ComboBox::from_id_salt("display-fps").selected_text(format!("{} FPS", self.render_fps)).show_ui(ui, |ui| {
                        for rate in [30, 60, 90, 120] { ui.selectable_value(&mut self.render_fps, rate, format!("{rate} FPS")); }
                    }).response.help_text("Global display target, saved on this computer. Does not change audio analysis or project settings. Actual FPS depends on GPU load and display refresh rate.");
                });

                section(ui, "Display");
                let mut zoom = self
                    .pending_zoom
                    .unwrap_or_else(|| self.appearance.ui_scale.unwrap_or(self.automatic_scale))
                    * 100.0;
                let mut commit = false;
                row(ui, "Interface zoom", |ui| {
                    let response = ui.add(
                        egui::Slider::new(&mut zoom, 60.0..=200.0)
                            .step_by(5.0)
                            .suffix("%"),
                    );
                    if response.changed() {
                        self.pending_zoom = Some(zoom / 100.0);
                    }
                    commit = !response.dragged() && self.pending_zoom.is_some();
                });
                if commit {
                    self.appearance.ui_scale = self.pending_zoom.take();
                }
                if ui.small_button("Use default zoom").clicked() {
                    self.appearance.ui_scale = None;
                    self.pending_zoom = None;
                }
                row(ui, "Application fullscreen", |ui| {
                    if ui
                        .button(if self.fullscreen {
                            "Exit fullscreen"
                        } else {
                            "Enter fullscreen"
                        })
                        .clicked()
                    {
                        self.toggle_fullscreen(ui.ctx());
                    }
                });
                section(ui, "Workspace");
                row(ui, "Component inspector", |ui| {
                    ui.checkbox(&mut self.sidebar_open, "Show");
                });
                row(ui, "Info View / hover help", |ui| {
                    ui.checkbox(&mut self.help_open, "Show");
                });
                if ui.button("Balance pane sizes").clicked()
                    && let Some(tree) = &mut self.tiles
                {
                    equalize_splits(tree);
                }
                ui.small("Use the top scene menu to recall complete setups or open the visual preset library. File → Save Project saves all scenes.");
            }
            Category::Audio => {
                section(ui, "Audio source");
                row(ui, "Capture source", |ui| {
                    self.source_selector(ui);
                });
                row(ui, "Capture status", |ui| {
                    ui.label(self.status.label());
                });
                row(ui, "Format", |ui| {
                    ui.label(format!(
                        "{:.1} kHz · {} channels",
                        frame.sample_rate as f32 / 1000.0,
                        frame.channels
                    ));
                });
                row(ui, "Captured signal", |ui| {
                    ui.label(format!(
                        "RMS {:.1} dBFS",
                        20.0 * frame.mix_rms.max(1e-9).log10()
                    ));
                });
                if ui
                    .add_enabled(
                        self.selected_node.is_some(),
                        egui::Button::new("Retry audio capture"),
                    )
                    .clicked()
                    && let Some(source) = self
                        .sources
                        .iter()
                        .find(|s| Some(s.node_name.as_str()) == self.selected_node.as_deref())
                        .cloned()
                {
                    self.engine.select_source(source);
                    self.status = AudioStatus::Connecting;
                    self.reset_capture();
                }
                #[cfg(target_os = "macos")]
                {
                    section(ui, "macOS permissions");
                    ui.label("Allow Symphos in System Settings → Privacy & Security → Screen & System Audio Recording. After enabling permission, retry capture.");
                }
                #[cfg(target_os = "linux")]
                {
                    section(ui, "Linux capture");
                    ui.label("System output and microphone sources are provided by PipeWire. Choose the output monitor for computer playback.");
                }
            }
            Category::Midi => {
                if let Some(midi) = &mut self.midi_runtime {
                    midi.settings(ui);
                }
                section(ui, "Controller mappings · current scene");
                ui.label("Right-click a parameter or assigned macro and choose Learn MIDI controller. Move an absolute knob, fader, or pad to assign it.");
                if self.scene_macros.midi.is_empty() {
                    ui.weak("No direct controller mappings in this scene.");
                }
                let snapshot = self.midi_runtime.as_ref().map(|m| m.snapshot.load_full());
                for (index, mapping) in self.scene_macros.midi.iter().enumerate() {
                    ui.push_id(("midi-mapping", index), |ui| {
                        egui::Frame::group(ui.style()).show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.strong(&mapping.parameter.label);
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui.small_button("Remove").clicked() {
                                            crate::midi::mapping::remove(ui.ctx(), mapping);
                                        }
                                    },
                                );
                            });
                            let pane = self
                                .panes
                                .iter()
                                .position(|p| p.instance_id() == mapping.target.pane);
                            ui.label(format!(
                                "{} · {}",
                                pane.map_or_else(
                                    || "Missing component".into(),
                                    |i| format!("Component {}", i + 1)
                                ),
                                mapping.target.kind.label()
                            ));
                            let port = snapshot
                                .as_ref()
                                .and_then(|s| {
                                    s.ports.iter().find(|p| Some(p.id) == mapping.input.port)
                                })
                                .map(|p| p.name.as_str())
                                .unwrap_or("Input unavailable");
                            ui.weak(format!(
                                "{port} · Ch {} · {} {}",
                                mapping.input.channel.unwrap_or(1),
                                mapping.input.message.label(),
                                mapping
                                    .input
                                    .number
                                    .map_or(String::new(), |n| n.to_string())
                            ));
                            ui.small(format!(
                                "{}–{} · {}",
                                mapping.parameter.min,
                                mapping.parameter.max,
                                if snapshot
                                    .as_ref()
                                    .is_some_and(|s| s.available(&mapping.input))
                                {
                                    "Ready"
                                } else {
                                    "Input disconnected or disabled"
                                }
                            ));
                        });
                    });
                }
            }
            Category::Analysis => {
                let mut settings = self
                    .engine
                    .analysis()
                    .settings
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone();
                let before = (settings.fft_size, settings.window, settings.channel_mode);
                section(ui, "Frequency analysis");
                row(ui, "FFT size", |ui| {
                    crate::dropdown::ComboBox::from_id_salt("settings-fft")
                        .selected_text(settings.fft_size.to_string())
                        .show_ui(ui, |ui| {
                            for size in FFT_SIZES {
                                ui.selectable_value(&mut settings.fft_size, size, size.to_string());
                            }
                        });
                });
                row(ui, "FFT window", |ui| {
                    crate::dropdown::ComboBox::from_id_salt("settings-window")
                        .selected_text(settings.window.label())
                        .show_ui(ui, |ui| {
                            for w in WindowFunction::ALL {
                                ui.selectable_value(&mut settings.window, w, w.label());
                            }
                        });
                });
                row(ui, "Analysis channel", |ui| {
                    crate::global_bar::channel_selector(
                        ui,
                        "settings-analysis-channel",
                        &mut settings.channel_mode,
                    );
                });
                row(ui, "Analysis rate", |ui| {
                    crate::dropdown::ComboBox::from_id_salt("settings-rate")
                        .selected_text(format!("{} Hz", settings.analysis_fps))
                        .show_ui(ui, |ui| {
                            for rate in [30, 60, 90, 120] {
                                ui.selectable_value(
                                    &mut settings.analysis_fps,
                                    rate,
                                    format!("{rate} Hz"),
                                );
                            }
                        });
                });
                ui.small("FFT size trades time resolution for frequency detail. Changing size, window, or analysis channel clears captured histories.");
                *self
                    .engine
                    .analysis()
                    .settings
                    .lock()
                    .unwrap_or_else(|e| e.into_inner()) = settings.clone();
                if before != (settings.fft_size, settings.window, settings.channel_mode) {
                    self.reset_modules();
                }
                section(ui, "Footer meters");
                row(ui, "Meter layout", |ui| {
                    ui.checkbox(&mut self.global_bar.stereo, "Stereo L/R");
                });
                row(ui, "Single-meter channel", |ui| {
                    crate::global_bar::channel_selector(
                        ui,
                        "settings-meter-channel",
                        &mut self.global_bar.channel,
                    );
                });
                if ui.button("Reset peak / clip hold").clicked() {
                    self.global_bar.reset_meter_hold();
                }
                row(ui, "Dropped samples", |ui| {
                    ui.label(frame.dropped_samples.to_string());
                });
                if ui
                    .button(if self.show_inspector {
                        "Close FFT diagnostics"
                    } else {
                        "Open FFT diagnostics"
                    })
                    .clicked()
                {
                    self.show_inspector = !self.show_inspector;
                    dismiss = self.show_inspector;
                }
            }
            Category::Help => {
                section(ui, "Help");
                if ui
                    .button(crate::shortcuts::platform_label(
                        "Keyboard shortcuts · Ctrl+K",
                    ))
                    .clicked()
                {
                    self.show_shortcuts = true;
                    dismiss = true;
                }
                if ui
                    .button(format!("Issue log ({})", crate::issues::count()))
                    .clicked()
                {
                    self.show_issues = true;
                    dismiss = true;
                }
                section(ui, "About Symphos");
                build_details(ui);
                row(ui, "Platform", |ui| {
                    ui.label(format!(
                        "{} / {}",
                        std::env::consts::OS,
                        std::env::consts::ARCH
                    ));
                });
                row(ui, "Current theme", |ui| {
                    ui.label(&self.theme.name);
                });
            }
        }
        dismiss
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn render(dialog: &mut SettingsWindow, ctx: &egui::Context, events: Vec<egui::Event>) -> Rect {
        let mut rect = Rect::NOTHING;
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0))),
                events,
                ..Default::default()
            },
            |ui| {
                rect = dialog.show(ui.ctx(), &AppTheme::default(), |ui, _| {
                    ui.label("Test settings");
                });
            },
        );
        output.textures_delta.clear();
        rect
    }
    fn pointer(pos: Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            pressed,
            button: egui::PointerButton::Primary,
            modifiers: egui::Modifiers::NONE,
        }
    }
    #[test]
    fn settings_drag_only_moves_from_title_bar_and_escape_closes() {
        let ctx = egui::Context::default();
        let mut dialog = SettingsWindow {
            open: true,
            ..Default::default()
        };
        render(&mut dialog, &ctx, vec![]);
        let rect = render(&mut dialog, &ctx, vec![]);
        let start = rect.left_top() + Vec2::new(220.0, 20.0);
        render(
            &mut dialog,
            &ctx,
            vec![egui::Event::PointerMoved(start), pointer(start, true)],
        );
        let end = start + Vec2::new(85.0, 60.0);
        render(&mut dialog, &ctx, vec![egui::Event::PointerMoved(end)]);
        let moved = render(&mut dialog, &ctx, vec![pointer(end, false)]);
        assert!(
            (moved.min - rect.min - Vec2::new(85.0, 60.0)).length() < 2.0,
            "{rect:?} -> {moved:?}"
        );
        let body = moved.min + Vec2::new(230.0, 330.0);
        render(
            &mut dialog,
            &ctx,
            vec![egui::Event::PointerMoved(body), pointer(body, true)],
        );
        let end = body + Vec2::new(20.0, 20.0);
        render(&mut dialog, &ctx, vec![egui::Event::PointerMoved(end)]);
        let still = render(&mut dialog, &ctx, vec![pointer(end, false)]);
        assert!((still.min - moved.min).length() < 2.0);
        assert!(dialog.open);
        render(
            &mut dialog,
            &ctx,
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert!(!dialog.open);
    }
    #[test]
    fn settings_categories_remain_selected_after_reopening() {
        let ctx = egui::Context::default();
        let mut dialog = SettingsWindow {
            open: true,
            ..Default::default()
        };
        render(&mut dialog, &ctx, vec![]);
        let rect = render(&mut dialog, &ctx, vec![]);
        let audio = rect.min + Vec2::new(80.0, 40.0 + 12.0 + 39.0 * 2.0 + 18.0);
        render(
            &mut dialog,
            &ctx,
            vec![egui::Event::PointerMoved(audio), pointer(audio, true)],
        );
        render(&mut dialog, &ctx, vec![pointer(audio, false)]);
        assert_eq!(dialog.category, Category::Audio);
        dialog.open = false;
        dialog.open = true;
        render(&mut dialog, &ctx, vec![]);
        assert_eq!(dialog.category, Category::Audio);
    }
}
