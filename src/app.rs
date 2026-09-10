use std::time::{Duration, Instant};

use eframe::egui::{
    self, Align, Color32, FontId, Layout, Pos2, Rect, RichText, Sense, Stroke, Vec2,
};

use crate::analysis::AnalysisFrame;
use crate::audio::{AudioEngine, AudioEvent, AudioSource, AudioStatus, SourceKind};
use crate::global_bar::{BarInfo, GlobalBar};
use crate::help::{self, HoverHelp};
use crate::icons::{self, Icon};
use crate::modules::{ModuleKind, ModulePane, ModuleSettings};
use crate::theme::AppTheme;

const THEME_REFRESH_INTERVAL: Duration = Duration::from_millis(100);
const DEFAULT_SETTINGS_WIDTH: f32 = 236.0;
const LOGO_HEIGHT: f32 = 26.0;
const LOGO_ASPECT_RATIO: f32 = 1197.0 / 761.0;

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
    show_issues: bool,
    show_shortcuts: bool,
    settings_clipboard: Option<ModuleSettings>,
    display_fps: f32,
    last_frame: Instant,
    panes: [ModulePane; 4],
    selected_pane: usize,
    sidebar_open: bool,
    sidebar_width: f32,
    help_open: bool,
    global_bar: GlobalBar,
    focused_pane: Option<usize>,
    top_fraction: f32,
    bottom_splits: [f32; 2],
}

impl SymphosApp {
    pub fn new(context: &eframe::CreationContext<'_>) -> Self {
        egui_extras::install_image_loaders(&context.egui_ctx);
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
            show_issues: false,
            show_shortcuts: false,
            settings_clipboard: None,
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
            sidebar_width: DEFAULT_SETTINGS_WIDTH,
            help_open: false,
            global_bar: GlobalBar::default(),
            focused_pane: None,
            top_fraction: 0.62,
            bottom_splits: [1.0 / 3.0, 2.0 / 3.0],
        }
    }

    fn reset_modules(&mut self) {
        self.engine.analysis.clear_history();
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
                    if let AudioStatus::Error(message) = &status {
                        crate::issues::record("Audio capture", message);
                    }
                    if matches!(
                        status,
                        AudioStatus::Connecting | AudioStatus::Idle | AudioStatus::Error(_)
                    ) {
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

    fn source_selector(&mut self, ui: &mut egui::Ui) -> egui::Response {
        let (response, choice) =
            audio_source_picker(ui, &self.sources, self.selected_node.as_deref());
        if let Some(source) = choice {
            self.source_manually_selected = true;
            if self.selected_node.as_deref() != Some(source.node_name.as_str()) {
                self.selected_node = Some(source.node_name.clone());
                self.status = AudioStatus::Connecting;
                self.engine.select_source(source);
                self.reset_modules();
            }
        }
        response
    }

    fn sidebar(&mut self, ui: &mut egui::Ui, frame: &AnalysisFrame) {
        ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
        ui.spacing_mut().slider_width = 90.0;
        ui.style_mut()
            .text_styles
            .insert(egui::TextStyle::Body, FontId::proportional(13.0));
        ui.style_mut()
            .text_styles
            .insert(egui::TextStyle::Button, FontId::proportional(13.0));
        ui.spacing_mut().button_padding = Vec2::new(6.0, 3.0);
        ui.spacing_mut().interact_size.y = 22.0;
        ui.spacing_mut().indent = 16.0;
        let field = if self.theme.dark_mode {
            mix(self.theme.panel, Color32::BLACK, 0.24)
        } else {
            mix(self.theme.panel, self.theme.foreground, 0.07)
        };
        let visuals = ui.visuals_mut();
        visuals.collapsing_header_frame = false;
        visuals.extreme_bg_color = field;
        visuals.widgets.inactive.bg_fill = field;
        visuals.widgets.inactive.weak_bg_fill = field;
        visuals.widgets.inactive.bg_stroke = Stroke::NONE;
        visuals.widgets.noninteractive.bg_stroke =
            Stroke::new(1.0, self.theme.foreground.gamma_multiply(0.10));
        for widget in [
            &mut visuals.widgets.inactive,
            &mut visuals.widgets.hovered,
            &mut visuals.widgets.active,
            &mut visuals.widgets.open,
        ] {
            widget.corner_radius = egui::CornerRadius::same(3);
        }
        if settings_header(ui, self.panes[self.selected_pane].kind).clicked() {
            self.sidebar_open = false;
        }
        settings_scroll_area(self.selected_pane, &self.panes[self.selected_pane])
            .show(ui, |ui| {
                ui.push_id(self.selected_pane, |ui| {
                    ui.data_mut(|d| {
                        d.insert_temp(
                            egui::Id::new("parameter-context"),
                            format!(
                                "Pane {} · {} · {}",
                                self.selected_pane + 1,
                                self.panes[self.selected_pane].kind.label(),
                                self.panes[self.selected_pane].active_section().title()
                            ),
                        )
                    });
                    self.panes[self.selected_pane].controls(ui, frame)
                });
                ui.horizontal(|ui| {
                    if ui.small_button("Copy settings").help_text("Copy this module's settings only, including its appearance and camera if present. Does not copy audio, history, pause state, or global settings.").clicked() {
                        self.settings_clipboard = Some(self.panes[self.selected_pane].copy_settings());
                    }
                    let compatible = self.settings_clipboard.as_ref().is_some_and(|settings| settings.kind() == self.panes[self.selected_pane].kind);
                    if ui.add_enabled(compatible, egui::Button::new("Paste settings").small())
                        .help_text("Apply copied settings to another pane using the same module type. Keeps that pane's captured history and global channel selection. Clipboard is session-only.").clicked() {
                        self.paste_module_settings();
                    }
                });
                if let Some(settings) = &self.settings_clipboard { ui.weak(format!("Clipboard: {}", settings.kind().label())); }
            });
    }

    fn toggle_fullscreen(&mut self, context: &egui::Context) {
        self.fullscreen = !self.fullscreen;
        context.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.fullscreen));
    }

    fn handle_shortcut(&mut self, ctx: &egui::Context, frame: &AnalysisFrame, now: Instant) {
        use crate::shortcuts::Action;
        let live = matches!(self.status, AudioStatus::Streaming)
            && self.selected_node.is_some()
            && frame.sequence > 0
            && self.engine.analysis.is_current(frame);
        match crate::shortcuts::take_action(
            ctx,
            self.show_shortcuts || self.show_issues || self.show_inspector,
        ) {
            Some(Action::Pause) if live || self.panes[self.selected_pane].is_frozen() => {
                self.panes[self.selected_pane].toggle_freeze(frame, now)
            }
            Some(Action::PauseAll) => {
                let pause = self.panes.iter().any(|pane| !pane.is_frozen());
                for pane in &mut self.panes {
                    if pane.is_frozen() != pause && (live || pane.is_frozen()) {
                        pane.toggle_freeze(frame, now);
                    }
                }
            }
            Some(Action::Sidebar) => self.sidebar_open = !self.sidebar_open,
            Some(Action::Expand) => {
                self.focused_pane = if self.focused_pane == Some(self.selected_pane) {
                    None
                } else {
                    Some(self.selected_pane)
                }
            }
            Some(Action::Fullscreen) => self.toggle_fullscreen(ctx),
            Some(Action::Restore) => self.focused_pane = None,
            Some(Action::Help) => self.help_open = !self.help_open,
            Some(Action::Bindings) => self.show_shortcuts = true,
            Some(Action::CopySettings) => {
                self.settings_clipboard = Some(self.panes[self.selected_pane].copy_settings())
            }
            Some(Action::PasteSettings) => self.paste_module_settings(),
            Some(Action::Pane(index)) => {
                self.selected_pane = index;
                if self.focused_pane.is_some() {
                    self.focused_pane = Some(index);
                }
            }
            _ => {}
        }
    }

    fn paste_module_settings(&mut self) {
        if let Some(settings) = &self.settings_clipboard {
            if !self.panes[self.selected_pane].paste_settings(settings) {
                crate::issues::record(
                    "Paste settings",
                    &format!(
                        "Clipboard contains {} settings; select a matching module before pasting.",
                        settings.kind().label()
                    ),
                );
            }
        } else {
            crate::issues::record("Paste settings", "No module settings have been copied yet.");
        }
    }

    fn pane(&mut self, ui: &mut egui::Ui, index: usize, rect: Rect, frame: &AnalysisFrame) {
        let live = matches!(self.status, AudioStatus::Streaming)
            && self.selected_node.is_some()
            && frame.sequence > 0
            && self.engine.analysis.is_current(frame);
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
            module_kind_picker(ui, &mut self.panes[index].kind);
            if before != self.panes[index].kind {
                self.panes[index].reset();
                self.selected_pane = index;
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let focused = self.focused_pane == Some(index);
                if icons::sized_button(
                    ui,
                    if focused { Icon::Restore } else { Icon::Fullscreen },
                    false,
                    24.0,
                    if focused { "Restore dashboard" } else { "Expand pane" },
                ).clicked()
                {
                    self.focused_pane = if focused { None } else { Some(index) };
                    self.selected_pane = index;
                }
                let frozen = self.panes[index].is_frozen();
                ui.add_enabled_ui(live || frozen, |ui| {
                    if icons::sized_button(ui, if frozen { Icon::Play } else { Icon::Pause }, frozen, 24.0,
                        if frozen { "Resume this pane. Capture and other panes have kept running; paused time is omitted from this pane's history." }
                        else { "Freeze this pane for inspection. Audio capture and other panes keep running; settings remain editable." }).clicked() {
                        self.panes[index].toggle_freeze(frame, Instant::now());
                        self.selected_pane = index;
                    }
                });
            });
        });
        let canvas = Rect::from_min_max(
            Pos2::new(rect.left() + 8.0, child.cursor().top() + 3.0),
            rect.max - Vec2::splat(8.0),
        );
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
        let now = Instant::now();
        let elapsed = now
            .duration_since(self.last_frame)
            .as_secs_f32()
            .max(1.0e-4);
        self.last_frame = now;
        self.display_fps += (elapsed.recip() - self.display_fps) * 0.08;
        let snapshot = self.engine.analysis.snapshot.load_full();
        self.handle_shortcut(ui.ctx(), &snapshot, now);
        let rect = ui.available_rect_before_wrap();
        ui.painter().rect_filled(rect, 0.0, self.theme.background);
        let header = Rect::from_min_max(
            rect.min + Vec2::new(12.0, 8.0),
            Pos2::new(rect.right() - 12.0, rect.top() + 48.0),
        );
        let mut nav = ui.new_child(egui::UiBuilder::new().id_salt("navbar").max_rect(header));
        app_logo(ui, header, self.theme.foreground);
        nav.horizontal_centered(|ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().interact_size.y = 26.0;
                ui.spacing_mut().button_padding = Vec2::new(6.0, 4.0);
                ui.style_mut().override_font_id = Some(FontId::proportional(13.0));
                let source = self.source_selector(ui);
                let options = icons::sized_button(ui, Icon::Gear, false, source.rect.height(), "View settings: dashboard layout, fullscreen, and detailed diagnostics.");
                egui::Popup::menu(&options).show(|ui| {
                    if ui.button("Keyboard shortcuts · Ctrl+K").clicked() {
                        self.show_shortcuts = true;
                        ui.close();
                    }
                    if ui.button(format!("Issue log ({})", crate::issues::count())).clicked() {
                        self.show_issues = true;
                        ui.close();
                    }
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
                    ui.separator();
                    build_details(ui);
                });
            });
        });
        let footer = Rect::from_min_max(
            Pos2::new(rect.left() + 12.0, rect.bottom() - 42.0),
            rect.max - Vec2::new(12.0, 2.0),
        );
        let mut footer_ui = ui.new_child(egui::UiBuilder::new().id_salt("footer").max_rect(footer));
        let mut settings = self
            .engine
            .analysis
            .settings
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        let reset = self.global_bar.show(
            &mut footer_ui,
            &mut settings,
            &mut self.help_open,
            &mut self.show_inspector,
            BarInfo {
                frame: &snapshot,
                status: &self.status,
                theme: &self.theme,
                fps: self.display_fps,
            },
        );
        *self
            .engine
            .analysis
            .settings
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = settings;
        if reset {
            self.reset_modules();
        }
        // Consume capture for every assigned pane, even when another pane is
        // expanded. The analysis-side archive catches up after window occlusion.
        let after = self
            .panes
            .iter()
            .map(|pane| pane.last_capture)
            .min()
            .unwrap_or(0);
        let captures = self
            .engine
            .analysis
            .capture_history
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .since(after);
        for pane in &mut self.panes {
            pane.ingest(&captures);
        }
        let content = Rect::from_min_max(
            Pos2::new(rect.left() + 12.0, rect.top() + 56.0),
            Pos2::new(rect.right() - 12.0, rect.bottom() - 50.0),
        );
        let initial_layout = workspace_layout(
            content,
            self.sidebar_open,
            self.help_open,
            self.sidebar_width,
        );
        let sidebar_background = ui.painter().add(egui::Shape::Noop);
        settings_rail(
            ui,
            initial_layout.rail,
            &mut self.panes[self.selected_pane],
            &mut self.sidebar_open,
        );
        resize_settings(
            ui,
            content,
            self.sidebar_open,
            self.help_open,
            &mut self.sidebar_width,
        );
        let layout = workspace_layout(
            content,
            self.sidebar_open,
            self.help_open,
            self.sidebar_width,
        );
        let settings_rect = layout
            .sidebar
            .map_or(layout.rail, |sidebar| layout.rail.union(sidebar));
        ui.painter().set(
            sidebar_background,
            egui::Shape::rect_filled(settings_rect, 4.0, self.theme.panel),
        );
        if let Some(sidebar_rect) = layout.sidebar {
            ui.painter().vline(
                sidebar_rect.left(),
                sidebar_rect.y_range(),
                Stroke::new(1.0, self.theme.foreground.gamma_multiply(0.08)),
            );
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
        if self.show_issues {
            let details = format!(
                "Source: {}\nState: {}\nSample rate: {} Hz · channels: {} · FFT: {}\nDropped samples: {} · capture epoch: {}\nPane {} · paused: {}\n{:#?}",
                self.selected_node.as_deref().unwrap_or("None"),
                self.status.label(),
                snapshot.sample_rate,
                snapshot.channels,
                snapshot.fft_size,
                snapshot.dropped_samples,
                snapshot.capture_epoch,
                self.selected_pane + 1,
                self.panes[self.selected_pane].is_frozen(),
                self.panes[self.selected_pane].copy_settings()
            );
            crate::issues::show(ui.ctx(), &mut self.show_issues, &details);
        }
        crate::shortcuts::show(ui.ctx(), &mut self.show_shortcuts);
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

fn app_logo(ui: &mut egui::Ui, header: Rect, tint: Color32) -> egui::Response {
    let rect = app_logo_rect(header);
    ui.put(
        rect,
        egui::Image::new(egui::include_image!("../assets/symphos-logo.svg"))
            .fit_to_exact_size(rect.size())
            .tint(tint)
            .sense(Sense::hover()),
    )
    .on_hover_text("Symphos")
}

fn app_logo_rect(header: Rect) -> Rect {
    let size = Vec2::new(LOGO_HEIGHT * LOGO_ASPECT_RATIO, LOGO_HEIGHT);
    Rect::from_center_size(
        Pos2::new(header.left() + size.x * 0.5, header.center().y),
        size,
    )
}

fn settings_header(ui: &mut egui::Ui, kind: ModuleKind) -> egui::Response {
    ui.horizontal(|ui| {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let accent = ui.visuals().selection.bg_fill;
            ui.visuals_mut().button_frame = false;
            ui.visuals_mut().widgets.hovered.fg_stroke.color = accent;
            ui.visuals_mut().widgets.active.fg_stroke.color = accent;
            let response = icons::sized_button(
                ui,
                Icon::Close,
                false,
                24.0,
                "Close module settings. Click any tab icon to reopen them.",
            );
            response.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Close module settings")
            });
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                ui.add(egui::Label::new(RichText::new(kind.label()).strong()).truncate());
            });
            response
        })
        .inner
    })
    .inner
}

fn settings_scroll_area(pane_index: usize, pane: &ModulePane) -> egui::ScrollArea {
    // Stable per-pane/module/tab identity keeps scroll positions when the panel
    // is closed, resized, or switched to another tab. This is session state only.
    egui::ScrollArea::vertical()
        .id_salt((
            "module-settings",
            pane_index,
            pane.kind.label(),
            pane.active_section().title(),
        ))
        .auto_shrink([false, false])
}

fn build_details(ui: &mut egui::Ui) {
    ui.small(format!("Symphos {}", env!("CARGO_PKG_VERSION")));
    ui.small(format!("Build {}", env!("SYMPHOS_BUILD_ID")));
    if ui.small_button("Copy build info")
        .help_text("Copy the version and source build identifier for issue reports or checking the installed app.")
        .clicked() {
        ui.ctx().copy_text(format!("Symphos {} · build {} · {}/{}", env!("CARGO_PKG_VERSION"), env!("SYMPHOS_BUILD_ID"), std::env::consts::OS, std::env::consts::ARCH));
    }
}

fn module_kind_picker(ui: &mut egui::Ui, kind: &mut ModuleKind) -> egui::Response {
    // ComboBox::width is only a minimum. Constrain its parent so long names
    // truncate before reaching the two 24px action buttons.
    let width =
        (ui.available_width() - 48.0 - 2.0 * ui.spacing().item_spacing.x).clamp(45.0, 180.0);
    ui.scope(|ui| {
        ui.set_width(width);
        egui::ComboBox::from_id_salt("module-kind")
            .width(width)
            .truncate()
            .selected_text(kind.label())
            .show_ui(ui, |ui| {
                ui.set_min_width(180.0);
                for value in ModuleKind::ALL {
                    ui.selectable_value(kind, value, value.label())
                        .help_text(value.description());
                }
            })
            .response
    })
    .inner
    .help_text("Choose which visualization appears in this pane. Each module has its own settings.")
}

fn settings_rail(ui: &mut egui::Ui, rect: Rect, pane: &mut ModulePane, open: &mut bool) {
    let mut rail = ui.new_child(
        egui::UiBuilder::new()
            .id_salt("settings-rail")
            .max_rect(rect.shrink2(Vec2::new(3.0, 4.0))),
    );
    rail.set_clip_rect(rect.intersect(ui.clip_rect()));
    rail.spacing_mut().item_spacing.y = 5.0;
    rail.spacing_mut().button_padding = Vec2::ZERO;
    rail.spacing_mut().interact_size = Vec2::splat(30.0);
    let accent = rail.visuals().selection.bg_fill;
    rail.visuals_mut().button_frame = false;
    rail.visuals_mut().widgets.hovered.fg_stroke.color = accent;
    rail.visuals_mut().widgets.active.fg_stroke.color = accent;
    rail.vertical(|ui| {
        for &section in pane.sections() {
            ui.push_id(section.title(), |ui| {
                let active = *open && pane.active_section() == section;
                if active {
                    ui.visuals_mut().widgets.inactive.fg_stroke.color = accent;
                }
                let response = icons::button(
                    ui,
                    section.icon(),
                    active,
                    &format!(
                        "{} · {} settings. Click to open this tab; click the active tab again to close it.",
                        section.title(),
                        pane.kind.label()
                    ),
                );
                if response.clicked() {
                    pane.select_section(section);
                    *open = !active;
                }
            });
        }
    });
}

struct WorkspaceLayout {
    dashboard: Rect,
    sidebar: Option<Rect>,
    help: Option<Rect>,
    rail: Rect,
}

fn audio_source_picker(
    ui: &mut egui::Ui,
    sources: &[AudioSource],
    selected_node: Option<&str>,
) -> (egui::Response, Option<AudioSource>) {
    let selected = sources
        .iter()
        .find(|source| Some(source.node_name.as_str()) == selected_node);
    let label = selected.map_or_else(
        || {
            if selected_node.is_some() {
                "Source unavailable"
            } else if sources.is_empty() {
                "Looking for outputs…"
            } else {
                "Select an audio source…"
            }
        },
        |source| source.display_name.as_str(),
    );
    let mut choice = None;
    let response = ui.scope(|ui| {
        // ComboBox::width is a minimum, not a maximum. Constrain the child UI
        // so long selected names truncate instead of moving the settings button.
        ui.set_width(260.0);
        egui::ComboBox::from_id_salt("audio-source")
            .width(260.0).height(300.0).truncate().selected_text(label)
            .show_ui(ui, |ui| {
                ui.set_width(360.0);
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                if sources.is_empty() { ui.label("No audio sources available. Devices appear here automatically."); }
                for source in sources {
                    ui.push_id(&source.node_name, |ui| {
                        let selected = Some(source.node_name.as_str()) == selected_node;
                        let text = format!("{}  {}", source.kind.label(), source.display_name);
                        if ui.add(egui::Button::selectable(selected, text).wrap())
                            .help_text_with(|| format!("Analyze {}: {}. Choosing a different source clears the displayed histories.", source.kind.label(), source.display_name)).clicked() {
                            choice = Some(source.clone());
                            ui.close();
                        }
                    });
                }
            }).response
    }).inner.help_text_with(|| format!("Audio source: {label}. Available devices update automatically. Speakers/system output is selected automatically on startup. Choose another output or microphone here; your choice is kept for this session."));
    (response, choice)
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

fn settings_width(content: Rect, requested: f32) -> f32 {
    // Preserve enough space for useful plots even at the minimum window size.
    let maximum = (content.width() - 36.0 - 10.0 - 540.0).clamp(DEFAULT_SETTINGS_WIDTH, 440.0);
    requested.clamp(DEFAULT_SETTINGS_WIDTH, maximum)
}

fn resize_settings(ui: &mut egui::Ui, content: Rect, open: bool, help_open: bool, width: &mut f32) {
    let layout = workspace_layout(content, open, help_open, *width);
    let Some(sidebar) = layout.sidebar else {
        return;
    };
    let handle = Rect::from_min_max(
        Pos2::new(sidebar.right() - 3.0, sidebar.top()),
        Pos2::new(sidebar.right() + 5.0, sidebar.bottom()),
    );
    let response = ui
        .interact(
            handle,
            ui.id().with("settings-resize"),
            Sense::click_and_drag(),
        )
        .on_hover_cursor(egui::CursorIcon::ResizeHorizontal)
        .help_text("Drag to resize module settings. Double-click to restore the compact width.");
    if response.double_clicked() {
        *width = DEFAULT_SETTINGS_WIDTH;
    } else if response.dragged()
        && let Some(pointer) = response.interact_pointer_pos()
    {
        *width = settings_width(content, pointer.x - layout.rail.right());
    }
    if response.hovered() || response.dragged() {
        ui.painter().vline(
            sidebar.right() + 2.0,
            sidebar.y_range(),
            Stroke::new(2.0, ui.visuals().selection.bg_fill),
        );
    }
}

fn workspace_layout(
    content: Rect,
    sidebar_open: bool,
    help_open: bool,
    requested_width: f32,
) -> WorkspaceLayout {
    const RAIL_WIDTH: f32 = 36.0;
    let settings_width = settings_width(content, requested_width);
    let help = help_open.then(|| {
        Rect::from_min_max(
            Pos2::new(content.left(), content.bottom() - 160.0),
            Pos2::new(
                content.left()
                    + if sidebar_open {
                        RAIL_WIDTH + settings_width
                    } else {
                        DEFAULT_SETTINGS_WIDTH
                    },
                content.bottom(),
            ),
        )
    });
    let rail = Rect::from_min_max(
        content.min,
        Pos2::new(
            content.left() + RAIL_WIDTH,
            help.map_or(content.bottom(), |rect| rect.top() - 10.0),
        ),
    );
    let sidebar = sidebar_open.then(|| {
        Rect::from_min_max(
            Pos2::new(rail.right(), content.top()),
            Pos2::new(rail.right() + settings_width, rail.bottom()),
        )
    });
    let mut dashboard = content;
    dashboard.min.x = sidebar.map_or(rail.right(), |rect| rect.right()) + 10.0;
    if !sidebar_open && let Some(help) = help {
        dashboard.max.y = help.top() - 10.0;
    }
    WorkspaceLayout {
        dashboard,
        sidebar,
        help,
        rail,
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
    fn logo_uses_its_source_aspect_ratio_and_anchors_to_header_left() {
        let header = Rect::from_min_max(Pos2::new(12.0, 8.0), Pos2::new(1012.0, 48.0));
        let logo = app_logo_rect(header);

        assert!((logo.left() - header.left()).abs() < 0.01);
        assert!((logo.center().y - header.center().y).abs() < 0.01);
        assert!((logo.height() - LOGO_HEIGHT).abs() < 0.01);
        assert!((logo.width() / logo.height() - LOGO_ASPECT_RATIO).abs() < 0.01);
        assert!(header.contains_rect(logo));
    }

    #[test]
    fn narrow_pane_picker_reserves_space_for_both_action_buttons() {
        for width in [120.0, 160.0, 240.0, 600.0] {
            for mut kind in ModuleKind::ALL {
                let context = egui::Context::default();
                let mut output = context.run_ui(egui::RawInput::default(), |ui| {
                    ui.set_width(width);
                    ui.spacing_mut().item_spacing.x = 10.0;
                    ui.spacing_mut().button_padding = Vec2::new(6.0, 3.0);
                    ui.horizontal(|ui| {
                        let picker = module_kind_picker(ui, &mut kind);
                        let pause = icons::sized_button(ui, Icon::Pause, false, 24.0, "Pause");
                        let expand =
                            icons::sized_button(ui, Icon::Fullscreen, false, 24.0, "Expand");
                        assert!(picker.rect.right() < pause.rect.left());
                        assert!(pause.rect.right() < expand.rect.left());
                        assert!(expand.rect.right() <= ui.max_rect().right() + 0.1);
                    });
                    assert!(ui.min_rect().width() <= width + 0.1);
                });
                output.textures_delta.clear();
            }
        }
    }

    #[test]
    fn source_picker_has_stable_width_wraps_menu_names_and_closes_on_selection() {
        let sources = vec![
            AudioSource { global_id: 1, node_name: "speakers".into(), display_name: "Speakers".into(), kind: crate::audio::SourceKind::SystemOutput },
            AudioSource { global_id: 2, node_name: "long-output".into(), display_name: "External USB Audio Interface With A Very Long Manufacturer And Device Name For Studio Speakers".into(), kind: crate::audio::SourceKind::SystemOutput },
        ];
        let context = egui::Context::default();
        let render = |selected: Option<&str>, events| {
            let mut picker_rect = Rect::NOTHING;
            let mut choice = None;
            let mut output = context.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1024.0, 700.0))),
                    events,
                    ..Default::default()
                },
                |ui| {
                    let mut nav = ui.new_child(egui::UiBuilder::new().max_rect(
                        Rect::from_min_max(Pos2::new(12.0, 8.0), Pos2::new(1012.0, 48.0)),
                    ));
                    nav.horizontal_centered(|ui| {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.spacing_mut().interact_size.y = 26.0;
                            ui.spacing_mut().button_padding = Vec2::new(6.0, 4.0);
                            ui.style_mut().override_font_id = Some(FontId::proportional(13.0));
                            let (response, result) = audio_source_picker(ui, &sources, selected);
                            picker_rect = response.rect;
                            choice = result;
                            let gear = icons::sized_button(
                                ui,
                                Icon::Gear,
                                false,
                                response.rect.height(),
                                "View settings",
                            );
                            assert!((gear.rect.height() - response.rect.height()).abs() < 0.01);
                            assert!((gear.rect.center().y - response.rect.center().y).abs() < 0.01);
                        });
                    });
                },
            );
            output.textures_delta.clear();
            assert!(
                (picker_rect.width() - 260.0).abs() < 0.01,
                "picker {picker_rect:?}"
            );
            assert!((picker_rect.height() - 26.0).abs() < 0.01);
            (output, picker_rect, choice)
        };
        for selected in [
            None,
            Some("speakers"),
            Some("long-output"),
            Some("unavailable"),
        ] {
            render(selected, vec![]);
        }
        let (_, rect, _) = render(Some("speakers"), vec![]);
        let click = |position, pressed| {
            vec![
                egui::Event::PointerMoved(position),
                egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ]
        };
        for pressed in [true, false] {
            render(Some("speakers"), click(rect.center(), pressed));
        }
        let (menu, _, _) = render(Some("speakers"), vec![]);
        let full_label = format!("System output  {}", sources[1].display_name);
        let row = menu
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.job.text == full_label => Some(text),
                _ => None,
            })
            .expect("full device name is visible in the menu");
        assert!(row.galley.size().x <= 360.0);
        assert!(row.pos.x >= 0.0 && row.pos.x + row.galley.size().x <= 1024.0);
        assert!(
            row.galley.rows.len() > 1,
            "long names wrap instead of widening the popup"
        );
        let position = row.pos + row.galley.size() * 0.5;
        render(Some("speakers"), click(position, true));
        let (_, _, choice) = render(Some("speakers"), click(position, false));
        assert_eq!(choice.unwrap().node_name, "long-output");
        let (closed, _, _) = render(Some("long-output"), vec![]);
        assert!(!closed.shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.job.text == full_label)));
    }

    #[test]
    fn sidebar_close_button_collapses_and_icon_tabs_reopen_every_module() {
        for kind in ModuleKind::ALL {
            let context = egui::Context::default();
            let mut pane = ModulePane::new(kind);
            let mut open = true;
            let render = |pane: &mut ModulePane, open: &mut bool, events| {
                let mut close_rect = None;
                let mut output = context.run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(
                            Pos2::ZERO,
                            Vec2::new(1000.0, 700.0),
                        )),
                        events,
                        ..Default::default()
                    },
                    |ui| {
                        settings_rail(
                            ui,
                            Rect::from_min_size(Pos2::new(12.0, 56.0), Vec2::new(36.0, 580.0)),
                            pane,
                            open,
                        );
                        if *open {
                            let mut sidebar = ui.new_child(
                                egui::UiBuilder::new().id_salt("test-sidebar").max_rect(
                                    Rect::from_min_size(
                                        Pos2::new(58.0, 66.0),
                                        Vec2::new(216.0, 560.0),
                                    ),
                                ),
                            );
                            let close = settings_header(&mut sidebar, pane.kind);
                            close_rect = Some(close.rect);
                            if close.clicked() {
                                *open = false;
                            }
                        }
                    },
                );
                output.textures_delta.clear();
                (output, close_rect)
            };
            let (header, close_rect) = render(&mut pane, &mut open, vec![]);
            assert!(header.shapes.iter().any(|shape| matches!(&shape.shape,
                egui::Shape::Text(text) if text.galley.job.text == kind.label())));
            assert!(!header.shapes.iter().any(|shape| matches!(&shape.shape,
                egui::Shape::Text(text) if text.galley.job.text.contains("MODULE SETTINGS") || text.galley.job.text.contains("click a pane"))));
            let close_rect = close_rect.expect("close button in every module settings header");
            assert_eq!(close_rect.size(), Vec2::splat(24.0));
            assert!((close_rect.right() - 274.0).abs() < 0.1);
            assert!((close_rect.top() - 66.0).abs() < 0.1);
            let close = close_rect.center();
            for pressed in [true, false] {
                render(
                    &mut pane,
                    &mut open,
                    vec![
                        egui::Event::PointerMoved(close),
                        egui::Event::PointerButton {
                            pos: close,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                );
            }
            assert!(!open);
            let (_, close_rect) = render(&mut pane, &mut open, vec![]);
            assert!(close_rect.is_none());
            let sections = pane.sections();
            for (index, &section) in sections.iter().enumerate() {
                // Tabs start at the top of the rail, with no separate arrow row.
                let tab = Pos2::new(30.0, 75.0 + index as f32 * 35.0);
                for pressed in [true, false] {
                    render(
                        &mut pane,
                        &mut open,
                        vec![
                            egui::Event::PointerMoved(tab),
                            egui::Event::PointerButton {
                                pos: tab,
                                button: egui::PointerButton::Primary,
                                pressed,
                                modifiers: egui::Modifiers::NONE,
                            },
                        ],
                    );
                }
                assert!(open);
                assert_eq!(pane.active_section(), section);
                let (_, close_rect) = render(&mut pane, &mut open, vec![]);
                assert!(
                    close_rect.is_some(),
                    "close button remains available on every tab"
                );
                for pressed in [true, false] {
                    render(
                        &mut pane,
                        &mut open,
                        vec![
                            egui::Event::PointerMoved(tab),
                            egui::Event::PointerButton {
                                pos: tab,
                                button: egui::PointerButton::Primary,
                                pressed,
                                modifiers: egui::Modifiers::NONE,
                            },
                        ],
                    );
                }
                assert!(!open, "clicking the active tab closes settings");
                assert_eq!(
                    pane.active_section(),
                    section,
                    "closing retains the selected tab"
                );
            }
        }
    }

    #[test]
    fn settings_scroll_positions_survive_closing_switching_and_resizing() {
        for kind in ModuleKind::ALL {
            let context = egui::Context::default();
            let mut pane = ModulePane::new(kind);
            let render = |pane: &ModulePane, pane_index, open, width| {
                let mut scroll = None;
                let mut output = context.run_ui(egui::RawInput::default(), |ui| {
                    if open {
                        let mut sidebar =
                            ui.new_child(egui::UiBuilder::new().id_salt("sidebar").max_rect(
                                Rect::from_min_size(Pos2::ZERO, Vec2::new(width, 160.0)),
                            ));
                        let result =
                            settings_scroll_area(pane_index, pane).show(&mut sidebar, |ui| {
                                ui.allocate_space(Vec2::new(100.0, 1500.0));
                            });
                        scroll = Some((result.id, result.state));
                    }
                });
                output.textures_delta.clear();
                scroll
            };
            let first = pane.active_section();
            let (id, mut state) = render(&pane, 0, true, 216.0).unwrap();
            state.offset.y = 240.0;
            state.store(&context, id);
            assert!(render(&pane, 0, false, 216.0).is_none());
            pane.select_section(pane.sections()[1]);
            let (other_id, other_state) = render(&pane, 0, true, 216.0).unwrap();
            assert_ne!(id, other_id);
            assert_eq!(other_state.offset.y, 0.0);
            pane.select_section(first);
            let (restored_id, restored) = render(&pane, 0, true, 400.0).unwrap();
            assert_eq!(id, restored_id);
            assert_eq!(restored.offset.y, 240.0);
            assert_eq!(
                render(&pane, 1, true, 400.0).unwrap().1.offset.y,
                0.0,
                "another pane keeps its own scroll position"
            );
        }
    }

    #[test]
    fn inspector_resize_drag_and_double_click_reset() {
        let context = egui::Context::default();
        let content = Rect::from_min_size(Pos2::new(12.0, 56.0), Vec2::new(1400.0, 800.0));
        let mut width = DEFAULT_SETTINGS_WIDTH;
        let mut time = 0.0;
        let mut render = |width: &mut f32, events| {
            time += 0.05;
            let mut output = context.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 940.0))),
                    time: Some(time),
                    events,
                    ..Default::default()
                },
                |ui| resize_settings(ui, content, true, false, width),
            );
            output.textures_delta.clear();
        };
        let pointer = |pos, pressed| {
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ]
        };
        render(&mut width, vec![]);
        let start = Pos2::new(48.0 + width, 180.0);
        render(&mut width, pointer(start, true));
        let end = Pos2::new(48.0 + 380.0, 180.0);
        render(&mut width, vec![egui::Event::PointerMoved(end)]);
        render(&mut width, pointer(end, false));
        assert_eq!(width, 380.0);
        for _ in 0..2 {
            render(&mut width, pointer(end, true));
            render(&mut width, pointer(end, false));
        }
        assert_eq!(width, DEFAULT_SETTINGS_WIDTH);
    }

    #[test]
    fn inspector_width_is_bounded_and_keeps_dashboard_usable() {
        for size in [Vec2::new(1000.0, 594.0), Vec2::new(1416.0, 834.0)] {
            let content = Rect::from_min_size(Pos2::new(12.0, 56.0), size);
            for requested in [100.0, DEFAULT_SETTINGS_WIDTH, 360.0, 440.0, 2000.0] {
                for help_open in [false, true] {
                    let layout = workspace_layout(content, true, help_open, requested);
                    let sidebar = layout.sidebar.unwrap();
                    assert!(sidebar.width() >= DEFAULT_SETTINGS_WIDTH);
                    assert!(sidebar.width() <= 440.0);
                    assert!(layout.dashboard.width() >= 540.0);
                    assert!(!sidebar.intersects(layout.dashboard));
                    if let Some(help) = layout.help {
                        assert_eq!(help.right(), sidebar.right());
                        assert!(!help.intersects(layout.dashboard));
                    }
                }
            }
        }
    }

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
                    let layout =
                        workspace_layout(content, sidebar_open, help_open, DEFAULT_SETTINGS_WIDTH);
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
                    assert!(!layout.rail.intersects(layout.dashboard));
                    if let Some(sidebar) = layout.sidebar {
                        assert_eq!(
                            layout.rail.right(),
                            sidebar.left(),
                            "one continuous settings panel without a gutter"
                        );
                        assert_eq!(layout.rail.top(), sidebar.top());
                        assert_eq!(layout.rail.bottom(), sidebar.bottom());
                        if let Some(help) = layout.help {
                            assert_eq!(help.right(), sidebar.right());
                        }
                    }
                    if let Some(help) = layout.help {
                        assert!(!layout.rail.intersects(help));
                    }
                    if !sidebar_open && !help_open {
                        assert_eq!(layout.dashboard.left(), layout.rail.right() + 10.0);
                        assert_eq!(layout.dashboard.right_bottom(), content.right_bottom());
                    }
                }
            }
        }
    }
}
