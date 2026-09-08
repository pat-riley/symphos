use std::time::{Duration, Instant};

use eframe::egui::{
    self, Align, Color32, FontId, Layout, Pos2, Rect, RichText, Sense, Stroke, Vec2,
};

use crate::analysis::{AnalysisFrame, ChannelMode, FFT_SIZES, WindowFunction};
use crate::audio::{AudioEngine, AudioEvent, AudioSource, AudioStatus, SourceKind};
use crate::help::{self, HoverHelp};
use crate::modules::{ModuleKind, ModulePane};
use crate::theme::AppTheme;

const THEME_REFRESH_INTERVAL: Duration = Duration::from_millis(100);

pub struct SymphosApp {
    engine: AudioEngine,
    sources: Vec<AudioSource>,
    selected_node: Option<String>,
    source_manually_selected: bool,
    status: AudioStatus,
    theme: AppTheme,
    last_theme_check: Instant,
    fullscreen: bool,
    show_inspector: bool,
    display_fps: f32,
    last_frame: Instant,
    panes: [ModulePane; 4],
    selected_pane: usize,
    sidebar_open: bool,
    help_open: bool,
    focused_pane: Option<usize>,
    top_fraction: f32,
    bottom_splits: [f32; 2],
}

impl SymphosApp {
    pub fn new(context: &eframe::CreationContext<'_>) -> Self {
        let theme = AppTheme::load_omarchy();
        apply_style(&context.egui_ctx, &theme);
        let engine = AudioEngine::start();
        engine.refresh_sources();
        Self {
            engine,
            sources: Vec::new(),
            selected_node: None,
            source_manually_selected: false,
            status: AudioStatus::Idle,
            theme,
            last_theme_check: Instant::now(),
            fullscreen: false,
            show_inspector: false,
            display_fps: 0.0,
            last_frame: Instant::now(),
            panes: [
                ModulePane::new(ModuleKind::Waterfall),
                ModulePane::new(ModuleKind::Spectrum),
                ModulePane::new(ModuleKind::Waveform),
                ModulePane::new(ModuleKind::Spectrogram),
            ],
            selected_pane: 0,
            sidebar_open: true,
            help_open: false,
            focused_pane: None,
            top_fraction: 0.62,
            bottom_splits: [1.0 / 3.0, 2.0 / 3.0],
        }
    }

    fn reset_modules(&mut self) {
        for pane in &mut self.panes {
            pane.reset();
        }
    }

    fn process_audio_events(&mut self) {
        let events: Vec<_> = self.engine.drain_events().collect();
        let mut sources_changed = false;
        for event in events {
            match event {
                AudioEvent::Sources(sources) => {
                    self.sources = sources;
                    sources_changed = true;
                }
                AudioEvent::Status(status) => {
                    if !matches!(status, AudioStatus::Streaming) {
                        self.reset_modules();
                    }
                    self.status = status;
                }
            }
        }
        // Discovery arrives incrementally. Prefer speakers when they appear,
        // but never override an explicit choice (including a microphone).
        if sources_changed
            && let Some(source) = default_output_source(
                &self.sources,
                self.selected_node.as_deref(),
                self.source_manually_selected,
            )
            .cloned()
        {
            self.selected_node = Some(source.node_name.clone());
            self.status = AudioStatus::Connecting;
            self.reset_modules();
            self.engine.select_source(source);
        }
    }

    fn refresh_theme(&mut self, context: &egui::Context) {
        if self.last_theme_check.elapsed() < THEME_REFRESH_INTERVAL {
            return;
        }
        self.last_theme_check = Instant::now();
        let Some(theme) = AppTheme::load_current_omarchy() else {
            return;
        };
        if theme == self.theme {
            return;
        }
        self.theme = theme;
        apply_style(context, &self.theme);
        context.request_repaint();
    }

    fn source_selector(&mut self, ui: &mut egui::Ui) {
        let previous_node = self.selected_node.clone();
        let selected_label = self
            .sources
            .iter()
            .find(|source| Some(&source.node_name) == self.selected_node.as_ref())
            .map(|source| source.display_name.clone())
            .unwrap_or_else(|| {
                if self.sources.is_empty() {
                    "Looking for outputs…".into()
                } else {
                    "Select an audio source…".into()
                }
            });
        egui::ComboBox::from_id_salt("audio-source")
            .width(260.0)
            .truncate()
            .selected_text(selected_label)
            .show_ui(ui, |ui| {
                for source in &self.sources {
                    let selected = Some(&source.node_name) == self.selected_node.as_ref();
                    let label = format!("{}  {}", source.kind.label(), source.display_name);
                    if ui.selectable_label(selected, label).help_text(format!("Analyze {}: {}. Changing sources clears the displayed histories.", source.kind.label(), source.display_name)).clicked() {
                        self.source_manually_selected = true;
                        self.selected_node = Some(source.node_name.clone());
                        self.engine.select_source(source.clone());
                    }
                }
            }).response.help_text(format!("Audio source: {}. Speakers/system output is selected automatically on startup. Choose another output or microphone here; your choice is kept for this session.",
                self.sources.iter().find(|source| Some(&source.node_name) == self.selected_node.as_ref())
                    .map_or("none", |source| source.display_name.as_str())));
        if self.selected_node != previous_node {
            self.reset_modules();
        }
    }

    fn analysis_controls(&mut self, ui: &mut egui::Ui) {
        let mut settings = self
            .engine
            .analysis
            .settings
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        let before = (settings.fft_size, settings.window, settings.channel_mode);
        ui.small("Shared by all panes");
        egui::Grid::new("shared-audio-settings")
            .num_columns(2)
            .show(ui, |ui| {
                ui.label("FFT");
                egui::ComboBox::from_id_salt("fft-size")
                    .width(115.0)
                    .selected_text(format!("{} samples", settings.fft_size))
                    .show_ui(ui, |ui| {
                        for size in FFT_SIZES {
                            ui.selectable_value(&mut settings.fft_size, size, size.to_string()).help_text("Larger FFT sizes give finer frequency detail but a longer analysis window. Changing this clears histories in all panes.");
                        }
                    }).response.help_text("FFT sample count, shared by all panes. Larger sizes give finer frequency detail and a longer capture window.");
                ui.end_row();
                ui.label("Window");
                egui::ComboBox::from_id_salt("window")
                    .width(115.0)
                    .selected_text(settings.window.label())
                    .show_ui(ui, |ui| {
                        for window in WindowFunction::ALL {
                            ui.selectable_value(&mut settings.window, window, window.label());
                        }
                    }).response.help_text("Window function used before the FFT. Controls the tradeoff between frequency separation and spectral leakage. Changing this clears all histories.");
                ui.end_row();
                ui.label("Channel");
                egui::ComboBox::from_id_salt("channel")
                    .width(115.0)
                    .selected_text(settings.channel_mode.label())
                    .show_ui(ui, |ui| {
                        for channel in ChannelMode::ALL {
                            ui.selectable_value(
                                &mut settings.channel_mode,
                                channel,
                                channel.label(),
                            );
                        }
                    }).response.help_text("Channel or stereo combination used for shared frequency analysis. Changing this clears all histories.");
                ui.end_row();
                ui.label("Rate");
                egui::ComboBox::from_id_salt("analysis-fps")
                    .width(115.0)
                    .selected_text(format!("{} Hz", settings.analysis_fps))
                    .show_ui(ui, |ui| {
                        for rate in [30, 60, 90, 120] {
                            ui.selectable_value(
                                &mut settings.analysis_fps,
                                rate,
                                format!("{rate} Hz"),
                            );
                        }
                    }).response.help_text("Target analysis and display update rate. Higher rates use more CPU/GPU resources.");
                ui.end_row();
            });
        if before != (settings.fft_size, settings.window, settings.channel_mode) {
            self.reset_modules();
        }
        *self
            .engine
            .analysis
            .settings
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = settings;
        ui.checkbox(&mut self.show_inspector, "FFT inspector / diagnostics")
            .help_text("Open raw frequency bins and detailed audio-analysis statistics.");
    }

    fn sidebar(&mut self, ui: &mut egui::Ui, frame: &AnalysisFrame) {
        ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
        ui.spacing_mut().slider_width = 90.0;
        ui.style_mut()
            .text_styles
            .insert(egui::TextStyle::Body, FontId::proportional(13.0));
        ui.style_mut()
            .text_styles
            .insert(egui::TextStyle::Button, FontId::proportional(13.0));
        ui.spacing_mut().button_padding = Vec2::new(7.0, 4.0);
        ui.spacing_mut().interact_size.y = 24.0;
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("MODULE SETTINGS")
                    .small()
                    .color(self.theme.muted),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .small_button("‹")
                    .help_text("Collapse settings")
                    .clicked()
                {
                    self.sidebar_open = false;
                }
            });
        });
        ui.label(RichText::new(self.panes[self.selected_pane].kind.label()).strong());
        ui.label(
            RichText::new(format!(
                "Pane {} · click a pane to select",
                self.selected_pane + 1
            ))
            .small()
            .color(self.theme.muted),
        );
        ui.separator();
        egui::ScrollArea::vertical()
            .id_salt(("module-settings", self.selected_pane))
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.push_id(self.selected_pane, |ui| {
                    self.panes[self.selected_pane].controls(ui, frame)
                });
                ui.add_space(8.0);
                ui.separator();
                crate::modules::settings_panel(ui, "Audio Analysis · Shared", false, |ui| {
                    self.analysis_controls(ui)
                });
                crate::modules::settings_panel(ui, "Stereo Levels", false, |ui| {
                    stereo_meters(ui, frame, &self.theme)
                });
                ui.add_space(8.0);
                ui.label(
                    RichText::new(format!("Theme · {}", self.theme.name))
                        .small()
                        .color(self.theme.muted),
                );
            });
    }

    fn toggle_fullscreen(&mut self, context: &egui::Context) {
        self.fullscreen = !self.fullscreen;
        context.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.fullscreen));
    }

    fn pane(&mut self, ui: &mut egui::Ui, index: usize, rect: Rect, frame: &AnalysisFrame) {
        let selected = self.selected_pane == index;
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 7.0, self.theme.panel);
        painter.rect_stroke(
            rect,
            7.0,
            Stroke::new(
                1.0,
                if selected {
                    self.theme.accent
                } else {
                    mix(self.theme.panel, self.theme.muted, 0.22)
                },
            ),
            egui::StrokeKind::Inside,
        );
        if ui.input(|input| {
            input.pointer.any_pressed()
                && input
                    .pointer
                    .interact_pos()
                    .is_some_and(|pos| rect.contains(pos))
        }) && ui.rect_contains_pointer(rect)
        {
            self.selected_pane = index;
        }
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .id_salt(("pane", index))
                .max_rect(rect.shrink(8.0)),
        );
        child.set_clip_rect(rect.shrink(1.0).intersect(ui.clip_rect()));
        child.spacing_mut().interact_size.y = 24.0;
        child.spacing_mut().button_padding = Vec2::new(6.0, 3.0);
        child.horizontal(|ui| {
            let before = self.panes[index].kind;
            let compact = rect.width() < 250.0;
            egui::ComboBox::from_id_salt("module-kind")
                .width(
                    (ui.available_width() - if compact { 36.0 } else { 78.0 }).clamp(45.0, 180.0),
                )
                .truncate()
                .selected_text(before.label())
                .show_ui(ui, |ui| {
                    for kind in ModuleKind::ALL {
                        ui.selectable_value(&mut self.panes[index].kind, kind, kind.label()).help_text(kind.description());
                    }
                }).response.help_text("Choose which visualization appears in this pane. Each module has its own settings.");
            if before != self.panes[index].kind {
                self.panes[index].reset();
                self.selected_pane = index;
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let focused = self.focused_pane == Some(index);
                if ui
                    .small_button(if compact {
                        "+"
                    } else if focused {
                        "Restore"
                    } else {
                        "Expand"
                    })
                    .help_text(if focused {
                        "Restore dashboard"
                    } else {
                        "Expand pane"
                    })
                    .clicked()
                {
                    self.focused_pane = if focused { None } else { Some(index) };
                    self.selected_pane = index;
                }
            });
        });
        let canvas = Rect::from_min_max(
            Pos2::new(rect.left() + 8.0, child.cursor().top() + 3.0),
            rect.max - Vec2::splat(8.0),
        );
        let live = matches!(self.status, AudioStatus::Streaming)
            && self.selected_node.is_some()
            && frame.sequence > 0;
        self.panes[index].draw(&mut child, canvas, frame, &self.theme, live);
    }

    fn dashboard(&mut self, ui: &mut egui::Ui, rect: Rect, frame: &AnalysisFrame) {
        if let Some(index) = self.focused_pane {
            self.pane(ui, index, rect, frame);
            return;
        }
        let y = rect.top() + rect.height() * self.top_fraction;
        let divider = Rect::from_min_max(
            Pos2::new(rect.left(), y - 5.0),
            Pos2::new(rect.right(), y + 5.0),
        );
        let response = ui
            .interact(divider, ui.id().with("row-split"), Sense::drag())
            .on_hover_cursor(egui::CursorIcon::ResizeVertical)
            .help_text("Drag to resize the main pane and the lower row.");
        if response.dragged() {
            self.top_fraction =
                (self.top_fraction + response.drag_delta().y / rect.height()).clamp(0.35, 0.75);
        }
        if response.hovered() || response.dragged() {
            ui.painter().line_segment(
                [divider.left_center(), divider.right_center()],
                Stroke::new(1.0, self.theme.accent),
            );
        }
        self.pane(
            ui,
            0,
            Rect::from_min_max(rect.min, Pos2::new(rect.right(), y - 5.0)),
            frame,
        );
        let bottom = Rect::from_min_max(Pos2::new(rect.left(), y + 5.0), rect.max);
        for split in 0..2 {
            let x = bottom.left() + bottom.width() * self.bottom_splits[split];
            let handle = Rect::from_min_max(
                Pos2::new(x - 5.0, bottom.top()),
                Pos2::new(x + 5.0, bottom.bottom()),
            );
            let response = ui
                .interact(handle, ui.id().with(("column-split", split)), Sense::drag())
                .on_hover_cursor(egui::CursorIcon::ResizeHorizontal)
                .help_text("Drag to resize the adjacent visualization panes.");
            if response.dragged() {
                let minimum = if split == 0 {
                    0.2
                } else {
                    self.bottom_splits[0] + 0.2
                };
                let maximum = if split == 0 {
                    self.bottom_splits[1] - 0.2
                } else {
                    0.8
                };
                self.bottom_splits[split] = (self.bottom_splits[split]
                    + response.drag_delta().x / bottom.width())
                .clamp(minimum, maximum);
            }
            if response.hovered() || response.dragged() {
                ui.painter().line_segment(
                    [handle.center_top(), handle.center_bottom()],
                    Stroke::new(1.0, self.theme.accent),
                );
            }
        }
        let edges = [0.0, self.bottom_splits[0], self.bottom_splits[1], 1.0];
        for index in 0..3 {
            let left =
                bottom.left() + edges[index] * bottom.width() + if index > 0 { 5.0 } else { 0.0 };
            let right = bottom.left() + edges[index + 1] * bottom.width()
                - if index < 2 { 5.0 } else { 0.0 };
            self.pane(
                ui,
                index + 1,
                Rect::from_min_max(
                    Pos2::new(left, bottom.top()),
                    Pos2::new(right, bottom.bottom()),
                ),
                frame,
            );
        }
    }
}

impl eframe::App for SymphosApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.refresh_theme(ui.ctx());
        help::begin_frame(ui);
        self.process_audio_events();
        if ui.input(|input| input.key_pressed(egui::Key::F11)) {
            self.toggle_fullscreen(ui.ctx());
        }
        if ui.input(|input| input.key_pressed(egui::Key::Escape)) {
            self.focused_pane = None;
        }
        let now = Instant::now();
        let elapsed = now
            .duration_since(self.last_frame)
            .as_secs_f32()
            .max(1.0e-4);
        self.last_frame = now;
        self.display_fps += (elapsed.recip() - self.display_fps) * 0.08;
        let snapshot = self.engine.analysis.snapshot.load_full();
        let rect = ui.available_rect_before_wrap();
        ui.painter().rect_filled(rect, 0.0, self.theme.background);
        let header = Rect::from_min_max(
            rect.min + Vec2::new(12.0, 8.0),
            Pos2::new(rect.right() - 12.0, rect.top() + 48.0),
        );
        let mut nav = ui.new_child(egui::UiBuilder::new().id_salt("navbar").max_rect(header));
        nav.horizontal_centered(|ui| {
            if ui
                .selectable_label(self.sidebar_open, "☰")
                .help_text("Toggle module settings")
                .clicked()
            {
                self.sidebar_open = !self.sidebar_open;
            }
            ui.label(
                RichText::new("SYMPHOS")
                    .size(20.0)
                    .strong()
                    .color(self.theme.foreground),
            );
            ui.add_space(10.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.scope(|ui| {
                    ui.spacing_mut().interact_size.y = 26.0;
                    ui.spacing_mut().button_padding = Vec2::new(6.0, 4.0);
                    ui.style_mut().override_font_id = Some(FontId::proportional(13.0));
                    self.source_selector(ui);
                });
                if ui.small_button("↻").help_text("Refresh audio sources").clicked() {
                    self.engine.refresh_sources();
                }
                ui.menu_button("View", |ui| {
                    if ui.button("Reset pane sizes").help_text("Restore the default dashboard proportions and leave expanded-pane mode.").clicked() {
                        self.top_fraction = 0.62;
                        self.bottom_splits = [1.0 / 3.0, 2.0 / 3.0];
                        self.focused_pane = None;
                        ui.close();
                    }
                    if ui.button("Fullscreen · F11").help_text("Toggle fullscreen. You can also press F11.").clicked() {
                        self.toggle_fullscreen(ui.ctx());
                        ui.close();
                    }
                    ui.checkbox(&mut self.show_inspector, "FFT inspector / diagnostics").help_text("Open raw FFT bins and detailed audio-analysis statistics.");
                }).response.help_text("Dashboard layout, fullscreen, and detailed diagnostics.");
            });
        });
        let footer = Rect::from_min_max(
            Pos2::new(rect.left() + 12.0, rect.bottom() - 32.0),
            rect.max - Vec2::new(12.0, 2.0),
        );
        let mut footer_ui = ui.new_child(egui::UiBuilder::new().id_salt("footer").max_rect(footer));
        footer_ui.spacing_mut().interact_size.y = 22.0;
        footer_ui.horizontal_centered(|ui| {
            ui.label(
                RichText::new(format!("{:.0} FPS", self.display_fps))
                    .monospace()
                    .small()
                    .color(self.theme.muted),
            );
            ui.separator();
            ui.label(
                RichText::new(format!("{:.1} kHz", snapshot.sample_rate as f32 / 1000.0))
                    .monospace()
                    .small()
                    .color(self.theme.muted),
            );
            let peak = snapshot.peak[0].max(snapshot.peak[1]);
            let level = if matches!(self.status, AudioStatus::Streaming) {
                format!("Peak {:.1} dBFS", amplitude_db(peak))
            } else {
                "Peak —".into()
            };
            ui.label(
                RichText::new(level)
                    .monospace()
                    .small()
                    .color(if peak >= 1.0 {
                        self.theme.warning
                    } else {
                        self.theme.muted
                    }),
            );
            if snapshot.dropped_samples > 0 {
                ui.label(
                    RichText::new(format!("{} dropped", snapshot.dropped_samples))
                        .small()
                        .color(self.theme.warning),
                );
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                help::toggle(ui, &mut self.help_open);
                status_badge(ui, &self.status, &self.theme);
            });
        });
        let content = Rect::from_min_max(
            Pos2::new(rect.left() + 12.0, rect.top() + 56.0),
            Pos2::new(rect.right() - 12.0, rect.bottom() - 40.0),
        );
        let layout = workspace_layout(content, self.sidebar_open, self.help_open);
        if let Some(sidebar_rect) = layout.sidebar {
            ui.painter()
                .rect_filled(sidebar_rect, 7.0, self.theme.panel);
            let mut sidebar = ui.new_child(
                egui::UiBuilder::new()
                    .id_salt("sidebar")
                    .max_rect(sidebar_rect.shrink(10.0)),
            );
            sidebar.set_clip_rect(sidebar_rect.intersect(ui.clip_rect()));
            self.sidebar(&mut sidebar, &snapshot);
        }
        self.dashboard(ui, layout.dashboard, &snapshot);
        if self.show_inspector {
            egui::Window::new("FFT inspector / diagnostics")
                .open(&mut self.show_inspector)
                .default_width(540.0)
                .show(ui.ctx(), |ui| {
                    diagnostics(ui, &snapshot);
                    raw_inspector(ui, &snapshot, &self.theme);
                });
        }
        if let Some(help_rect) = layout.help {
            ui.painter().rect_filled(help_rect, 7.0, self.theme.panel);
            help::draw(ui, help_rect, &mut self.help_open);
        }
        let target_rate = self
            .engine
            .analysis
            .settings
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .analysis_fps;
        ui.ctx()
            .request_repaint_after(Duration::from_secs_f32(1.0 / target_rate.max(1) as f32));
    }
}

struct WorkspaceLayout {
    dashboard: Rect,
    sidebar: Option<Rect>,
    help: Option<Rect>,
}

fn default_output_source<'a>(
    sources: &'a [AudioSource],
    selected_node: Option<&str>,
    manually_selected: bool,
) -> Option<&'a AudioSource> {
    if manually_selected {
        return None;
    }
    sources
        .iter()
        .filter(|source| source.kind == SourceKind::SystemOutput)
        .min_by_key(|source| {
            let speakers = source.display_name.to_lowercase().contains("speaker")
                || source.node_name.to_lowercase().contains("speaker");
            (!speakers, &source.node_name)
        })
        .filter(|source| Some(source.node_name.as_str()) != selected_node)
}

fn workspace_layout(content: Rect, sidebar_open: bool, help_open: bool) -> WorkspaceLayout {
    let help = help_open.then(|| {
        Rect::from_min_max(
            Pos2::new(content.left(), content.bottom() - 160.0),
            Pos2::new(content.left() + 236.0, content.bottom()),
        )
    });
    let sidebar = sidebar_open.then(|| {
        Rect::from_min_max(
            content.min,
            Pos2::new(
                content.left() + 236.0,
                help.map_or(content.bottom(), |rect| rect.top() - 10.0),
            ),
        )
    });
    let mut dashboard = content;
    if let Some(sidebar) = sidebar {
        dashboard.min.x = sidebar.right() + 10.0;
    } else if let Some(help) = help {
        // Reserve a bottom strip when settings are hidden; never cover a pane.
        dashboard.max.y = help.top() - 10.0;
    }
    WorkspaceLayout {
        dashboard,
        sidebar,
        help,
    }
}

fn apply_style(context: &egui::Context, theme: &AppTheme) {
    let mut visuals = if theme.dark_mode {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    let border = mix(theme.card, theme.muted, 0.4);
    let selected_text = contrasting_color(theme.accent, theme.foreground, theme.background);
    visuals.override_text_color = Some(theme.foreground);
    visuals.weak_text_color = Some(theme.muted);
    visuals.panel_fill = theme.background;
    visuals.window_fill = theme.card;
    visuals.window_stroke = Stroke::new(1.0, border);
    visuals.window_corner_radius = egui::CornerRadius::same(8);
    visuals.menu_corner_radius = egui::CornerRadius::same(8);
    visuals.extreme_bg_color = theme.background;
    visuals.faint_bg_color = theme.card;
    visuals.code_bg_color = theme.card;
    visuals.hyperlink_color = theme.accent;
    visuals.warn_fg_color = theme.warning;
    visuals.error_fg_color = theme.error;
    visuals.selection.bg_fill = theme.accent;
    visuals.selection.stroke = Stroke::new(1.0, selected_text);
    visuals.widgets.noninteractive.bg_fill = theme.card;
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, theme.foreground);
    visuals.widgets.inactive.bg_fill = theme.card;
    visuals.widgets.inactive.weak_bg_fill = theme.card;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, border);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, theme.foreground);
    visuals.widgets.hovered.bg_fill = theme.selection;
    visuals.widgets.hovered.weak_bg_fill = theme.selection;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, theme.accent);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, theme.foreground);
    visuals.widgets.active.bg_fill = mix(theme.selection, theme.accent, 0.35);
    visuals.widgets.active.weak_bg_fill = visuals.widgets.active.bg_fill;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, theme.accent);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, theme.foreground);
    visuals.widgets.open = visuals.widgets.active;
    context.set_visuals(visuals);
    context.style_mut_of(egui::Theme::Dark, |style| {
        help::suppress_tooltips(style);
        style.spacing.item_spacing = Vec2::new(10.0, 9.0);
        style.spacing.button_padding = Vec2::new(12.0, 8.0);
        style.spacing.menu_margin = egui::Margin::same(8);
        style.spacing.interact_size.y = 32.0;
        style.compact_menu_style = false;
        style
            .text_styles
            .insert(egui::TextStyle::Body, FontId::proportional(15.0));
        style
            .text_styles
            .insert(egui::TextStyle::Button, FontId::proportional(15.0));
        style
            .text_styles
            .insert(egui::TextStyle::Small, FontId::proportional(13.0));
    });
    context.style_mut_of(egui::Theme::Light, |style| {
        help::suppress_tooltips(style);
        style.spacing.item_spacing = Vec2::new(10.0, 9.0);
        style.spacing.button_padding = Vec2::new(12.0, 8.0);
        style.spacing.menu_margin = egui::Margin::same(8);
        style.spacing.interact_size.y = 32.0;
        style.compact_menu_style = false;
        style
            .text_styles
            .insert(egui::TextStyle::Body, FontId::proportional(15.0));
        style
            .text_styles
            .insert(egui::TextStyle::Button, FontId::proportional(15.0));
        style
            .text_styles
            .insert(egui::TextStyle::Small, FontId::proportional(13.0));
    });
}

fn contrasting_color(background: Color32, a: Color32, b: Color32) -> Color32 {
    let brightness = |color: Color32| {
        0.2126 * color.r() as f32 + 0.7152 * color.g() as f32 + 0.0722 * color.b() as f32
    };
    if (brightness(a) - brightness(background)).abs()
        >= (brightness(b) - brightness(background)).abs()
    {
        a
    } else {
        b
    }
}

fn panel(
    ui: &mut egui::Ui,
    title: &str,
    subtitle: &str,
    theme: &AppTheme,
    body: impl FnOnce(&mut egui::Ui),
) {
    egui::Frame::new()
        .fill(theme.panel)
        .corner_radius(10)
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(title)
                        .small()
                        .strong()
                        .color(theme.foreground),
                );
                ui.label(RichText::new(subtitle).small().color(theme.muted));
            });
            ui.add_space(8.0);
            body(ui);
        });
}

fn stereo_meters(ui: &mut egui::Ui, frame: &AnalysisFrame, theme: &AppTheme) {
    ui.label(RichText::new("STEREO LEVEL").small().color(theme.muted));
    for (label, rms, peak, color) in [
        ("L", frame.rms[0], frame.peak[0], theme.accent),
        ("R", frame.rms[1], frame.peak[1], theme.accent_alt),
    ] {
        ui.horizontal(|ui| {
            ui.label(RichText::new(label).monospace().small().color(color));
            let (rect, _) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), 7.0), Sense::hover());
            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, 3.5, theme.background);
            let rms_fraction = ((amplitude_db(rms) + 60.0) / 60.0).clamp(0.0, 1.0);
            painter.rect_filled(
                Rect::from_min_max(
                    rect.min,
                    Pos2::new(rect.left() + rect.width() * rms_fraction, rect.bottom()),
                ),
                3.5,
                color,
            );
            let peak_fraction = ((amplitude_db(peak) + 60.0) / 60.0).clamp(0.0, 1.0);
            let peak_x = rect.left() + rect.width() * peak_fraction;
            painter.line_segment(
                [
                    Pos2::new(peak_x, rect.top() - 1.0),
                    Pos2::new(peak_x, rect.bottom() + 1.0),
                ],
                Stroke::new(1.0, theme.foreground),
            );
        });
    }
}

fn raw_inspector(ui: &mut egui::Ui, frame: &AnalysisFrame, theme: &AppTheme) {
    panel(
        ui,
        "RAW FFT BINS",
        "Linear frequency resolution",
        theme,
        |ui| {
            egui::ScrollArea::vertical()
                .max_height(260.0)
                .show(ui, |ui| {
                    egui::Grid::new("raw-bins")
                        .striped(true)
                        .num_columns(4)
                        .show(ui, |ui| {
                            ui.strong("Bin");
                            ui.strong("Frequency");
                            ui.strong("Magnitude");
                            ui.strong("dBFS");
                            ui.end_row();
                            for (index, bin) in frame.bins.iter().enumerate() {
                                ui.monospace(index.to_string());
                                ui.monospace(format!("{:.2} Hz", bin.frequency_hz));
                                ui.monospace(format!("{:.7}", bin.magnitude));
                                ui.monospace(format!("{:.2}", bin.dbfs));
                                ui.end_row();
                            }
                        });
                });
        },
    );
}

fn status_badge(ui: &mut egui::Ui, status: &AudioStatus, theme: &AppTheme) {
    let color = match status {
        AudioStatus::Streaming => theme.accent,
        AudioStatus::Connecting | AudioStatus::Paused => theme.warning,
        AudioStatus::Error(_) => theme.error,
        AudioStatus::Idle => theme.muted,
    };
    egui::Frame::new()
        .fill(mix(theme.background, color, 0.16))
        .corner_radius(12)
        .inner_margin(egui::Margin::symmetric(9, 4))
        .show(ui, |ui| {
            ui.label(
                RichText::new(format!("●  {}", status.label()))
                    .small()
                    .color(color),
            );
        });
}

fn diagnostics(ui: &mut egui::Ui, frame: &AnalysisFrame) {
    ui.label(format!(
        "{} Hz · {} ch · FFT {} · {:.1} ms window",
        frame.sample_rate, frame.channels, frame.fft_size, frame.latency_ms
    ));
    ui.label(format!(
        "Dominant {:.1} Hz ({}) · centroid {:.1} Hz · rolloff {:.1} Hz",
        frame.dominant_frequency_hz,
        frame.dominant_note,
        frame.spectral_centroid_hz,
        frame.spectral_rolloff_hz
    ));
    ui.label(format!(
        "Flatness {:.3} · zero crossing {:.3} · crest {:.1} dB",
        frame.spectral_flatness, frame.zero_crossing_rate, frame.crest_factor_db
    ));
    ui.label(format!(
        "Tempo {:.0} BPM · {:.0}% confidence · {} log bands",
        frame.bpm,
        frame.bpm_confidence * 100.0,
        frame.log_bands.len()
    ));
    if let Some(band) = frame.log_bands.first() {
        ui.small(format!(
            "First analysis band: {:.1} Hz · {:.1} dBFS",
            band.center_hz, band.dbfs
        ));
    }
}

fn amplitude_db(amplitude: f32) -> f32 {
    20.0 * amplitude.max(1.0e-7).log10()
}

fn mix(a: Color32, b: Color32, amount: f32) -> Color32 {
    let channel = |left: u8, right: u8| {
        (left as f32 + (right as f32 - left as f32) * amount.clamp(0.0, 1.0)) as u8
    };
    Color32::from_rgb(
        channel(a.r(), b.r()),
        channel(a.g(), b.g()),
        channel(a.b(), b.b()),
    )
}

#[cfg(test)]
mod layout_tests {
    use super::*;

    #[test]
    fn defaults_to_speakers_without_overriding_manual_choices() {
        let microphone = AudioSource {
            global_id: 1,
            node_name: "input".into(),
            display_name: "Microphone".into(),
            kind: SourceKind::Input,
        };
        let headphones = AudioSource {
            global_id: 2,
            node_name: "alsa.headphones".into(),
            display_name: "Built-in Audio Headphones".into(),
            kind: SourceKind::SystemOutput,
        };
        let speakers = AudioSource {
            global_id: 3,
            node_name: "audio_effect.j314-convolver".into(),
            display_name: "MacBook Pro J314 Speakers".into(),
            kind: SourceKind::SystemOutput,
        };
        assert!(default_output_source(&[], None, false).is_none());
        assert!(default_output_source(std::slice::from_ref(&microphone), None, false).is_none());
        let mut sources = vec![microphone, headphones];
        assert_eq!(
            default_output_source(&sources, None, false)
                .unwrap()
                .global_id,
            2
        );
        sources.push(speakers);
        assert_eq!(
            default_output_source(&sources, Some("alsa.headphones"), false)
                .unwrap()
                .global_id,
            3
        );
        assert!(
            default_output_source(&sources, Some("audio_effect.j314-convolver"), false).is_none()
        );
        for selected in ["input", "alsa.headphones", "disconnected-manual-source"] {
            assert!(default_output_source(&sources, Some(selected), true).is_none());
        }
        sources.reverse();
        assert_eq!(
            default_output_source(&sources, None, false)
                .unwrap()
                .global_id,
            3
        );
    }

    #[test]
    fn info_view_never_overlaps_dashboard_or_settings() {
        for size in [Vec2::new(1024.0, 700.0), Vec2::new(1440.0, 940.0)] {
            let content = Rect::from_min_max(
                Pos2::new(12.0, 56.0),
                (size - Vec2::new(12.0, 40.0)).to_pos2(),
            );
            for sidebar_open in [false, true] {
                for help_open in [false, true] {
                    let layout = workspace_layout(content, sidebar_open, help_open);
                    assert!(content.contains_rect(layout.dashboard));
                    assert!(layout.dashboard.is_positive());
                    if let Some(help) = layout.help {
                        assert_eq!(help.left_bottom(), content.left_bottom());
                        assert!(!help.intersects(layout.dashboard));
                        if let Some(sidebar) = layout.sidebar {
                            assert!(!help.intersects(sidebar));
                            assert!(sidebar.height() >= 400.0);
                        }
                    }
                    if !sidebar_open && !help_open {
                        assert_eq!(layout.dashboard, content);
                    }
                }
            }
        }
    }
}
