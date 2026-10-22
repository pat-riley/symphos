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
use crate::theme::{AppTheme, AppearancePreferences};

mod history;
mod menu;
mod scene_library;
mod settings;
use crate::workspace::{self, Axis, Edge, MAX_PANES, PresetContent, Session, Storage, Tile};
use crate::workspace_ui::{self, DragPayload, Manager, Scope};

const THEME_REFRESH_INTERVAL: Duration = Duration::from_millis(100);
const DEFAULT_SETTINGS_WIDTH: f32 = 236.0;

pub struct SymphosApp {
    history: history::History,
    performance: crate::performance::Monitor,
    frame_pacer: crate::performance::Pacer,
    recording: crate::recording::Recorder,
    #[cfg(feature = "development")]
    design_system: crate::development::gallery::Gallery,
    midi_runtime: Option<crate::midi::Runtime>,
    midi_controller: crate::midi::mapping::Controller,
    signal_runtime: Option<crate::signals::Runtime>,
    signal_ui: crate::signals::ui::Shared,
    project_dialog: crate::project::Dialog,
    scenes: crate::scenes::Book,
    saved_project: Option<Vec<u8>>,
    scene_browser: crate::scenes::Browser,
    scene_library: crate::scenes::library::Library,
    pending_scene: Option<crate::scenes::Action>,
    engine: AudioEngine,
    #[cfg(feature = "development")]
    development: Option<crate::development::Development>,
    sources: Vec<AudioSource>,
    selected_node: Option<String>,
    source_manually_selected: bool,
    status: AudioStatus,
    theme: AppTheme,
    automatic_theme: AppTheme,
    automatic_scale: f32,
    appearance: AppearancePreferences,
    pending_zoom: Option<f32>,
    settings_window: settings::SettingsWindow,
    last_theme_check: Instant,
    fullscreen: bool,
    panel_fullscreen: Option<usize>,
    fullscreen_macros: bool,
    fullscreen_hint_started: Option<Instant>,
    show_inspector: bool,
    show_issues: bool,
    show_shortcuts: bool,
    settings_clipboard: Option<ModuleSettings>,
    display_fps: f32,
    render_fps: u32,
    last_frame: Instant,
    panes: Vec<ModulePane>,
    scene_macros: crate::macros::scene::SceneMacros,
    tiles: Option<Tile>,
    storage: Storage,
    manager: Manager,
    context: Option<egui::Context>,
    pending_close: Option<usize>,
    selected_pane: usize,
    sidebar_open: bool,
    sidebar_width: f32,
    library_width: f32,
    signals_height: f32,
    inspecting_signal: bool,
    inspecting_macro: Option<usize>,
    help_open: bool,
    global_bar: GlobalBar,
}

impl SymphosApp {
    pub fn new(context: &eframe::CreationContext<'_>) -> Self {
        crate::visuals::install(context);
        let theme = AppTheme::load_omarchy();
        apply_style(&context.egui_ctx, &theme);
        let engine = AudioEngine::start();
        engine.refresh_sources();
        let dir = workspace::directory();
        let session = Session::load(&dir);
        let midi_runtime = crate::midi::Runtime::start();
        let signal_runtime = Some(crate::signals::Runtime::start(
            engine.analysis().snapshot.clone(),
            midi_runtime.snapshot.clone(),
        ));
        let mut app = Self {
            midi_runtime: Some(midi_runtime),
            signal_runtime,
            midi_controller: Default::default(),
            signal_ui: Default::default(),
            scene_macros: Default::default(),
            project_dialog: Default::default(),
            scenes: Default::default(),
            saved_project: None,
            history: Default::default(),
            performance: Default::default(),
            frame_pacer: Default::default(),
            recording: crate::recording::Recorder::new(context.wgpu_render_state.clone()),
            #[cfg(feature = "development")]
            design_system: Default::default(),
            scene_browser: Default::default(),
            scene_library: Default::default(),
            pending_scene: None,
            engine,
            #[cfg(feature = "development")]
            development: crate::development::Development::new(),
            sources: Vec::new(),
            selected_node: None,
            source_manually_selected: false,
            status: AudioStatus::Idle,
            automatic_theme: theme.clone(),
            automatic_scale: 1.0,
            appearance: AppearancePreferences::default(),
            pending_zoom: None,
            settings_window: settings::SettingsWindow::default(),
            theme,
            last_theme_check: Instant::now(),
            fullscreen: false,
            panel_fullscreen: None,
            fullscreen_macros: false,
            fullscreen_hint_started: None,
            show_inspector: false,
            show_issues: false,
            show_shortcuts: false,
            settings_clipboard: None,
            display_fps: 0.0,
            render_fps: 60,
            last_frame: Instant::now(),
            panes: vec![
                ModulePane::new(ModuleKind::Waterfall),
                ModulePane::new(ModuleKind::Spectrum),
                ModulePane::new(ModuleKind::Waveform),
                ModulePane::new(ModuleKind::Spectrogram),
            ],
            selected_pane: 0,
            sidebar_open: true,
            sidebar_width: DEFAULT_SETTINGS_WIDTH,
            library_width: workspace_ui::LIBRARY_WIDTH,
            signals_height: workspace::default_signals_height(),
            inspecting_signal: false,
            inspecting_macro: None,
            help_open: false,
            global_bar: GlobalBar::default(),
            tiles: Some(Tile::default_view()),
            storage: Storage::new(dir),
            manager: Manager::default(),
            context: None,
            pending_close: None,
        };
        if let Some(state) = session {
            app.scene_macros = state.macros.clone().unwrap_or_else(|| {
                crate::macros::scene::SceneMacros::from_legacy(&state.panes, state.selected_pane)
            });
            app.scenes = state.scenes.clone().unwrap_or_default();
            app.project_dialog.current_path = state.project_path.clone();
            if let Some(path) = &state.project_path {
                app.project_dialog.path = path.display().to_string();
                app.saved_project = crate::project::Project::open(path)
                    .ok()
                    .and_then(|p| serde_json::to_vec(&p).ok());
            }
            app.signal_ui
                .lock()
                .unwrap()
                .replace_bank(state.signals.clone());
            app.panes = state
                .panes
                .iter()
                .map(|state| {
                    let mut pane = ModulePane::new(state.kind);
                    pane.restore(state);
                    pane
                })
                .collect();
            app.tiles = state.layout;
            app.selected_pane = state.selected_pane;
            app.sidebar_open = state.sidebar_open;
            app.sidebar_width = state.sidebar_width;
            app.library_width = state.library_width;
            app.signals_height = state.signals_height;
            app.help_open = state.help_open;
            app.storage.active = state.active_preset;
            *app.engine
                .analysis()
                .settings
                .lock()
                .unwrap_or_else(|error| error.into_inner()) = state.analysis;
            app.render_fps = state.render_fps;
            app.appearance = state.appearance;
            app.global_bar.stereo = state.meter_stereo;
            app.global_bar.channel = state.meter_channel;
        }
        app.scenes.store(app.capture_scene());
        app.apply_appearance(&context.egui_ctx);
        app
    }

    fn workspace(&self, ctx: &egui::Context) -> Session {
        let zoom = ctx.zoom_factor();
        let (size, position) = ctx.input(|input| {
            let viewport = input.viewport();
            (
                viewport.inner_rect.map(|rect| {
                    [
                        (rect.width() * zoom).round(),
                        (rect.height() * zoom).round(),
                    ]
                }),
                viewport
                    .outer_rect
                    .map(|rect| [(rect.left() * zoom).round(), (rect.top() * zoom).round()]),
            )
        });
        let mut state = Session {
            macros: Some(self.scene_macros.clone()),
            project_path: self.project_dialog.current_path.clone(),
            scenes: None,
            signals: self
                .signal_ui
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .bank
                .clone(),
            version: 2,
            appearance: self.appearance.clone(),
            render_fps: self.render_fps,
            demo: {
                #[cfg(feature = "development")]
                {
                    self.development.as_ref().is_some_and(|dev| dev.demo)
                }
                #[cfg(not(feature = "development"))]
                {
                    false
                }
            },
            panes: self.panes.iter().map(ModulePane::snapshot).collect(),
            layout: self.tiles.clone(),
            active_preset: self.storage.active.clone(),
            selected_pane: self.selected_pane,
            focused_pane: None,
            sidebar_open: self.sidebar_open,
            sidebar_width: self.sidebar_width,
            library_width: self.library_width,
            signals_height: self.signals_height,
            help_open: self.help_open,
            analysis: self
                .engine
                .analysis()
                .settings
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .clone(),
            meter_stereo: self.global_bar.stereo,
            meter_channel: self.global_bar.channel,
            window_size: size,
            window_position: position,
        };
        let mut scenes = self.scenes.clone();
        scenes.store(crate::project::SceneState::capture(state.clone()));
        state.scenes = Some(scenes);
        state
    }

    fn capture_scene(&self) -> crate::project::SceneState {
        crate::project::SceneState {
            macros: Some(self.scene_macros.clone()),
            signals: self
                .signal_ui
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .bank
                .clone(),
            panes: self.panes.iter().map(ModulePane::snapshot).collect(),
            layout: self.tiles.clone(),
            selected_pane: self.selected_pane,
            analysis: self
                .engine
                .analysis()
                .settings
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone(),
        }
    }

    fn apply_scene(&mut self, state: crate::project::SceneState) {
        self.recording.stop();
        if let Some(midi) = &self.midi_runtime {
            self.midi_controller.reset(&midi.snapshot.load());
        }
        self.inspecting_macro = None;
        self.scene_macros = state.macros.clone().unwrap_or_else(|| {
            crate::macros::scene::SceneMacros::from_legacy(&state.panes, state.selected_pane)
        });
        if let Some(ctx) = &self.context {
            crate::macros::clear_clipboard(ctx);
        }
        self.panes = state
            .panes
            .iter()
            .map(|state| {
                let mut pane = ModulePane::new(state.kind);
                pane.restore(state);
                pane
            })
            .collect();
        self.tiles = state.layout;
        self.selected_pane = state.selected_pane;
        self.signal_ui
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .replace_bank(state.signals);
        // Detector/envelope state belongs to the scene; the underlying audio engine
        // and physical input connection remain running throughout the switch.
        if self.signal_runtime.is_some() {
            self.signal_runtime = Some(crate::signals::Runtime::start(
                self.engine.analysis().snapshot.clone(),
                self.midi_runtime.as_ref().unwrap().snapshot.clone(),
            ));
        }
        *self
            .engine
            .analysis()
            .settings
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = state.analysis;
        self.panel_fullscreen = None;
        self.pending_close = None;
        self.storage.active = "custom".into();
        self.storage.modified = false;
        self.scene_browser = Default::default();
    }

    fn scene_action(&mut self, action: crate::scenes::Action) {
        use crate::scenes::Action;
        self.scenes.store(self.capture_scene());
        let state = match action {
            Action::Switch(id) => {
                if id == self.scenes.active_scene {
                    return;
                }
                self.scenes.switch(id)
            }
            Action::New => {
                let state = crate::project::SceneState {
                    macros: Some(Default::default()),
                    signals: Default::default(),
                    panes: vec![ModulePane::new(ModuleKind::Waterfall).snapshot()],
                    layout: None,
                    selected_pane: 0,
                    analysis: self.capture_scene().analysis,
                };
                let name = format!("Scene {}", self.scenes.next_scene_id);
                self.scenes.insert(name, state.clone()).map(|_| state)
            }
            Action::Duplicate => {
                let Some(scene) = self.scenes.current().cloned() else {
                    return;
                };
                let name = format!("{} copy", scene.name.chars().take(24).collect::<String>());
                self.scenes
                    .insert(name, scene.state.clone())
                    .map(|_| scene.state)
            }
            Action::Rename(id, name) => {
                self.scenes.rename(id, &name);
                None
            }
            Action::Move(id, step) => {
                self.scenes.move_by(id, step);
                None
            }
            Action::Delete(id) => self.scenes.remove(id),
            Action::Library => {
                self.request_scene_preview(crate::scenes::library::CapturePurpose::Open);
                None
            }
            Action::SavePreset => {
                self.request_scene_preview(crate::scenes::library::CapturePurpose::Save);
                None
            }
            Action::Presets => {
                self.manager.open_presets(Scope::View);
                None
            }
        };
        if let Some(state) = state {
            self.apply_scene(state);
        }
    }

    fn project_changed(&self, ctx: &egui::Context) -> bool {
        let current = crate::project::Project::capture(self.workspace(ctx));
        self.saved_project
            .as_ref()
            .is_none_or(|saved| serde_json::to_vec(&current).as_ref().ok() != Some(saved))
    }
    fn new_project(&mut self, ctx: &egui::Context) {
        self.history = Default::default();
        let state = crate::project::SceneState {
            macros: Some(Default::default()),
            signals: Default::default(),
            panes: vec![ModulePane::new(ModuleKind::Waterfall).snapshot()],
            layout: None,
            selected_pane: 0,
            analysis: Default::default(),
        };
        self.scene_library.reset_project();
        self.scenes = crate::scenes::Book::new(state.clone());
        self.apply_scene(state);
        self.project_dialog = Default::default();
        self.saved_project =
            serde_json::to_vec(&crate::project::Project::capture(self.workspace(ctx))).ok();
    }
    fn confirm_project_replacement(&mut self, ctx: &egui::Context) {
        if !self.project_dialog.pending_new && self.project_dialog.pending_open.is_none() {
            return;
        }
        if self.project_dialog.waiting_for_save {
            if !self.project_changed(ctx) {
                self.finish_project_replacement(ctx);
                return;
            }
            if self.project_dialog.open {
                return;
            }
            self.project_dialog.waiting_for_save = false;
        }
        let mut choice = 0;
        egui::Modal::new(egui::Id::new("replace-project")).show(ctx, |ui| {
            ui.set_width(380.0);
            ui.heading("Save project changes?");
            ui.label("This replaces the current project and all of its scenes.");
            ui.horizontal(|ui| {
                if ui.button("Save all scenes").clicked() {
                    choice = 1;
                }
                if ui.button("Discard changes").clicked() {
                    choice = 2;
                }
                if ui.button("Cancel").clicked() {
                    choice = 3;
                }
            });
        });
        if choice == 1 {
            self.project_action(ctx, crate::project_menu::Action::Save);
            self.project_dialog.waiting_for_save = self.project_dialog.open;
        }
        if choice == 2 || (choice == 1 && !self.project_changed(ctx)) {
            self.finish_project_replacement(ctx);
        } else if choice == 3 {
            self.project_dialog.pending_open = None;
            self.project_dialog.pending_new = false;
            self.project_dialog.waiting_for_save = false;
        }
    }
    fn finish_project_replacement(&mut self, ctx: &egui::Context) {
        self.project_dialog.waiting_for_save = false;
        self.project_dialog.open = false;
        let path = self.project_dialog.pending_open.take();
        let new = std::mem::take(&mut self.project_dialog.pending_new);
        if new {
            self.new_project(ctx);
        } else if let Some(path) = path {
            self.open_project(&path);
        }
    }

    fn project_controls(&mut self, ctx: &egui::Context) {
        self.confirm_project_replacement(ctx);
        if !self.project_dialog.open {
            return;
        }
        if self.project_dialog.path.is_empty() {
            self.project_dialog.path =
                std::fs::read_to_string(self.storage.dir.join("recent-project.txt"))
                    .unwrap_or_else(|_| {
                        self.storage
                            .dir
                            .join("Projects/Untitled.symphos.json")
                            .display()
                            .to_string()
                    });
        }
        let mut save = false;
        let mut open = false;
        egui::Window::new("Project").collapsible(false).default_width(560.0).open(&mut self.project_dialog.open).show(ctx,|ui| {
            ui.label("Save every scene, including its signals, mappings, visuals and macros.");
            ui.small("GUI and device preferences stay on this computer.");
            ui.label("Project file (.symphos.json)");
            ui.add(egui::TextEdit::singleline(&mut self.project_dialog.path).desired_width(f32::INFINITY));
            ui.horizontal(|ui| {
                if ui.button("Save project").clicked() {
                    if std::path::Path::new(&self.project_dialog.path).exists(){self.project_dialog.confirm_save=true;}else{save=true;}
                }
                if ui.button("Open project…").clicked(){self.project_dialog.confirm_open=true;}
            });
            if self.project_dialog.confirm_save {
                ui.label("Replace the existing file with this creative setup?");
                ui.horizontal(|ui| {if ui.button("Replace file").clicked(){save=true;self.project_dialog.confirm_save=false;}
if ui.button("Cancel save").clicked(){self.project_dialog.confirm_save=false;}});
            }
            if self.project_dialog.confirm_open {
                ui.label("Opening replaces the current creative setup. Save it first if you want to keep it.");
                ui.horizontal(|ui| {if ui.button("Open and replace workspace").clicked(){open=true;self.project_dialog.confirm_open=false;}
if ui.button("Cancel open").clicked(){self.project_dialog.confirm_open=false;}});
            }
            if !self.project_dialog.message.is_empty(){ui.label(&self.project_dialog.message);}
        });
        let path = std::path::PathBuf::from(&self.project_dialog.path);
        if save {
            self.save_project(ctx, &path);
        }
        if open {
            self.open_project(&path);
        }
    }

    fn save_project(&mut self, ctx: &egui::Context, path: &std::path::Path) {
        let project = crate::project::Project::capture(self.workspace(ctx));
        match project.save(path) {
            Ok(()) => {
                self.saved_project = serde_json::to_vec(&project).ok();
                self.project_dialog.message = "All scenes saved.".into();
                self.project_dialog.current_path = Some(path.into());
                self.remember_project(path);
            }
            Err(e) => {
                self.project_dialog.message = e;
                self.project_dialog.open = true;
            }
        }
    }
    fn open_project(&mut self, path: &std::path::Path) {
        match crate::project::Project::open(path) {
            Ok(project) => {
                self.history = Default::default();
                self.saved_project = serde_json::to_vec(&project).ok();
                self.scene_library.reset_project();
                self.scenes = project.scenes;
                let state = self
                    .scenes
                    .current()
                    .expect("validated project has an active scene")
                    .state
                    .clone();
                self.apply_scene(state);
                self.project_dialog.message = "Project opened.".into();
                self.project_dialog.current_path = Some(path.into());
                self.remember_project(path);
            }
            Err(e) => {
                self.project_dialog.message = e;
                self.project_dialog.open = true;
            }
        }
    }
    fn remember_project(&mut self, path: &std::path::Path) {
        self.project_dialog.path = path.display().to_string();
        let _ = workspace::atomic_write(
            &self.storage.dir.join("recent-project.txt"),
            self.project_dialog.path.as_bytes(),
        );
    }
    fn project_action(&mut self, ctx: &egui::Context, action: crate::project_menu::Action) {
        if action == crate::project_menu::Action::Settings {
            self.settings_window.open = true;
            return;
        }
        if action == crate::project_menu::Action::New {
            if self.project_changed(ctx) {
                self.project_dialog.pending_new = true;
            } else {
                self.new_project(ctx);
            }
            return;
        }
        if action == crate::project_menu::Action::Save
            && let Some(path) = self.project_dialog.current_path.clone()
        {
            self.save_project(ctx, &path);
            return;
        }
        #[cfg(target_os = "macos")]
        {
            use crate::project_menu::Action;
            let save = action != Action::Open;
            let initial = if self.project_dialog.path.is_empty() {
                self.storage
                    .dir
                    .join("Projects/Untitled.symphos.json")
                    .display()
                    .to_string()
            } else {
                self.project_dialog.path.clone()
            };
            if let Some(path) = crate::project_menu::choose(save, &initial) {
                if save {
                    self.save_project(ctx, &path);
                } else if self.project_changed(ctx) {
                    self.project_dialog.pending_open = Some(path);
                } else {
                    self.open_project(&path);
                }
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = ctx;
            self.project_dialog.open = true;
            self.project_dialog.message = match action {
                crate::project_menu::Action::Open => "Choose a project file to open.",
                _ => "Choose where to save this project.",
            }
            .into();
        }
    }

    fn reset_capture(&mut self) {
        self.engine.analysis().clear_history();
        for pane in &mut self.panes {
            pane.reset_capture();
        }
    }
    fn reset_modules(&mut self) {
        self.engine.analysis().clear_history();
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
                        self.reset_capture();
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
            self.reset_capture();
            self.engine.select_source(source);
        }
    }

    fn refresh_theme(&mut self, context: &egui::Context) {
        #[cfg(feature = "development")]
        if self.development.is_some() {
            return;
        }
        if self.last_theme_check.elapsed() < THEME_REFRESH_INTERVAL {
            return;
        }
        self.last_theme_check = Instant::now();
        let Some(theme) = AppTheme::load_current_omarchy() else {
            return;
        };
        self.automatic_theme = theme;
        self.apply_appearance(context);
    }

    fn apply_appearance(&mut self, context: &egui::Context) {
        let theme = self.appearance.theme.resolve(&self.automatic_theme);
        crate::window_chrome::apply(&theme);
        if self.theme != theme {
            self.theme = theme;
            apply_style(context, &self.theme);
            context.request_repaint();
        }
        let scale = self.appearance.ui_scale.unwrap_or(self.automatic_scale);
        if (context.zoom_factor() - scale).abs() > 0.001 {
            context.set_zoom_factor(scale);
        }
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
                self.reset_capture();
            }
        }
        response
    }

    fn sidebar(&mut self, ui: &mut egui::Ui, frame: &AnalysisFrame) {
        workspace_ui::sidebar_style(ui, &self.theme);
        ui.data_mut(|data| data.insert_temp(egui::Id::new("inspector-bounds"), ui.max_rect()));
        let state = if self.panes[self.selected_pane].is_frozen() {
            "Paused"
        } else if matches!(self.status, AudioStatus::Streaming) {
            "Live"
        } else {
            "Waiting"
        };
        let mut metadata = format!("Panel {} · {state}", self.selected_pane + 1);
        if frame.sample_rate > 0 {
            metadata.push_str(&format!(
                " · {} kHz",
                egui::emath::format_with_decimals_in_range(
                    frame.sample_rate as f64 / 1000.0,
                    0..=1
                )
            ));
        }
        if settings_header(ui, self.panes[self.selected_pane].kind, &metadata).clicked() {
            self.sidebar_open = false;
        }
        egui::Frame::new()
            .inner_margin(egui::Margin::symmetric(8, 4))
            .show(ui, |ui| {
                ui.menu_button("Component actions", |ui| {
                    if ui.button("Browse component presets…").clicked() {
                        self.manager.open_presets(Scope::Component);
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Copy settings").clicked() {
                        self.settings_clipboard =
                            Some(self.panes[self.selected_pane].export_settings());
                        ui.close();
                    }
                    let compatible = self
                        .settings_clipboard
                        .as_ref()
                        .is_some_and(|s| s.kind() == self.panes[self.selected_pane].kind);
                    if ui
                        .add_enabled(compatible, egui::Button::new("Paste settings"))
                        .clicked()
                    {
                        self.paste_module_settings();
                        ui.close();
                    }
                    ui.separator();
                    if ui
                        .button("Reset component")
                        .help_text("Restore component defaults. Undo is available from Edit.")
                        .clicked()
                    {
                        self.panes[self.selected_pane].reset_settings();
                        ui.close();
                    }
                });
            });
        let bounds = ui.available_rect_before_wrap();
        let content_rect = workspace_ui::inspector_rail(
            ui,
            bounds,
            &self.theme,
            ("inspector-tabs", self.selected_pane),
            |rail| {
                self.panes[self.selected_pane].inspector_tabs(rail);
            },
        );
        let mut properties = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("inspector-properties")
                .max_rect(content_rect),
        );
        properties.set_clip_rect(content_rect.intersect(ui.clip_rect()));
        properties.data_mut(|d| d.insert_temp(egui::Id::new("inspector-bounds"), content_rect));
        settings_scroll_area(self.selected_pane, &self.panes[self.selected_pane]).show(
            &mut properties,
            |ui| {
                ui.push_id(self.selected_pane, |ui| {
                    ui.data_mut(|d| {
                        d.insert_temp(
                            egui::Id::new("parameter-context"),
                            format!(
                                "Pane {} · {}",
                                self.selected_pane + 1,
                                self.panes[self.selected_pane].kind.label()
                            ),
                        )
                    });
                    self.panes[self.selected_pane].inspector_controls(ui, frame)
                });
            },
        );
        ui.data_mut(|data| {
            data.remove::<Rect>(egui::Id::new("inspector-bounds"));
            data.remove::<crate::macros::InspectorIndicators>(egui::Id::new(
                crate::macros::INDICATORS,
            ));
            data.remove::<crate::macros::ParameterContext>(egui::Id::new(crate::macros::CONTEXT));
        });
    }

    fn toggle_fullscreen(&mut self, context: &egui::Context) {
        self.fullscreen = !self.fullscreen;
        context.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.fullscreen));
    }

    fn toggle_panel_fullscreen(&mut self, index: usize, now: Instant) {
        self.fullscreen_macros = false;
        self.panel_fullscreen = if self.panel_fullscreen == Some(index) {
            None
        } else {
            Some(index)
        };
        self.fullscreen_hint_started = self.panel_fullscreen.map(|_| now);
        self.selected_pane = index;
    }

    fn inspect_panel(&mut self, index: usize) {
        self.inspecting_signal = false;
        self.inspecting_macro = None;
        self.selected_pane = index;
        self.sidebar_open = true;
        self.panel_fullscreen = None;
        self.fullscreen_hint_started = None;
    }

    fn handle_shortcut(&mut self, ctx: &egui::Context, frame: &AnalysisFrame, now: Instant) {
        use crate::shortcuts::Action;
        if self.scene_library.busy() || self.settings_window.open {
            return; // The modal owns Escape and blocks workspace shortcuts.
        }
        // Always provide an exit even if a hidden inspector or dialog held focus.
        if self.panel_fullscreen.is_some()
            && ctx.input_mut(|input| {
                input.focused && input.consume_key(egui::Modifiers::NONE, egui::Key::Escape)
            })
        {
            self.panel_fullscreen = None;
            self.fullscreen_hint_started = None;
            egui::Popup::close_all(ctx);
            return;
        }
        if self.tiles.is_none() {
            match crate::shortcuts::take_action(ctx, self.manager.modal_open()) {
                Some(Action::Fullscreen) => self.toggle_fullscreen(ctx),
                Some(Action::Help) => self.help_open = !self.help_open,
                Some(Action::Sidebar) => self.sidebar_open = !self.sidebar_open,
                Some(Action::Performance) => self.toggle_macros(),
                Some(Action::Bindings) => self.show_shortcuts = true,
                Some(Action::Settings) => self.settings_window.open = true,
                _ => {}
            }
            return;
        }
        let live = matches!(self.status, AudioStatus::Streaming)
            && self.selected_node.is_some()
            && frame.sequence > 0
            && self.engine.analysis().is_current(frame);
        match crate::shortcuts::take_action(
            ctx,
            self.show_shortcuts
                || self.show_issues
                || self.show_inspector
                || self.manager.modal_open(),
        ) {
            Some(Action::Pause) if live || self.panes[self.selected_pane].is_frozen() => {
                self.panes[self.selected_pane].toggle_freeze(frame, now)
            }
            Some(Action::PauseAll) => {
                let ids = self.open_panes();
                let pause = ids.iter().any(|id| !self.panes[*id].is_frozen());
                for id in ids {
                    let pane = &mut self.panes[id];
                    if pane.is_frozen() != pause && (live || pane.is_frozen()) {
                        pane.toggle_freeze(frame, now);
                    }
                }
            }
            Some(Action::Performance) => self.toggle_macros(),
            Some(Action::Sidebar) => self.sidebar_open = !self.sidebar_open,
            Some(Action::Inspect) => self.inspect_panel(self.selected_pane),
            Some(Action::PanelFullscreen) => self.toggle_panel_fullscreen(self.selected_pane, now),
            Some(Action::Fullscreen) => self.toggle_fullscreen(ctx),
            Some(Action::Help) => self.help_open = !self.help_open,
            Some(Action::Bindings) => self.show_shortcuts = true,
            Some(Action::Settings) => self.settings_window.open = true,
            Some(Action::CopySettings) => {
                self.settings_clipboard = Some(self.panes[self.selected_pane].export_settings())
            }
            Some(Action::PasteSettings) => self.paste_module_settings(),
            Some(Action::Pane(index)) if index < self.open_panes().len() => {
                let index = self.open_panes()[index];
                self.selected_pane = index;
                if self.panel_fullscreen.is_some() {
                    self.panel_fullscreen = Some(index);
                    self.fullscreen_hint_started = Some(now);
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
        let pane_started = self.performance.open.then(Instant::now);
        let live = matches!(self.status, AudioStatus::Streaming)
            && self.selected_node.is_some()
            && frame.sequence > 0
            && self.engine.analysis().is_current(frame);
        let selected = self.selected_pane == index;
        let painter = ui.painter_at(rect);
        // Visual backdrop and default signal colors follow the platform palette,
        // independently of the GUI skin. The header shares that backdrop.
        painter.rect_filled(rect, 7.0, self.automatic_theme.background);
        painter.rect_stroke(
            rect,
            7.0,
            Stroke::new(
                1.0,
                if selected {
                    mix(
                        mix(self.theme.panel, self.theme.muted, 0.22),
                        self.theme.accent,
                        0.18,
                    )
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
            self.inspecting_signal = false;
            self.inspecting_macro = None;
        }
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .id_salt(("pane", index))
                .max_rect(rect.shrink(8.0)),
        );
        child.set_clip_rect(rect.shrink(1.0).intersect(ui.clip_rect()));
        child.spacing_mut().interact_size.y = 24.0;
        child.spacing_mut().item_spacing.x = 6.0;
        child.spacing_mut().button_padding = Vec2::new(6.0, 3.0);
        child.horizontal(|ui| {
            if self.panel_fullscreen.is_none() && rect.width() >= 190.0 { workspace_ui::drag_grip(ui, index); }
            pane_title(ui, self.panes[index].kind);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if icons::sized_button(ui, Icon::Close, false, 24.0, "Close pane. Its neighbor expands to fill the space.").clicked() {
                    self.pending_close = Some(index);
                }
                if icons::sized_button(ui, Icon::Eye, self.sidebar_open && !self.inspecting_signal && self.inspecting_macro.is_none() && self.selected_pane == index,
                    24.0, "Inspect this panel. Open all its parameters in the right sidebar.").clicked() {
                    self.inspect_panel(index);
                }
                let expanded = self.panel_fullscreen == Some(index);
                if icons::sized_button(ui, if expanded { Icon::Restore } else { Icon::Fullscreen }, false,
                    24.0, if expanded { "Exit panel fullscreen · Esc" } else { "Fullscreen panel · F" }).clicked() {
                    self.toggle_panel_fullscreen(index, Instant::now());
                }
                let frozen = self.panes[index].is_frozen();
                if rect.width() >= 250.0 { ui.add_enabled_ui(live || frozen, |ui| {
                    if icons::sized_button(ui, if frozen { Icon::Play } else { Icon::Pause }, frozen, 24.0,
                        if frozen { "Resume this pane. Capture and other panes have kept running; paused time is omitted from this pane's history." }
                        else { "Freeze this pane for inspection. Audio capture and other panes keep running; settings remain editable." }).clicked() {
                        self.panes[index].toggle_freeze(frame, Instant::now());
                        self.selected_pane = index;
                    }
                }); }
            });
        });
        let canvas = Rect::from_min_max(
            Pos2::new(rect.left() + 8.0, child.cursor().top() + 3.0),
            rect.max - Vec2::splat(8.0),
        );
        if canvas.width() >= 24.0
            && canvas.height() >= 24.0
            && !self.recording.draw(
                &mut child,
                canvas,
                &mut self.panes[index],
                frame,
                &self.automatic_theme,
                live,
            )
        {
            self.panes[index].draw(&mut child, canvas, frame, &self.automatic_theme, live);
        }
        if let Some(started) = pane_started {
            self.performance.pane(
                format!("{} · {}", index + 1, self.panes[index].kind.label()),
                started.elapsed(),
            );
        }
    }

    fn open_panes(&self) -> Vec<usize> {
        self.tiles.as_ref().map_or_else(Vec::new, Tile::ids)
    }

    fn close_pane(&mut self, id: usize) {
        self.scene_macros
            .remove_component(self.panes[id].instance_id());
        self.panel_fullscreen = None;
        self.fullscreen_hint_started = None;
        if let Some(tree) = self.tiles.take() {
            self.tiles = tree.remove(id);
        }
        self.panes[id].reset();
        if !self.open_panes().contains(&self.selected_pane) {
            self.selected_pane = self.open_panes().first().copied().unwrap_or(0);
        }
    }

    fn available_slot(&mut self, kind: ModuleKind) -> Option<usize> {
        let ids = self.open_panes();
        let slot = (0..self.panes.len()).find(|id| !ids.contains(id));
        if let Some(id) = slot {
            self.panes[id] = ModulePane::new(kind);
            Some(id)
        } else if self.panes.len() < MAX_PANES {
            self.panes.push(ModulePane::new(kind));
            Some(self.panes.len() - 1)
        } else {
            None
        }
    }

    fn add_component(&mut self, kind: ModuleKind, target: usize, edge: Edge) {
        if edge == Edge::Center && self.tiles.is_some() {
            self.panes[target].kind = kind;
            self.panes[target].reset();
            self.selected_pane = target;
            return;
        }
        if self.open_panes().len() >= MAX_PANES {
            return;
        }
        if let Some(id) = self.available_slot(kind) {
            if let Some(tree) = &mut self.tiles {
                if !tree.insert(target, id, edge) {
                    return;
                }
            } else {
                self.tiles = Some(Tile::Pane(id));
            }
            self.selected_pane = id;
        }
    }

    fn dashboard(&mut self, ui: &mut egui::Ui, rect: Rect, frame: &AnalysisFrame) {
        if self.tiles.is_none() {
            // Passive guidance: the empty canvas must not behave like a giant
            // button or acquire a selection fill when the pointer crosses it.
            let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
            let message = painter.layout(
                "Click the + button at the top left,\nthen drag a visualizer into this window."
                    .into(),
                FontId::proportional(14.0),
                self.theme.muted,
                (rect.width() - 48.0).clamp(1.0, 400.0),
            );
            let position = rect.center() - message.size() * 0.5;
            painter.galley(position, message, self.theme.muted);
            let response = ui.interact(rect, ui.id().with("empty-drop"), Sense::hover());
            if response.dnd_hover_payload::<DragPayload>().is_some() {
                workspace_ui::drop_hint(ui, rect, "Add component", self.theme.accent);
            }
            if let Some(payload) = response.dnd_release_payload::<DragPayload>()
                && let DragPayload::Component(kind) = *payload
            {
                self.add_component(kind, 0, Edge::Right);
            }
            return;
        }
        let tree = self.tiles.as_ref().unwrap();
        let (leaves, dividers) = tree.geometry(rect);
        for divider in dividers {
            let response = ui
                .interact(
                    divider.handle,
                    ui.id().with(("tile-divider", &divider.path)),
                    Sense::click_and_drag(),
                )
                .on_hover_cursor(if divider.axis == Axis::Horizontal {
                    egui::CursorIcon::ResizeHorizontal
                } else {
                    egui::CursorIcon::ResizeVertical
                })
                .help_text("Drag to resize adjacent panes. Double-click to make this split equal.");
            let value = if response.double_clicked() {
                Some(0.5)
            } else if response.dragged() {
                response.interact_pointer_pos().map(|pos| {
                    if divider.axis == Axis::Horizontal {
                        (pos.x - divider.bounds.left()) / divider.bounds.width()
                    } else {
                        (pos.y - divider.bounds.top()) / divider.bounds.height()
                    }
                })
            } else {
                None
            };
            if let Some(value) = value {
                self.tiles
                    .as_mut()
                    .unwrap()
                    .set_fraction(&divider.path, value);
            }
            if response.hovered() || response.dragged() {
                ui.painter().rect_filled(
                    divider.handle.shrink2(Vec2::splat(2.0)),
                    1.0,
                    self.theme.accent,
                );
            }
        }
        let mut drop = None;
        for (id, cell) in leaves {
            self.pane(ui, id, cell, frame);
            let response = ui.interact(cell, ui.id().with(("pane-drop", id)), Sense::hover());
            if let Some(payload) = response.dnd_hover_payload::<DragPayload>()
                && let Some(pos) = ui.ctx().pointer_interact_pos()
            {
                let (edge, preview) = workspace::drop_zone(cell, pos);
                let can_drop = !matches!(*payload, DragPayload::Pane(source) if source == id)
                    && (edge == Edge::Center
                        || !matches!(*payload, DragPayload::Component(_))
                        || self.open_panes().len() < MAX_PANES);
                if can_drop {
                    let message = match (*payload, edge) {
                        (DragPayload::Pane(_), Edge::Center) => "Swap panes",
                        (DragPayload::Component(_), Edge::Center) => "Replace component",
                        _ => "Split here",
                    };
                    workspace_ui::drop_hint(ui, preview, message, self.theme.accent);
                }
            }
            if let Some(payload) = response.dnd_release_payload::<DragPayload>()
                && let Some(pos) = ui.ctx().pointer_interact_pos()
            {
                drop = Some((*payload, id, workspace::drop_zone(cell, pos).0));
            }
        }
        if let Some((payload, target, edge)) = drop {
            match payload {
                DragPayload::Component(kind) => self.add_component(kind, target, edge),
                DragPayload::Pane(source) => {
                    if let Some(tree) = &mut self.tiles
                        && tree.move_pane(source, target, edge)
                    {
                        self.selected_pane = source;
                    }
                }
            }
        }
        if let Some(id) = self.pending_close.take() {
            self.close_pane(id);
        }
    }

    fn preset_content(&self, scope: Scope) -> Option<PresetContent> {
        let tree = self.tiles.as_ref()?;
        Some(match scope {
            Scope::Layout => PresetContent::Layout(tree.canonical()),
            Scope::View => {
                let panes: Vec<_> = tree
                    .ids()
                    .iter()
                    .map(|id| self.panes[*id].snapshot())
                    .collect();
                PresetContent::View {
                    macros: Some(Box::new(self.scene_macros.for_panes(&panes))),
                    layout: tree.canonical(),
                    panes,
                }
            }
            Scope::Component => {
                PresetContent::Component(Box::new(self.panes[self.selected_pane].export_settings()))
            }
        })
    }

    fn apply_preset(&mut self, id: &str) {
        let Some(mut preset) = self
            .storage
            .presets
            .iter()
            .find(|preset| preset.id == id)
            .cloned()
        else {
            return;
        };
        crate::project::bind_preset_sources(
            &mut preset.content,
            &preset.signals,
            &self
                .signal_ui
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .bank,
        );
        match &preset.content {
            PresetContent::Component(settings) => {
                if self.panes[self.selected_pane].paste_settings(settings) {
                    self.panes[self.selected_pane].reset();
                } else {
                    return;
                }
            }
            PresetContent::View {
                layout,
                panes,
                macros,
            } => {
                self.scene_macros = macros
                    .clone()
                    .map(|m| *m)
                    .unwrap_or_else(|| crate::macros::scene::SceneMacros::from_legacy(panes, 0));
                self.panes = panes
                    .iter()
                    .map(|state| {
                        let mut pane = ModulePane::new(state.kind);
                        pane.restore(state);
                        pane
                    })
                    .collect();
                self.tiles = Some(layout.clone());
                self.selected_pane = 0;
            }
            PresetContent::Layout(layout) => {
                self.tiles = Some(workspace::apply_layout(
                    &mut self.panes,
                    self.tiles.as_ref(),
                    layout,
                ));
                if !self.open_panes().contains(&self.selected_pane) {
                    self.selected_pane = self.open_panes()[0];
                }
            }
        }
        self.storage.active = preset.id;
        self.storage.modified = false;
    }

    fn manager_actions(&mut self, actions: Vec<workspace_ui::Action>) {
        for action in actions {
            match action {
                workspace_ui::Action::Apply(id) => self.apply_preset(&id),
                workspace_ui::Action::Add(kind) => {
                    self.add_component(kind, self.selected_pane, Edge::Right);
                    self.inspecting_signal = false;
                    self.inspecting_macro = None;
                }
                workspace_ui::Action::Save(name, scope) => {
                    if let Some(content) = self.preset_content(scope) {
                        let id = format!(
                            "user:{}",
                            std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_nanos()
                        );
                        let signals = crate::project::dependencies(
                            &content,
                            &self
                                .signal_ui
                                .lock()
                                .unwrap_or_else(|e| e.into_inner())
                                .bank,
                        );
                        self.storage.presets.push(workspace::Preset {
                            signals,
                            id: id.clone(),
                            name,
                            content,
                        });
                        self.storage.active = id;
                        self.storage.modified = false;
                        self.storage.save_library();
                    }
                }
                workspace_ui::Action::Rename(id, name) => {
                    if let Some(preset) = self
                        .storage
                        .presets
                        .iter_mut()
                        .find(|p| p.id == id && !p.factory())
                    {
                        preset.name = name;
                        self.storage.save_library();
                    }
                }
                workspace_ui::Action::Delete(id) => {
                    self.storage.presets.retain(|p| p.id != id || p.factory());
                    if self.storage.active == id {
                        self.storage.active = "custom".into();
                        self.storage.modified = false;
                    }
                    self.storage.save_library();
                }
            }
        }
    }

    fn workspace_view(
        &mut self,
        ui: &mut egui::Ui,
        rect: Rect,
        snapshot: &AnalysisFrame,
        now: Instant,
    ) {
        for pane in &mut self.panes {
            pane.set_scene_macros(&self.scene_macros);
        }
        if self.scene_library.open {
            if let Some(command) = self.scene_library.show(ui, rect, &self.scenes) {
                self.scene_library_command(command);
            }
            return;
        }
        if let Some(index) = self.panel_fullscreen {
            let mut panel_rect = rect;
            if self.fullscreen_macros
                && let Some(height) = self.scene_macros.height()
            {
                let height = height.min((rect.height() - 150.0).max(30.0));
                let bar_rect =
                    Rect::from_min_max(Pos2::new(rect.left(), rect.bottom() - height), rect.max);
                panel_rect.max.y = bar_rect.top() - 6.0;
                self.scene_macro_bar(ui, bar_rect);
            }
            self.scene_library.canvas = Some(panel_rect);
            self.pane(ui, index, panel_rect, snapshot);
            if let Some(id) = self.pending_close.take() {
                self.close_pane(id);
            }
            if self.panel_fullscreen.is_some() {
                self.fullscreen_hint(ui, rect, now);
            }
        } else {
            self.normal_workspace(ui, rect, snapshot);
        }
    }

    fn fullscreen_hint(&mut self, ui: &egui::Ui, rect: Rect, now: Instant) {
        let Some(started) = self.fullscreen_hint_started else {
            return;
        };
        let alpha = (3.0 - now.saturating_duration_since(started).as_secs_f32()).clamp(0.0, 1.0);
        if alpha == 0.0 {
            self.fullscreen_hint_started = None;
            return;
        }
        let painter = ui.ctx().layer_painter(egui::LayerId::new(
            egui::Order::Tooltip,
            egui::Id::new("panel-fullscreen-hint"),
        ));
        let text = painter.layout_no_wrap(
            "Press Esc to exit fullscreen".into(),
            FontId::proportional(14.0),
            self.theme.foreground.gamma_multiply(alpha),
        );
        let bounds = Rect::from_center_size(
            Pos2::new(rect.center().x, rect.bottom() - 48.0),
            text.size() + Vec2::new(32.0, 20.0),
        );
        painter.rect_filled(bounds, 6.0, self.theme.card.gamma_multiply(alpha));
        painter.rect_stroke(
            bounds,
            6.0,
            Stroke::new(1.0, self.theme.muted.gamma_multiply(alpha * 0.5)),
            egui::StrokeKind::Inside,
        );
        painter.galley(
            bounds.min + Vec2::new(16.0, 10.0),
            text,
            self.theme.foreground.gamma_multiply(alpha),
        );
    }

    fn scene_macro_bar(&mut self, ui: &mut egui::Ui, rect: Rect) {
        ui.data_mut(|d| {
            let key = egui::Id::new(crate::macros::INSPECT_SELECTION);
            if let Some(index) = self.inspecting_macro.filter(|_| self.sidebar_open) {
                d.insert_temp(key, index);
            } else {
                d.remove::<usize>(key);
            }
        });
        self.scene_macros.bar(
            ui,
            rect,
            &self.theme,
            &format!("scene-{}", self.scenes.active_scene),
            &mut self.panes,
        );
        if let Some(index) =
            ui.data_mut(|d| d.remove_temp::<usize>(egui::Id::new(crate::macros::INSPECT_REQUEST)))
        {
            self.inspecting_macro = Some(index);
            self.inspecting_signal = false;
            self.sidebar_open = true;
            self.panel_fullscreen = None;
            self.fullscreen_hint_started = None;
        }
        for pane in &mut self.panes {
            pane.set_scene_macros(&self.scene_macros);
        }
    }
    fn normal_workspace(&mut self, ui: &mut egui::Ui, rect: Rect, snapshot: &AnalysisFrame) {
        let navigation = Rect::from_min_max(
            rect.min,
            Pos2::new(rect.right(), rect.top() + workspace_ui::NAV_HEIGHT),
        );
        workspace_ui::navigation_surface(ui, navigation, &self.theme);
        let header = navigation.shrink(workspace_ui::NAV_PADDING);
        let left = Rect::from_min_max(
            header.min,
            Pos2::new(header.left() + 240.0, header.bottom()),
        );
        let right_width = (header.width() - left.width() - 8.0).clamp(100.0, 260.0);
        let right = Rect::from_min_max(
            Pos2::new(header.right() - right_width, header.top()),
            header.max,
        );
        let half_width = (navigation.center().x - left.right() - 6.0)
            .min(right.left() - navigation.center().x - 6.0);
        let center = Rect::from_center_size(
            Pos2::new(navigation.center().x, header.center().y),
            Vec2::new((2.0 * half_width).min(620.0), header.height()),
        );
        #[cfg(not(target_os = "macos"))]
        let mut menu_action = None;
        let mut tabs = workspace_ui::navigation_region(ui, left, "main-navigation", &self.theme);
        tabs.horizontal_centered(|ui| {
            #[cfg(not(target_os = "macos"))]
            ui.menu_button("Menu", |ui| {
                let ctx = ui.ctx().clone();
                menu_action = crate::project_menu::show(ui, |action| self.menu_item_state(&ctx, action));
            });
            let toggle = icons::library_toggle(
                ui,
                self.manager.library_open,
                workspace_ui::NAV_CONTROL_HEIGHT,
                if self.manager.library_open {
                    "Hide component browser"
                } else {
                    "Show component browser\nClick an instrument or visualizer to add it, or drag it into the workspace."
                },
            );
            toggle.widget_info(|| egui::WidgetInfo::labeled(
                egui::WidgetType::Button, true, "Toggle component browser",
            ));
            if toggle.clicked() {
                self.manager.library_open = !self.manager.library_open;
            }
        });
        #[cfg(not(target_os = "macos"))]
        if let Some(action) = menu_action {
            self.menu_action(ui.ctx(), action);
        }
        let signals_open = self
            .signal_ui
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .show_signals;
        if signals_open && self.tiles.is_none() {
            self.inspecting_signal = true;
            self.inspecting_macro = None;
        }
        if self.scenes.scenes.is_empty() {
            self.scenes.store(self.capture_scene());
        }
        if center.width() > 180.0 {
            self.pending_scene = self
                .scene_browser
                .toolbar(
                    &mut workspace_ui::navigation_region(ui, center, "scene-toolbar", &self.theme),
                    &self.scenes,
                )
                .or(self.pending_scene.take());
        }
        let mut nav = workspace_ui::navigation_region(ui, right, "navbar", &self.theme);
        nav.horizontal_centered(|ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_enabled_ui(
                    signals_open || self.tiles.is_some() || self.inspecting_macro.is_some(),
                    |ui| {
                        let toggle = icons::sidebar_toggle(
                            ui,
                            self.sidebar_open
                                && (signals_open
                                    || self.tiles.is_some()
                                    || self.inspecting_macro.is_some()),
                            workspace_ui::NAV_CONTROL_HEIGHT,
                            if self.sidebar_open {
                                if self.inspecting_macro.is_some() {
                                    "Hide macro inspector · N"
                                } else if self.inspecting_signal {
                                    "Hide signal inspector · N"
                                } else {
                                    "Hide component inspector · N"
                                }
                            } else {
                                if self.inspecting_macro.is_some() {
                                    "Show macro inspector · N"
                                } else if self.inspecting_signal {
                                    "Show signal inspector · N"
                                } else {
                                    "Show component inspector · N"
                                }
                            },
                        );
                        toggle.widget_info(|| {
                            egui::WidgetInfo::labeled(
                                egui::WidgetType::Button,
                                signals_open
                                    || self.tiles.is_some()
                                    || self.inspecting_macro.is_some(),
                                "Toggle inspector",
                            )
                        });
                        if toggle.clicked() {
                            self.sidebar_open = !self.sidebar_open;
                        }
                    },
                );
            });
        });
        let inspector_open = self.sidebar_open
            && (self.tiles.is_some() || self.inspecting_signal || self.inspecting_macro.is_some());
        let browser_bounds =
            Rect::from_min_max(Pos2::new(rect.left(), navigation.bottom()), rect.max);
        if self.manager.library_open {
            let width = library_width(browser_bounds, inspector_open, self.library_width);
            let library = Rect::from_min_max(
                browser_bounds.min,
                Pos2::new(browser_bounds.left() + width, browser_bounds.bottom()),
            );
            let actions = self.manager.library(
                ui,
                library,
                &self.theme,
                self.open_panes().len() < MAX_PANES,
            );
            self.manager_actions(actions);
            // Register the boundary after the list/scrollbar so it owns edge gestures.
            resize_library(ui, browser_bounds, inspector_open, &mut self.library_width);
        }
        let browser_width = library_width(browser_bounds, inspector_open, self.library_width);
        let content_bounds = Rect::from_min_max(
            Pos2::new(
                rect.left()
                    + 12.0
                    + if self.manager.library_open {
                        browser_width
                    } else {
                        0.0
                    },
                rect.top() + workspace_ui::NAV_HEIGHT,
            ),
            rect.max,
        );
        let footer = Rect::from_min_max(
            Pos2::new(content_bounds.left(), rect.bottom() - 42.0),
            Pos2::new(
                if self.sidebar_open
                    && (self.tiles.is_some()
                        || self.inspecting_signal
                        || self.inspecting_macro.is_some())
                {
                    rect.right() - settings_width(content_bounds, self.sidebar_width) - 10.0
                } else {
                    rect.right() - 12.0
                },
                rect.bottom() - 2.0,
            ),
        );
        let mut footer_ui = ui.new_child(egui::UiBuilder::new().id_salt("footer").max_rect(footer));
        let signals_were_open = signals_open;
        let mut signals_open = signals_open;
        let mut macros_open = !signals_open && self.scene_macros.open;
        let macros_were_open = macros_open;
        self.global_bar.show(
            &mut footer_ui,
            &mut self.help_open,
            &mut signals_open,
            &mut macros_open,
            BarInfo {
                frame: snapshot,
                status: &self.status,
                theme: &self.theme,
                fps: self.display_fps,
            },
        );
        self.scene_macros.open = macros_open;
        if signals_open && !signals_were_open {
            self.inspecting_signal = true;
            self.inspecting_macro = None;
        }
        {
            let mut panel = self.signal_ui.lock().unwrap_or_else(|e| e.into_inner());
            panel.show_signals = signals_open;
            if std::mem::take(&mut panel.inspect_requested) {
                self.inspecting_signal = true;
                self.inspecting_macro = None;
                self.sidebar_open = true;
            }
        }
        if !signals_open || (macros_open && !macros_were_open) {
            self.inspecting_signal = false;
        }
        let content = content_bounds;
        resize_settings(
            ui,
            content,
            self.sidebar_open
                && (self.tiles.is_some()
                    || self.inspecting_signal
                    || self.inspecting_macro.is_some()),
            self.help_open,
            &mut self.sidebar_width,
        );
        let mut layout = workspace_layout(
            content,
            self.sidebar_open
                && (self.tiles.is_some()
                    || self.inspecting_signal
                    || self.inspecting_macro.is_some()),
            self.help_open,
            self.sidebar_width,
        );
        if signals_open {
            let dock_bottom = layout
                .help
                .map_or(layout.dashboard.bottom(), |r| r.bottom());
            let max_height = (dock_bottom - layout.dashboard.top() - 106.0).max(100.0);
            let min_height = crate::signals::ui::MIN_DOCK_HEIGHT.min(max_height);
            let height = self
                .signals_height
                .clamp(min_height, 1000.0)
                .min(max_height);
            let top = dock_bottom - height;
            let grip = Rect::from_min_max(
                Pos2::new(layout.dashboard.left(), top - 5.0),
                Pos2::new(layout.dashboard.right(), top + 3.0),
            );
            let response = ui
                .interact(grip, ui.id().with("signals-resize"), Sense::drag())
                .on_hover_cursor(egui::CursorIcon::ResizeVertical)
                .help_text("Drag to resize Signals · double-click to reset");
            if response.double_clicked() {
                self.signals_height = workspace::default_signals_height();
            } else if response.dragged()
                && let Some(pos) = response.interact_pointer_pos()
            {
                self.signals_height =
                    (dock_bottom - pos.y).clamp(min_height, max_height.clamp(min_height, 1000.0));
            }
            if response.hovered() || response.dragged() {
                ui.painter().hline(
                    grip.x_range(),
                    top - 1.0,
                    Stroke::new(1.0, self.theme.muted.gamma_multiply(0.8)),
                );
            }
            let dock = dock_performance(&mut layout, height);
            let mut panel = self.signal_ui.lock().unwrap_or_else(|e| e.into_inner());
            panel.show(ui, dock, snapshot, false, &self.theme);
            if std::mem::take(&mut panel.inspect_requested) {
                self.inspecting_signal = true;
                self.inspecting_macro = None;
                self.sidebar_open = true;
            }
        } else if let Some(height) = self.scene_macros.height() {
            let performance_rect = dock_performance(&mut layout, height);
            self.scene_macro_bar(ui, performance_rect);
        }
        if let Some(sidebar_rect) = layout.sidebar {
            ui.painter()
                .rect_filled(sidebar_rect, 0.0, self.theme.navigation);
            ui.painter().hline(
                sidebar_rect.x_range(),
                sidebar_rect.top() + 0.5,
                Stroke::new(1.0, self.theme.foreground.gamma_multiply(0.14)),
            );
            ui.painter().vline(
                sidebar_rect.left(),
                sidebar_rect.y_range(),
                Stroke::new(1.0, self.theme.foreground.gamma_multiply(0.12)),
            );
            let mut sidebar = ui.new_child(
                egui::UiBuilder::new()
                    .id_salt("sidebar")
                    .max_rect(sidebar_rect),
            );
            sidebar.set_clip_rect(sidebar_rect.intersect(ui.clip_rect()));
            if let Some(index) = self.inspecting_macro {
                let action = self.scene_macros.inspector(
                    &mut sidebar,
                    index,
                    &self.theme,
                    &mut self.panes,
                    &self.signal_ui,
                );
                match action {
                    crate::macros::scene::InspectAction::Close => self.sidebar_open = false,
                    crate::macros::scene::InspectAction::Signal(id) => {
                        let mut panel = self.signal_ui.lock().unwrap_or_else(|e| e.into_inner());
                        panel.selected = Some(id);
                        panel.show_signals = true;
                        self.scene_macros.open = false;
                        self.inspecting_macro = None;
                        self.inspecting_signal = true;
                    }
                    crate::macros::scene::InspectAction::None => {}
                }
                for pane in &mut self.panes {
                    pane.set_scene_macros(&self.scene_macros);
                }
            } else if self.inspecting_signal {
                let mut panel = self.signal_ui.lock().unwrap_or_else(|e| e.into_inner());
                panel.show_inspector(&mut sidebar, sidebar_rect, snapshot, &self.theme);
                if std::mem::take(&mut panel.close_inspector) {
                    self.sidebar_open = false;
                }
            } else {
                self.sidebar(&mut sidebar, snapshot);
            }
        }
        self.scene_library.canvas = Some(layout.dashboard);
        self.dashboard(ui, layout.dashboard, snapshot);
        if self.show_inspector {
            egui::Window::new("FFT inspector / diagnostics")
                .open(&mut self.show_inspector)
                .default_width(540.0)
                .show(ui.ctx(), |ui| {
                    diagnostics(ui, snapshot);
                    raw_inspector(ui, snapshot, &self.theme);
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
        if let Some(notice) = self.storage.notice.clone() {
            egui::Window::new("Workspace notice").show(ui.ctx(), |ui| {
                ui.label(notice);
                if ui.button("Dismiss").clicked() {
                    self.storage.notice = None;
                }
            });
        }
    }
}

impl eframe::App for SymphosApp {
    fn raw_input_hook(&mut self, ctx: &egui::Context, input: &mut egui::RawInput) {
        let pressed = input
            .events
            .iter()
            .any(|e| matches!(e, egui::Event::PointerButton { pressed: true, .. }));
        let activity = input.events.iter().any(|e| {
            matches!(
                e,
                egui::Event::PointerButton { .. }
                    | egui::Event::Key { pressed: true, .. }
                    | egui::Event::Text(_)
                    | egui::Event::Paste(_)
                    | egui::Event::MouseWheel { .. }
            )
        }) || (ctx.input(|i| i.pointer.any_down()) && !input.events.is_empty());
        if activity {
            self.begin_edit(pressed);
        }
        while let Some(action) = crate::project_menu::take() {
            if !self.menu_item_state(ctx, action).enabled {
                continue;
            }
            if matches!(
                action,
                crate::project_menu::Action::Undo | crate::project_menu::Action::Redo
            ) {
                if crate::shortcuts::editing_text(ctx) {
                    action.append_text_events(&mut input.events);
                } else {
                    self.menu_action(ctx, action);
                }
            } else if !action.append_text_events(&mut input.events) {
                self.begin_edit(true);
                self.menu_action(ctx, action);
            }
        }
    }
    fn on_exit(&mut self) {
        if let Some(ctx) = self.context.clone() {
            let state = self.workspace(&ctx);
            self.storage.checkpoint(&state);
        }
    }
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.context = Some(ui.ctx().clone());
        if !crate::shortcuts::editing_text(ui.ctx()) && !self.workspace_menu_blocked(ui.ctx()) {
            if ui.input_mut(|i| {
                i.consume_key(
                    egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                    egui::Key::Z,
                )
            }) {
                self.undo_edit(true);
            } else if ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Z)) {
                self.undo_edit(false);
            }
        }
        crate::project_menu::install();
        #[cfg(feature = "development")]
        if let Some(dev) = &mut self.development
            && let Some((theme, scale)) = dev.poll(ui.ctx())
        {
            self.automatic_theme = theme;
            self.automatic_scale = scale;
        }
        self.refresh_theme(ui.ctx());
        self.apply_appearance(ui.ctx());
        ui.ctx().data_mut(|d| {
            d.insert_temp(
                egui::Id::new("visual-target-fps"),
                self.render_fps.max(self.recording.target_fps()),
            );
            d.insert_temp(
                egui::Id::new("component-color-theme"),
                self.automatic_theme.clone(),
            )
        });
        help::begin_frame(ui);
        self.process_audio_events();
        let now = Instant::now();
        let elapsed = now
            .duration_since(self.last_frame)
            .as_secs_f32()
            .max(1.0e-4);
        self.last_frame = now;
        self.performance.begin(Duration::from_secs_f32(elapsed));
        self.display_fps += (elapsed.recip() - self.display_fps) * 0.08;
        let captured = self.engine.analysis().snapshot.load_full();
        let mut snapshot = (*captured).clone();
        {
            let mut panel = self.signal_ui.lock().unwrap_or_else(|e| e.into_inner());
            for pane in &mut self.panes {
                pane.reconcile_signals(&panel.bank);
            }
            self.scene_macros.reconcile(&self.panes, &panel.bank);
            panel.consumers.clear();
            for (index, slot) in self.scene_macros.slots.iter().enumerate() {
                if let (Some(id), Some(parameter), Some(target)) =
                    (slot.signal, &slot.parameter, &slot.target)
                {
                    panel.consumers.entry(id).or_default().push(format!(
                        "Macro {} · {} · {}",
                        index + 1,
                        target.kind.label(),
                        parameter.label
                    ));
                }
            }
            for (i, pane) in self.panes.iter().enumerate() {
                let mut settings = pane.copy_settings();
                if let Some(m) = settings.modulation_mut() {
                    for r in &m.routes {
                        if let Some(id) = r.signal {
                            panel.consumers.entry(id).or_default().push(format!(
                                "{} {} · {}",
                                settings_kind_name(pane.kind),
                                i + 1,
                                r.target.label()
                            ));
                        }
                    }
                }
            }
            if let Some(midi) = &self.midi_runtime {
                panel.midi_snapshot = midi.snapshot.load_full();
            }
            if let Some(runtime) = &self.signal_runtime {
                runtime.bind_audio(self.engine.analysis().snapshot.clone());
                runtime.publish(&panel.bank);
                runtime.connected.store(
                    matches!(self.status, AudioStatus::Streaming)
                        && self.engine.analysis().is_current(&captured),
                    std::sync::atomic::Ordering::Relaxed,
                );
                panel.snapshot = runtime.snapshot.load_full();
                snapshot.signals = Some(panel.snapshot.clone());
            }
            panel.advance_learning();
        }
        ui.data_mut(|d| {
            d.insert_temp(
                egui::Id::new(crate::signals::ui::CONTEXT_KEY),
                self.signal_ui.clone(),
            )
        });
        #[cfg(not(target_os = "macos"))]
        {
            // Compute availability outside input_mut to avoid re-locking egui input.
            let available: std::collections::BTreeSet<_> = crate::project_menu::commands()
                .filter(|c| self.menu_item_state(ui.ctx(), c.action).enabled)
                .map(|c| c.action.code())
                .collect();
            if let Some(action) =
                crate::project_menu::take_shortcut(ui.ctx(), |a| available.contains(&a.code()))
            {
                self.menu_action(ui.ctx(), action);
            }
        }
        self.handle_shortcut(ui.ctx(), &snapshot, now);
        let rect = ui.available_rect_before_wrap();
        ui.painter().rect_filled(rect, 0.0, self.theme.background);
        // Keep all assigned panes ingesting capture during panel fullscreen.
        // The analysis-side archive catches up after window occlusion.
        let after = self
            .open_panes()
            .iter()
            .map(|id| self.panes[*id].last_capture)
            .min()
            .unwrap_or(0);
        let captures = self
            .engine
            .analysis()
            .capture_history
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .since(after);
        for id in self.open_panes() {
            self.panes[id].ingest(&captures);
        }
        self.receive_scene_preview(ui.ctx());
        self.workspace_view(ui, rect, &snapshot, now);
        if let Some(index) = self.recording.target().and_then(|target| {
            self.open_panes()
                .into_iter()
                .find(|i| self.panes[*i].instance_id() == target)
        }) {
            self.recording.render_hidden(
                ui.ctx(),
                &mut self.panes[index],
                &snapshot,
                &self.automatic_theme,
                matches!(self.status, AudioStatus::Streaming),
            );
        }
        self.show_settings(ui.ctx(), &snapshot);
        self.signal_ui
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .show_processor(ui.ctx(), &snapshot);
        self.project_controls(ui.ctx());
        let actions =
            self.manager
                .dialogs(ui.ctx(), &self.storage, self.panes[self.selected_pane].kind);
        self.manager_actions(actions);
        if let Some(action) = self.scene_browser.dialogs(ui.ctx(), &self.scenes) {
            self.pending_scene = Some(action);
        }
        if let Some(command) = self.scene_library.dialogs(ui.ctx()) {
            self.scene_library_command(command);
        }
        if let Some(action) = self.pending_scene.take() {
            self.scene_action(action);
        }

        if let Some(snapshot) = self.midi_runtime.as_ref().map(|m| m.snapshot.load_full()) {
            let binding_edit = self.midi_controller.pending_change(ui.ctx(), &snapshot);
            if binding_edit {
                self.begin_edit(true);
            }
            if self.midi_controller.update(
                ui.ctx(),
                &snapshot,
                &mut self.scene_macros.midi,
                &mut self.panes,
            ) {
                self.finish_edit();
            }
        }
        if self.recording.target().is_some_and(|id| {
            !self
                .open_panes()
                .iter()
                .any(|i| self.panes[*i].instance_id() == id)
        }) {
            self.recording.stop();
        }
        let selected = self.open_panes().contains(&self.selected_pane).then(|| {
            (
                self.panes[self.selected_pane].instance_id(),
                self.panes[self.selected_pane].kind.label(),
            )
        });
        self.recording.show(
            ui.ctx(),
            selected,
            self.engine.analysis().snapshot.load().sample_rate,
        );
        self.performance
            .show(ui.ctx(), self.render_fps.max(self.recording.target_fps()));
        #[cfg(feature = "development")]
        self.design_system.show(ui.ctx(), &self.theme);
        self.settle_edit(ui.ctx());
        self.storage.poll_checkpoint();
        if ui.ctx().input(|input| input.viewport().close_requested()) {
            let state = self.workspace(ui.ctx());
            self.storage.checkpoint(&state);
        } else if self.storage.save_due() {
            let state = self.workspace(ui.ctx());
            self.storage.checkpoint_async(state);
        }
        crate::project_menu::update(|action| self.menu_item_state(ui.ctx(), action));
        // Display cadence is a local performance preference, independent of FFT cadence.
        self.performance.finish(now.elapsed());
        let delay = self.frame_pacer.delay(
            now,
            Instant::now(),
            self.render_fps.max(self.recording.target_fps()),
        );
        // egui subtracts predicted_dt internally. Compensate so a 60 FPS
        // target does not become an immediate repaint on a 120 Hz display.
        let predicted = Duration::from_secs_f32(ui.input(|i| i.predicted_dt).max(0.0));
        ui.ctx().request_repaint_after(delay + predicted);
    }
}

fn equalize_splits(tree: &mut Tile) {
    if let Tile::Split {
        fraction,
        first,
        second,
        ..
    } = tree
    {
        *fraction = 0.5;
        equalize_splits(first);
        equalize_splits(second);
    }
}

fn settings_kind_name(kind: ModuleKind) -> String {
    format!("{kind:?}")
}

#[cfg(all(test, feature = "development"))]
mod workspace_interaction_tests {
    use super::*;

    fn app() -> SymphosApp {
        SymphosApp {
            midi_runtime: None,
            midi_controller: Default::default(),
            signal_runtime: None,
            signal_ui: Default::default(),
            scene_macros: Default::default(),
            project_dialog: Default::default(),
            scenes: Default::default(),
            saved_project: None,
            history: Default::default(),
            performance: Default::default(),
            frame_pacer: Default::default(),
            recording: Default::default(),
            #[cfg(feature = "development")]
            design_system: Default::default(),
            scene_browser: Default::default(),
            scene_library: Default::default(),
            pending_scene: None,
            engine: AudioEngine::development_mode(true),
            development: None,
            sources: Vec::new(),
            selected_node: None,
            source_manually_selected: false,
            status: AudioStatus::Idle,
            theme: AppTheme::default(),
            automatic_theme: AppTheme::default(),
            automatic_scale: 1.0,
            appearance: AppearancePreferences::default(),
            pending_zoom: None,
            settings_window: settings::SettingsWindow::default(),
            last_theme_check: Instant::now(),
            fullscreen: false,
            panel_fullscreen: None,
            fullscreen_macros: false,
            fullscreen_hint_started: None,
            show_inspector: false,
            show_issues: false,
            show_shortcuts: false,
            settings_clipboard: None,
            display_fps: 0.0,
            render_fps: 60,
            last_frame: Instant::now(),
            panes: [
                ModuleKind::Waterfall,
                ModuleKind::Spectrum,
                ModuleKind::Waveform,
                ModuleKind::Spectrogram,
            ]
            .into_iter()
            .map(ModulePane::new)
            .collect(),
            tiles: Some(Tile::default_view()),
            storage: Storage::new(std::env::temp_dir().join("symphos-interaction-test-unused")),
            manager: Manager::default(),
            context: None,
            pending_close: None,
            selected_pane: 0,
            sidebar_open: false,
            sidebar_width: 236.0,
            library_width: workspace_ui::LIBRARY_WIDTH,
            signals_height: workspace::default_signals_height(),
            inspecting_signal: false,
            inspecting_macro: None,
            help_open: false,
            global_bar: GlobalBar::default(),
        }
    }
    #[test]
    fn undo_redo_restores_components_signals_and_keeps_audio_runtime() {
        let mut app = app();
        app.scenes = crate::scenes::Book::new(app.capture_scene());
        let audio = app.engine.analysis().snapshot.clone();
        let id = app.panes[0].instance_id().to_owned();
        app.begin_edit(true);
        app.signal_ui
            .lock()
            .unwrap()
            .bank
            .add(crate::signals::Method::Lfo)
            .unwrap();
        app.close_pane(0);
        app.finish_edit();
        assert!(!app.open_panes().contains(&0));
        app.undo_edit(false);
        assert!(app.open_panes().contains(&0));
        assert_eq!(app.panes[0].instance_id(), id);
        assert!(app.signal_ui.lock().unwrap().bank.channels.is_empty());
        assert!(std::sync::Arc::ptr_eq(
            &audio,
            &app.engine.analysis().snapshot
        ));
        app.undo_edit(true);
        assert!(!app.open_panes().contains(&0));
        assert_eq!(app.signal_ui.lock().unwrap().bank.channels.len(), 1);
    }
    #[test]
    fn undo_scene_creation_and_grouped_parameter_drag() {
        let mut app = app();
        app.scenes = crate::scenes::Book::new(app.capture_scene());
        let original = app.scenes.active_scene;
        app.begin_edit(true);
        app.scene_action(crate::scenes::Action::Duplicate);
        app.finish_edit();
        assert_eq!(app.scenes.scenes.len(), 2);
        app.undo_edit(false);
        assert_eq!(app.scenes.scenes.len(), 1);
        assert_eq!(app.scenes.active_scene, original);
        app.undo_edit(true);
        assert_eq!(app.scenes.scenes.len(), 2);
        app.begin_edit(true);
        for value in [0.2, 0.4, 0.6] {
            app.begin_edit(false);
            let mut settings = app.panes[0].copy_settings();
            crate::macros::set_many(&mut settings, &[("height".into(), value)]);
            app.panes[0].paste_settings(&settings);
        }
        app.finish_edit();
        app.undo_edit(false);
        assert_eq!(
            crate::macros::get(&app.panes[0].copy_settings(), "height"),
            Some(f64::from(0.85_f32))
        );
        app.undo_edit(true);
        assert_eq!(
            crate::macros::get(&app.panes[0].copy_settings(), "height"),
            Some(f64::from(0.6_f32))
        );
    }

    #[test]
    fn library_refresh_and_inspector_navigation_do_not_consume_parameter_undo() {
        let mut app = app();
        app.scenes = crate::scenes::Book::new(app.capture_scene());
        app.begin_edit(true);
        let mut settings = app.panes[0].copy_settings();
        crate::macros::set_many(&mut settings, &[("height".into(), 1.2)]);
        app.panes[0].paste_settings(&settings);
        app.finish_edit();

        app.begin_edit(true);
        app.selected_pane = 1;
        app.scene_macros.open = false;
        app.scene_action(crate::scenes::Action::Library);
        app.finish_edit();

        // One Undo reaches the actual parameter edit, not the Library's book cache.
        app.undo_edit(false);
        assert_eq!(
            crate::macros::get(&app.panes[0].copy_settings(), "height"),
            Some(f64::from(0.85_f32))
        );
        assert_eq!(app.selected_pane, 1);
        assert!(!app.scene_macros.open);
        assert!(!app.history.can_undo());
        app.undo_edit(true);
        assert_eq!(
            crate::macros::get(&app.panes[0].copy_settings(), "height"),
            Some(f64::from(1.2_f32))
        );
    }

    #[test]
    fn automatic_camera_motion_does_not_make_edits_or_jump_on_parameter_undo() {
        let mut app = app();
        let mut settings = serde_json::to_value(app.panes[0].copy_settings()).unwrap();
        settings["Waterfall"]["auto_orbit"] = true.into();
        app.panes[0].paste_settings(&serde_json::from_value(settings).unwrap());
        app.scenes = crate::scenes::Book::new(app.capture_scene());
        app.begin_edit(true);
        let mut settings = app.panes[0].copy_settings();
        *settings.automatic_yaw().unwrap() = 1.5;
        app.panes[0].paste_settings(&settings);
        app.finish_edit();
        assert!(!app.history.can_undo());
        app.begin_edit(true);
        crate::macros::set_many(&mut settings, &[("height".into(), 1.4)]);
        app.panes[0].paste_settings(&settings);
        app.finish_edit();
        *settings.automatic_yaw().unwrap() = 2.0;
        app.panes[0].paste_settings(&settings);
        app.undo_edit(false);
        let mut restored = app.panes[0].copy_settings();
        assert_eq!(*restored.automatic_yaw().unwrap(), 2.0);
        assert_eq!(
            crate::macros::get(&restored, "height"),
            Some(f64::from(0.85_f32))
        );
    }

    #[test]
    fn scenes_recall_both_tabs_keep_edits_and_do_not_restart_audio() {
        let mut app = app();
        let source = app.engine.analysis().snapshot.clone();
        app.scene_action(crate::scenes::Action::Rename(1, "Intro".into()));
        app.signal_ui
            .lock()
            .unwrap()
            .bank
            .add(crate::signals::Method::Lfo)
            .unwrap();
        app.signal_ui.lock().unwrap().show_signals = true;
        app.scene_action(crate::scenes::Action::Duplicate);
        let duplicate = app.scenes.active_scene;
        app.signal_ui.lock().unwrap().bank.channels[0].name = "Drop pulse".into();
        app.panes[0].kind = ModuleKind::Rings;
        app.scene_action(crate::scenes::Action::Switch(1));
        assert_eq!(app.panes[0].kind, ModuleKind::Waterfall);
        assert_ne!(
            app.signal_ui.lock().unwrap().bank.channels[0].name,
            "Drop pulse"
        );
        assert!(app.signal_ui.lock().unwrap().show_signals);
        app.scene_action(crate::scenes::Action::Switch(duplicate));
        assert_eq!(app.panes[0].kind, ModuleKind::Rings);
        assert_eq!(
            app.signal_ui.lock().unwrap().bank.channels[0].name,
            "Drop pulse"
        );
        assert!(std::sync::Arc::ptr_eq(
            &source,
            &app.engine.analysis().snapshot
        ));
        app.scene_action(crate::scenes::Action::New);
        assert!(app.signal_ui.lock().unwrap().bank.channels.is_empty());
        assert!(app.tiles.is_none());
        assert!(
            app.panes[0]
                .macros
                .banks
                .iter()
                .flatten()
                .all(|m| m.parameter.is_none() && m.signal.is_none())
        );
        let state = app.workspace(&egui::Context::default());
        let loaded = Session::decode(&serde_json::to_vec(&state).unwrap()).unwrap();
        assert_eq!(loaded.scenes.as_ref().unwrap().scenes.len(), 3);
        let project = crate::project::Project::capture(loaded);
        assert!(project.validate());
        assert_eq!(
            project.scenes.scenes[1].state.panes[0].kind,
            ModuleKind::Rings
        );
    }

    #[test]
    fn saving_project_includes_inactive_edits_and_reopening_preserves_preferences() {
        let mut app = app();
        let ctx = egui::Context::default();
        app.scene_action(crate::scenes::Action::Duplicate);
        app.panes[0].kind = ModuleKind::Cylinder;
        app.scene_action(crate::scenes::Action::Switch(1));
        let path =
            std::env::temp_dir().join(format!("symphos-scene-app-{}.json", std::process::id()));
        app.save_project(&ctx, &path);
        assert!(!app.project_changed(&ctx));
        let saved = crate::project::Project::open(&path).unwrap();
        assert_eq!(
            saved.scenes.scenes[1].state.panes[0].kind,
            ModuleKind::Cylinder
        );
        app.signal_ui
            .lock()
            .unwrap()
            .bank
            .add(crate::signals::Method::Band)
            .unwrap();
        assert!(app.project_changed(&ctx));
        let appearance = serde_json::to_value(&app.appearance).unwrap();
        app.selected_node = Some("test-device".into());
        app.open_project(&path);
        assert_eq!(serde_json::to_value(&app.appearance).unwrap(), appearance);
        assert_eq!(app.selected_node.as_deref(), Some("test-device"));
        assert_eq!(app.scenes.scenes.len(), 2);
        assert!(!app.project_changed(&ctx));
        app.new_project(&ctx);
        assert_eq!(app.scenes.scenes.len(), 1);
        assert!(app.project_dialog.current_path.is_none());
        assert!(app.signal_ui.lock().unwrap().bank.channels.is_empty());
        assert!(!app.project_changed(&ctx));
        let _ = std::fs::remove_file(path);
    }

    fn dashboard_rect() -> Rect {
        Rect::from_min_max(Pos2::new(300.0, 20.0), Pos2::new(1180.0, 760.0))
    }
    fn render(
        app: &mut SymphosApp,
        ctx: &egui::Context,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0))),
                events,
                ..Default::default()
            },
            |ui| {
                let actions = app.manager.library(
                    ui,
                    Rect::from_min_max(Pos2::new(0.0, 20.0), Pos2::new(242.0, 760.0)),
                    &app.theme,
                    true,
                );
                app.manager_actions(actions);
                app.dashboard(ui, dashboard_rect(), &AnalysisFrame::default());
            },
        );
        output.textures_delta.clear();
        output
    }
    fn button(pos: Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        }
    }
    #[test]
    fn library_drag_adds_a_pane_and_center_drop_replaces_without_adding() {
        let mut app = app();
        let ctx = egui::Context::default();
        render(&mut app, &ctx, vec![]);
        let output = render(&mut app, &ctx, vec![]);
        let source = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.job.text == ModuleKind::Spectrum.label() => {
                    Some(text.pos + text.galley.size() * 0.5)
                }
                _ => None,
            })
            .unwrap();
        let target = Pos2::new(305.0, 180.0);
        render(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(source), button(source, true)],
        );
        render(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(source + Vec2::new(15.0, 0.0))],
        );
        assert!(egui::DragAndDrop::has_payload_of_type::<DragPayload>(&ctx));
        render(&mut app, &ctx, vec![egui::Event::PointerMoved(target)]);
        render(&mut app, &ctx, vec![button(target, false)]);
        assert_eq!(app.open_panes().len(), 5);
        assert_eq!(app.panes[app.selected_pane].kind, ModuleKind::Spectrum);
        egui::DragAndDrop::set_payload(&ctx, DragPayload::Component(ModuleKind::Oscilloscope));
        let cell = app
            .tiles
            .as_ref()
            .unwrap()
            .geometry(dashboard_rect())
            .0
            .into_iter()
            .find(|(id, _)| *id == 0)
            .unwrap()
            .1;
        render(
            &mut app,
            &ctx,
            vec![
                egui::Event::PointerMoved(cell.center()),
                button(cell.center(), true),
            ],
        );
        render(&mut app, &ctx, vec![button(cell.center(), false)]);
        assert_eq!(app.open_panes().len(), 5);
        assert_eq!(app.panes[0].kind, ModuleKind::Oscilloscope);
    }
    fn workspace_frame(
        app: &mut SymphosApp,
        ctx: &egui::Context,
        now: Instant,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0))),
                events,
                ..Default::default()
            },
            |ui| {
                app.handle_shortcut(ui.ctx(), &AnalysisFrame::default(), now);
                let rect = ui.available_rect_before_wrap();
                app.workspace_view(ui, rect, &AnalysisFrame::default(), now);
            },
        );
        output.textures_delta.clear();
        output
    }

    #[test]
    fn component_browser_resize_keeps_panes_and_inspector_and_resets_on_double_click() {
        let mut app = app();
        let ctx = egui::Context::default();
        let now = Instant::now();
        app.manager.library_open = true;
        app.sidebar_open = true;
        let panes = app.open_panes();
        let frame = |app: &mut SymphosApp, events| workspace_frame(app, &ctx, now, events);
        frame(&mut app, vec![]);
        frame(&mut app, vec![]);
        // Real browser edge, alongside the rows: resizing must not drag/add a component.
        let start = Pos2::new(workspace_ui::LIBRARY_WIDTH + 2.0, 180.0);
        frame(
            &mut app,
            vec![egui::Event::PointerMoved(start), button(start, true)],
        );
        let end = Pos2::new(380.0, 180.0);
        frame(&mut app, vec![egui::Event::PointerMoved(end)]);
        frame(&mut app, vec![button(end, false)]);
        assert_eq!(app.library_width, 380.0);
        assert_eq!(app.open_panes(), panes);
        assert!(app.sidebar_open);
        assert!(!egui::DragAndDrop::has_any_payload(&ctx));
        let saved = app.workspace(&ctx);
        assert_eq!(
            Session::decode(&serde_json::to_vec(&saved).unwrap())
                .unwrap()
                .library_width,
            380.0
        );
        let toggle = Pos2::new(29.0, 29.0);
        for expected in [false, true] {
            frame(
                &mut app,
                vec![egui::Event::PointerMoved(toggle), button(toggle, true)],
            );
            frame(&mut app, vec![button(toggle, false)]);
            assert_eq!(app.manager.library_open, expected);
            assert_eq!(app.library_width, 380.0);
        }
        // Separate the resize gesture from the rapid hide/show clicks so egui
        // does not classify this sequence as a triple click.
        for _ in 0..40 {
            frame(&mut app, vec![]);
        }
        for _ in 0..2 {
            frame(
                &mut app,
                vec![egui::Event::PointerMoved(end), button(end, true)],
            );
            frame(&mut app, vec![button(end, false)]);
        }
        assert_eq!(app.library_width, workspace_ui::LIBRARY_WIDTH);
        assert_eq!(app.open_panes(), panes);
    }

    #[test]
    fn clicking_scene_macros_opens_details_and_source_inspector() {
        let mut app = app();
        let ctx = egui::Context::default();
        let now = Instant::now();
        let out = workspace_frame(&mut app, &ctx, now, vec![]);
        let label = |out: &egui::FullOutput, name: &str| {
            out.shapes
                .iter()
                .find_map(|s| match &s.shape {
                    egui::Shape::Text(t) if t.galley.job.text == name => {
                        Some(t.pos + t.galley.size() * 0.5)
                    }
                    _ => None,
                })
                .unwrap_or_else(|| panic!("missing {name}"))
        };
        let click = |app: &mut SymphosApp, pos| {
            workspace_frame(
                app,
                &ctx,
                now,
                vec![egui::Event::PointerMoved(pos), button(pos, true)],
            );
            workspace_frame(app, &ctx, now, vec![button(pos, false)]);
        };
        click(&mut app, label(&out, "Macro 1"));
        assert_eq!(app.inspecting_macro, Some(0));
        assert!(app.sidebar_open && !app.inspecting_signal);
        let out = workspace_frame(&mut app, &ctx, now, vec![]);
        label(&out, "Macro inspector");
        click(&mut app, label(&out, "Macro 2"));
        assert_eq!(app.inspecting_macro, Some(1));
        let source = app
            .signal_ui
            .lock()
            .unwrap()
            .bank
            .add(crate::signals::Method::Band)
            .unwrap();
        app.scene_macros.slots[1].signal = Some(source);
        app.scene_macros.slots[1].target = Some(crate::macros::ParameterContext {
            pane: app.panes[0].instance_id().into(),
            kind: ModuleKind::Waterfall,
        });
        app.scene_macros.slots[1].parameter = Some(crate::macros::ParameterSpec {
            field: "height".into(),
            label: "Height".into(),
            min: 0.0,
            max: 10.0,
            safe_min: 0.0,
            safe_max: 10.0,
            default: 1.0,
            integer: false,
            logarithmic: false,
            scale: 1.0,
        });
        let mut settings = app.panes[0].copy_settings();
        crate::macros::set(&mut settings, "height", 3.0);
        app.panes[0].apply_macro_settings(&settings);
        let out = workspace_frame(&mut app, &ctx, now, vec![]);
        label(&out, "Live value");
        click(&mut app, label(&out, "Reset value"));
        assert_eq!(
            crate::macros::get(&app.panes[0].copy_settings(), "height"),
            Some(1.0)
        );
        let out = workspace_frame(&mut app, &ctx, now, vec![]);
        click(&mut app, label(&out, "Inspect source"));
        assert!(app.inspecting_signal);
        assert!(app.inspecting_macro.is_none());
        assert_eq!(app.signal_ui.lock().unwrap().selected, Some(source));
        app.scene_macros.open = true;
        app.signal_ui.lock().unwrap().show_signals = false;
        let out = workspace_frame(&mut app, &ctx, now, vec![]);
        click(&mut app, label(&out, "Macro 1"));
        app.inspect_panel(0);
        assert!(app.inspecting_macro.is_none());
    }

    #[test]
    fn signal_inspector_navbar_toggle_hides_drawer_and_expands_mixer_without_visualizer_panes() {
        let mut app = app();
        let ctx = egui::Context::default();
        let now = Instant::now();
        app.signal_ui.lock().unwrap().show_signals = true;
        app.tiles = None;
        app.sidebar_open = true;
        let visible = |out: &egui::FullOutput| {
            out.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Text(t) if t.galley.job.text=="Signal inspector"))
        };
        workspace_frame(&mut app, &ctx, now, vec![]);
        assert!(visible(&workspace_frame(&mut app, &ctx, now, vec![])));
        let toggle = Pos2::new(1179.0, 21.0);
        workspace_frame(
            &mut app,
            &ctx,
            now,
            vec![egui::Event::PointerMoved(toggle), button(toggle, true)],
        );
        let hidden = workspace_frame(&mut app, &ctx, now, vec![button(toggle, false)]);
        assert!(!app.sidebar_open);
        assert!(!visible(&hidden));
        workspace_frame(&mut app, &ctx, now, vec![button(toggle, true)]);
        let restored = workspace_frame(&mut app, &ctx, now, vec![button(toggle, false)]);
        assert!(app.sidebar_open);
        assert!(visible(&restored));
    }

    #[test]
    fn bottom_docks_switch_resize_and_route_inspection_without_creating_sources() {
        let mut app = app();
        let ctx = egui::Context::default();
        let now = Instant::now();
        let source = app
            .signal_ui
            .lock()
            .unwrap()
            .bank
            .add(crate::signals::Method::Intensity)
            .unwrap();
        let label = |out: &egui::FullOutput, name: &str| {
            out.shapes
                .iter()
                .find_map(|s| match &s.shape {
                    egui::Shape::Text(t) if t.galley.job.text == name => {
                        Some(t.pos + t.galley.size() * 0.5)
                    }
                    _ => None,
                })
                .unwrap_or_else(|| panic!("missing {name}"))
        };
        let click = |app: &mut SymphosApp, pos| {
            workspace_frame(
                app,
                &ctx,
                now,
                vec![egui::Event::PointerMoved(pos), button(pos, true)],
            );
            workspace_frame(app, &ctx, now, vec![button(pos, false)]);
        };
        workspace_frame(&mut app, &ctx, now, vec![]);
        let out = workspace_frame(&mut app, &ctx, now, vec![]);
        click(&mut app, label(&out, "Signals"));
        assert!(app.signal_ui.lock().unwrap().show_signals);
        assert!(!app.scene_macros.open);
        assert_eq!(app.open_panes().len(), 4);
        let out = workspace_frame(&mut app, &ctx, now, vec![]);
        let channel = label(&out, "1 Overall intensity");
        click(&mut app, channel);
        assert!(app.inspecting_signal && app.sidebar_open);
        assert_eq!(app.signal_ui.lock().unwrap().selected, Some(source));
        let out = workspace_frame(&mut app, &ctx, now, vec![]);
        let channel = label(&out, "1 Overall intensity");
        let grip = Pos2::new(500.0, channel.y - 24.0);
        let old_height = app.signals_height;
        workspace_frame(
            &mut app,
            &ctx,
            now,
            vec![egui::Event::PointerMoved(grip), button(grip, true)],
        );
        workspace_frame(
            &mut app,
            &ctx,
            now,
            vec![egui::Event::PointerMoved(grip - Vec2::new(0.0, 60.0))],
        );
        workspace_frame(
            &mut app,
            &ctx,
            now,
            vec![button(grip - Vec2::new(0.0, 60.0), false)],
        );
        assert!(app.signals_height > old_height + 40.0);
        let state = app.workspace(&ctx);
        assert_eq!(
            Session::decode(&serde_json::to_vec(&state).unwrap())
                .unwrap()
                .signals_height,
            app.signals_height
        );
        click(&mut app, Pos2::new(100.0, 110.0));
        assert!(!app.inspecting_signal);
        assert!(app.signal_ui.lock().unwrap().show_signals);
        let out = workspace_frame(&mut app, &ctx, now, vec![]);
        click(&mut app, label(&out, "Macros"));
        assert!(!app.signal_ui.lock().unwrap().show_signals);
        assert!(app.scene_macros.open);
        workspace_frame(&mut app, &ctx, now, vec![]);
        click(&mut app, Pos2::new(29.0, 29.0));
        assert!(app.manager.library_open);
        workspace_frame(&mut app, &ctx, now, vec![]);
        let out = workspace_frame(&mut app, &ctx, now, vec![]);
        click(&mut app, label(&out, "3D Rings"));
        assert_eq!(app.open_panes().len(), 5);
        assert_eq!(app.panes[app.selected_pane].kind, ModuleKind::Rings);
        assert_eq!(app.signal_ui.lock().unwrap().bank.channels.len(), 1);
    }

    #[test]
    fn scene_macros_bind_across_components_and_survive_selection_and_scene_recall() {
        let mut app = app();
        let ctx = egui::Context::default();
        let now = Instant::now();
        app.sidebar_open = true;
        app.selected_pane = 0;
        workspace_frame(&mut app, &ctx, now, vec![]);
        let out = workspace_frame(&mut app, &ctx, now, vec![]);
        let source = out
            .shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.job.text == "Height" => {
                    Some(t.pos + t.galley.size() * 0.5)
                }
                _ => None,
            })
            .expect("Waterfall height in inspector");
        workspace_frame(
            &mut app,
            &ctx,
            now,
            vec![egui::Event::PointerMoved(source), button(source, true)],
        );
        workspace_frame(
            &mut app,
            &ctx,
            now,
            vec![egui::Event::PointerMoved(source - Vec2::new(16.0, 0.0))],
        );
        assert!(egui::DragAndDrop::has_payload_of_type::<
            crate::macros::ParameterDrag,
        >(&ctx));
        let drop = Pos2::new(60.0, 650.0);
        workspace_frame(&mut app, &ctx, now, vec![egui::Event::PointerMoved(drop)]);
        workspace_frame(&mut app, &ctx, now, vec![button(drop, false)]);
        let first = app.scene_macros.slots[0].clone();
        assert_eq!(first.parameter.as_ref().unwrap().field, "height");
        assert_eq!(
            first.target.as_ref().unwrap().pane,
            app.panes[0].instance_id()
        );
        app.selected_pane = 1;
        let out = workspace_frame(&mut app, &ctx, now, vec![]);
        assert_eq!(app.scene_macros.slots[0], first);
        let knob = out
            .shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Circle(c) if (c.radius - 18.0).abs() < 0.01 && c.center.y > 500.0 => {
                    Some(c.center)
                }
                _ => None,
            })
            .expect("scene macro knob");
        let target_base = crate::macros::get(&app.panes[0].copy_settings(), "height").unwrap();
        let other_settings = app.panes[1].copy_settings();
        workspace_frame(
            &mut app,
            &ctx,
            now,
            vec![egui::Event::PointerMoved(knob), button(knob, true)],
        );
        workspace_frame(
            &mut app,
            &ctx,
            now,
            vec![egui::Event::PointerMoved(knob - Vec2::new(0.0, 20.0))],
        );
        workspace_frame(
            &mut app,
            &ctx,
            now,
            vec![button(knob - Vec2::new(0.0, 20.0), false)],
        );
        assert!(crate::macros::get(&app.panes[0].copy_settings(), "height").unwrap() > target_base);
        assert_eq!(app.panes[1].copy_settings(), other_settings);
        let spec = crate::macros::ParameterSpec {
            field: "height".into(),
            label: "Height".into(),
            min: 0.0,
            max: 10.0,
            safe_min: 0.0,
            safe_max: 10.0,
            default: 1.0,
            integer: false,
            logarithmic: false,
            scale: 1.0,
        };
        app.panes[1].kind = ModuleKind::Waterfall;
        app.scene_macros.slots[1].parameter = Some(spec);
        app.scene_macros.slots[1].target = Some(crate::macros::ParameterContext {
            pane: app.panes[1].instance_id().to_owned(),
            kind: ModuleKind::Waterfall,
        });
        app.scene_macros.slots[1].name = Some("Second panel lift".into());
        let bank = app.scene_macros.clone();
        app.scenes.store(app.capture_scene());
        app.scene_action(crate::scenes::Action::New);
        assert!(app.scene_macros.slots.iter().all(|s| s.parameter.is_none()));
        app.scene_action(crate::scenes::Action::Switch(1));
        assert_eq!(app.scene_macros, bank);
        let captured = app.workspace(&ctx);
        let project = crate::project::Project::capture(captured);
        let project =
            crate::project::Project::decode(&serde_json::to_vec(&project).unwrap()).unwrap();
        assert_eq!(
            project
                .scenes
                .current()
                .unwrap()
                .state
                .macros
                .as_ref()
                .unwrap(),
            &bank
        );
        let view = app.preset_content(Scope::View).unwrap();
        let PresetContent::View { macros, panes, .. } = &view else {
            panic!("view")
        };
        assert!(macros.as_ref().unwrap().valid(panes));
        let target = app.scene_macros.slots[0].target.clone().unwrap();
        app.close_pane(0);
        assert!(app.scene_macros.slots[0].parameter.is_none());
        assert!(app.scene_macros.slots[1].parameter.is_some());
        assert_ne!(
            app.scene_macros.slots[1].target.as_ref().unwrap().pane,
            target.pane
        );
    }

    #[test]
    fn appearance_preferences_override_automatic_without_resetting_panes() {
        let mut app = app();
        let ctx = egui::Context::default();
        app.appearance.theme = crate::theme::ThemeChoice::Graphite;
        app.appearance.ui_scale = Some(1.25);
        app.panes[0].last_capture = 42;
        app.apply_appearance(&ctx);
        let mut output = ctx.run_ui(Default::default(), |_| {});
        output.textures_delta.clear();
        app.automatic_theme = crate::theme::ThemeChoice::Ember.resolve(&AppTheme::default());
        app.automatic_scale = 1.1;
        app.apply_appearance(&ctx);
        let mut output = ctx.run_ui(Default::default(), |_| {});
        output.textures_delta.clear();
        assert_eq!(app.theme.name, "Graphite");
        assert_eq!(ctx.zoom_factor(), 1.25);
        assert_eq!(app.panes[0].last_capture, 42);
        app.appearance = AppearancePreferences::default();
        app.apply_appearance(&ctx);
        let mut output = ctx.run_ui(Default::default(), |_| {});
        output.textures_delta.clear();
        assert_eq!(app.theme.name, "Ember");
        assert_eq!(ctx.zoom_factor(), 1.1);
        assert_eq!(app.panes[0].last_capture, 42);
    }

    #[test]
    fn gui_skins_preserve_visual_colors_frozen_frames_and_component_settings() {
        let mut app = app();
        let ctx = egui::Context::default();
        apply_style(&ctx, &app.theme);
        app.panes[0].kind = ModuleKind::Waveform;
        let mut frame = AnalysisFrame {
            sample_rate: 48_000,
            ..Default::default()
        };
        frame.waveform_left = (0..2048).map(|i| (i as f32 * 0.025).sin() * 0.6).collect();
        frame.waveform_right = frame.waveform_left.clone();
        app.panes[0].toggle_freeze(&frame, Instant::now());
        let settings = app.panes[0].copy_settings();
        let render = |app: &mut SymphosApp| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0))),
                    ..Default::default()
                },
                |ui| {
                    app.pane(
                        ui,
                        0,
                        Rect::from_min_max(Pos2::new(10.0, 10.0), Pos2::new(1100.0, 700.0)),
                        &frame,
                    );
                },
            );
            output.textures_delta.clear();
            output
                .shapes
                .into_iter()
                .filter(|s| s.clip_rect.top() > 40.0)
                .map(|s| s.shape)
                .collect::<Vec<_>>()
        };
        render(&mut app);
        let before = render(&mut app);
        assert!(!before.is_empty());
        for choice in [
            crate::theme::ThemeChoice::Resolve,
            crate::theme::ThemeChoice::Live,
        ] {
            app.appearance.theme = choice;
            app.apply_appearance(&ctx);
            assert_ne!(app.theme.accent, app.automatic_theme.accent);
            assert_eq!(
                render(&mut app),
                before,
                "GUI skin must not recolor or reset the waveform"
            );
            assert!(app.panes[0].is_frozen());
            assert_eq!(app.panes[0].copy_settings(), settings);
            let restored =
                Session::decode(&serde_json::to_vec(&app.workspace(&ctx)).unwrap()).unwrap();
            assert_eq!(restored.appearance.theme, choice);
        }
    }

    #[test]
    fn settings_owns_escape_before_panel_fullscreen() {
        let mut app = app();
        let ctx = egui::Context::default();
        app.panel_fullscreen = Some(0);
        app.settings_window.open = true;
        let mut output = ctx.run_ui(Default::default(), |ui| {
            app.show_settings(ui.ctx(), &AnalysisFrame::default());
        });
        output.textures_delta.clear();
        let mut output = ctx.run_ui(
            egui::RawInput {
                events: vec![egui::Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
                ..Default::default()
            },
            |ui| {
                app.handle_shortcut(ui.ctx(), &AnalysisFrame::default(), Instant::now());
                app.show_settings(ui.ctx(), &AnalysisFrame::default());
            },
        );
        output.textures_delta.clear();
        assert!(!app.settings_window.open);
        assert_eq!(app.panel_fullscreen, Some(0));
    }

    #[test]
    fn fullscreen_button_fills_window_and_escape_preserves_inspector_and_layout() {
        let mut app = app();
        let ctx = egui::Context::default();
        let now = Instant::now();
        app.sidebar_open = true;
        app.manager.library_open = true;
        let layout = app.tiles.clone();
        render(&mut app, &ctx, vec![]);
        render(&mut app, &ctx, vec![]);
        let panel = app.tiles.as_ref().unwrap().geometry(dashboard_rect()).0[0].1;
        let full = Pos2::new(panel.right() - 80.0, panel.top() + 20.0);
        render(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(full), button(full, true)],
        );
        render(&mut app, &ctx, vec![button(full, false)]);
        assert_eq!(app.panel_fullscreen, Some(0));
        let output = workspace_frame(&mut app, &ctx, now, vec![]);
        assert!(output.shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Rect(shape)
            if shape.fill == app.theme.background && shape.rect == Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0)))));
        assert!(!output.shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.job.text == "Components")));
        app.show_shortcuts = true; // A hidden modal must never trap fullscreen.
        workspace_frame(
            &mut app,
            &ctx,
            now,
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert!(app.panel_fullscreen.is_none());
        assert_eq!(app.tiles, layout);
        assert!(app.sidebar_open && app.manager.library_open);
        assert!(!app.fullscreen);
    }

    #[test]
    fn fullscreen_hint_fades_and_restore_and_eye_buttons_both_work() {
        let mut app = app();
        let ctx = egui::Context::default();
        let now = Instant::now();
        app.toggle_panel_fullscreen(0, now);
        let hint = |output: &egui::FullOutput| {
            output.shapes.iter().find_map(|shape| match &shape.shape {
                egui::Shape::Text(text)
                    if text.galley.job.text == "Press Esc to exit fullscreen" =>
                {
                    Some(text.galley.job.sections[0].format.color)
                }
                _ => None,
            })
        };
        let initial = hint(&workspace_frame(&mut app, &ctx, now, vec![])).unwrap();
        let fading = hint(&workspace_frame(
            &mut app,
            &ctx,
            now + Duration::from_millis(2500),
            vec![],
        ))
        .unwrap();
        assert!(fading.a() < initial.a());
        assert!(
            hint(&workspace_frame(
                &mut app,
                &ctx,
                now + Duration::from_millis(3100),
                vec![]
            ))
            .is_none()
        );
        let restore = Pos2::new(1120.0, 20.0);
        workspace_frame(
            &mut app,
            &ctx,
            now,
            vec![egui::Event::PointerMoved(restore), button(restore, true)],
        );
        workspace_frame(&mut app, &ctx, now, vec![button(restore, false)]);
        assert!(app.panel_fullscreen.is_none());
        app.toggle_panel_fullscreen(0, now);
        workspace_frame(&mut app, &ctx, now, vec![]);
        let eye = Pos2::new(1150.0, 20.0);
        workspace_frame(
            &mut app,
            &ctx,
            now,
            vec![egui::Event::PointerMoved(eye), button(eye, true)],
        );
        workspace_frame(&mut app, &ctx, now, vec![button(eye, false)]);
        assert!(app.panel_fullscreen.is_none());
        assert!(app.sidebar_open);
    }

    #[test]
    fn eye_button_selects_its_panel_and_opens_inspector_without_changing_layout() {
        let mut app = app();
        let ctx = egui::Context::default();
        let layout = app.tiles.clone();
        render(&mut app, &ctx, vec![]);
        render(&mut app, &ctx, vec![]);
        let panel = app.tiles.as_ref().unwrap().geometry(dashboard_rect()).0[1].1;
        let eye = Pos2::new(panel.right() - 52.0, panel.top() + 20.0);
        render(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(eye), button(eye, true)],
        );
        render(&mut app, &ctx, vec![button(eye, false)]);
        assert!(app.sidebar_open);
        assert_eq!(app.selected_pane, 1);
        assert_eq!(app.tiles, layout);
        assert!(!app.fullscreen);
        // Inspecting the same panel again keeps the inspector open.
        render(&mut app, &ctx, vec![button(eye, true)]);
        render(&mut app, &ctx, vec![button(eye, false)]);
        assert!(app.sidebar_open);
    }

    #[test]
    fn pane_grip_moves_the_instance_and_close_reflows_to_a_valid_empty_state() {
        let mut app = app();
        let ctx = egui::Context::default();
        render(&mut app, &ctx, vec![]);
        render(&mut app, &ctx, vec![]);
        let selection = app.tiles.as_ref().unwrap().geometry(dashboard_rect()).0[1]
            .1
            .center();
        render(
            &mut app,
            &ctx,
            vec![
                egui::Event::PointerMoved(selection),
                button(selection, true),
            ],
        );
        render(&mut app, &ctx, vec![button(selection, false)]);
        assert_eq!(app.selected_pane, 1);
        assert!(
            !app.sidebar_open,
            "selecting another component must not open a hidden inspector"
        );
        let source = dashboard_rect().min + Vec2::new(17.0, 20.0);
        let cell = app.tiles.as_ref().unwrap().geometry(dashboard_rect()).0[1].1;
        let target = cell.right_center() - Vec2::new(3.0, 0.0);
        render(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(source), button(source, true)],
        );
        render(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(source + Vec2::new(15.0, 0.0))],
        );
        assert!(egui::DragAndDrop::has_payload_of_type::<DragPayload>(&ctx));
        render(&mut app, &ctx, vec![egui::Event::PointerMoved(target)]);
        render(&mut app, &ctx, vec![button(target, false)]);
        assert_eq!(app.open_panes().len(), 4);
        assert!(app.tiles.as_ref().unwrap().valid(app.panes.len()));
        assert_ne!(app.tiles, Some(Tile::default_view()));
        assert_eq!(app.panes[0].kind, ModuleKind::Waterfall);
        for id in app.open_panes() {
            app.close_pane(id);
        }
        assert!(app.tiles.is_none());
        assert!(app.workspace(&ctx).validate());
    }
    #[test]
    fn scene_library_saves_full_setups_and_loads_without_overwriting_project_scenes() {
        use crate::scenes::library::{Command, Selection};
        let mut app = app();
        let dir =
            std::env::temp_dir().join(format!("symphos-scene-library-app-{}", std::process::id()));
        app.storage.dir = dir.clone();
        app.signal_ui
            .lock()
            .unwrap()
            .bank
            .add(crate::signals::Method::Lfo)
            .unwrap();
        app.scene_macros.slots[0].name = Some("My macro".into());
        app.scenes.store(app.capture_scene());
        let original = app.scenes.active_scene;
        let expected = serde_json::to_value(app.capture_scene()).unwrap();
        app.scene_library_command(Command::Save("Performance".into()));
        assert_eq!(app.scene_library.presets.entries.len(), 1);
        assert_eq!(app.scenes.scenes.len(), 1);
        assert_eq!(app.scenes.active_scene, original);
        let preset = app.scene_library.presets.entries[0].id;
        app.scene_library_command(Command::New);
        assert_eq!(
            app.scenes.active_scene, original,
            "creating a library card does not load it"
        );
        let new = match app.scene_library.selection.unwrap() {
            Selection::Scene(id) => id,
            _ => panic!(),
        };
        app.scene_library.open = true;
        app.scene_library_command(Command::Load(Selection::Scene(new)));
        assert!(!app.scene_library.open, "loading closes Library");
        assert!(app.signal_ui.lock().unwrap().bank.channels.is_empty());
        assert!(app.tiles.is_none());
        app.scene_library_command(Command::Load(Selection::Preset(preset)));
        assert_eq!(app.scenes.scenes.len(), 3);
        assert_eq!(serde_json::to_value(app.capture_scene()).unwrap(), expected);
        assert_eq!(
            serde_json::to_value(
                &app.scenes
                    .scenes
                    .iter()
                    .find(|s| s.id == original)
                    .unwrap()
                    .state
            )
            .unwrap(),
            expected
        );
        let active = app.scenes.active_scene;
        app.scene_library_command(Command::Duplicate(Selection::Scene(original)));
        assert_eq!(
            app.scenes.active_scene, active,
            "duplicating doesn't switch the playing scene"
        );
        app.scene_library_command(Command::Rename(Selection::Preset(preset), "Renamed".into()));
        assert_eq!(
            crate::scenes::library::Presets::load(&dir).unwrap().entries[0].name,
            "Renamed"
        );
        app.scene_library_command(Command::Delete(Selection::Preset(preset)));
        assert!(
            crate::scenes::library::Presets::load(&dir)
                .unwrap()
                .entries
                .is_empty()
        );
        assert_eq!(
            app.scenes.scenes.len(),
            4,
            "deleting a preset never deletes project scenes"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn scene_library_write_failure_retains_existing_presets() {
        use crate::scenes::library::Command;
        let mut app = app();
        let path =
            std::env::temp_dir().join(format!("symphos-scene-library-file-{}", std::process::id()));
        std::fs::write(&path, b"not a directory").unwrap();
        app.storage.dir = path.clone();
        app.scene_library_command(Command::Save("Cannot save here".into()));
        assert!(app.scene_library.presets.entries.is_empty());
        assert!(
            app.scene_library
                .notice
                .as_ref()
                .unwrap()
                .contains("Could not save")
        );
        assert_eq!(std::fs::read(&path).unwrap(), b"not a directory");
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn menu_docks_have_checked_states_and_preserve_scene_and_component_selection() {
        use crate::project_menu::Action as A;
        let mut app = app();
        let ctx = egui::Context::default();
        let ids = app.open_panes();
        let selected = app.selected_pane;
        app.scene_macros.open = false;
        app.menu_action(&ctx, A::Signals);
        assert!(app.menu_item_state(&ctx, A::Signals).checked);
        assert!(!app.menu_item_state(&ctx, A::Macros).checked);
        app.menu_action(&ctx, A::Macros);
        assert!(app.scene_macros.open);
        assert!(!app.signal_ui.lock().unwrap().show_signals);
        assert!(app.menu_item_state(&ctx, A::Macros).checked);
        app.menu_action(&ctx, A::Components);
        app.menu_action(&ctx, A::Inspector);
        app.menu_action(&ctx, A::Info);
        assert!(app.menu_item_state(&ctx, A::Components).checked);
        assert!(app.menu_item_state(&ctx, A::Inspector).checked);
        assert!(app.menu_item_state(&ctx, A::Info).checked);
        assert_eq!(app.open_panes(), ids);
        assert_eq!(app.selected_pane, selected);
        app.menu_action(&ctx, A::PanelFullscreen);
        assert!(app.menu_item_state(&ctx, A::PanelFullscreen).checked);
        assert!(!app.menu_item_state(&ctx, A::Signals).enabled);
        app.menu_action(&ctx, A::Signals); // stale commands must be ignored
        assert!(!app.signal_ui.lock().unwrap().show_signals);
        app.menu_action(&ctx, A::PanelFullscreen);
        assert!(app.panel_fullscreen.is_none());
        assert_eq!(app.open_panes(), ids);
    }

    #[test]
    fn menu_creation_and_scene_commands_keep_signals_explicit_and_use_existing_dialogs() {
        use crate::project_menu::Action as A;
        let mut app = app();
        let ctx = egui::Context::default();
        app.scenes.store(app.capture_scene());
        assert!(!app.menu_item_state(&ctx, A::DeleteScene).enabled);
        let count = app.open_panes().len();
        app.menu_action(&ctx, A::Add(ModuleKind::Orbital));
        assert_eq!(app.open_panes().len(), count + 1);
        assert_eq!(app.panes[app.selected_pane].kind, ModuleKind::Orbital);
        assert!(app.signal_ui.lock().unwrap().bank.channels.is_empty());
        app.menu_action(&ctx, A::Signal(crate::signals::Method::Midi));
        assert!(app.inspecting_signal && app.sidebar_open);
        assert_eq!(
            app.signal_ui.lock().unwrap().bank.channels[0].input,
            crate::signals::Input::Midi
        );
        let source_id = app.signal_ui.lock().unwrap().selected.unwrap();
        app.menu_action(&ctx, A::DuplicateScene);
        assert_eq!(app.scenes.scenes.len(), 2);
        assert_eq!(app.signal_ui.lock().unwrap().selected, Some(source_id));
        let duplicate = app.scenes.active_scene;
        app.menu_action(&ctx, A::PreviousScene);
        assert_ne!(app.scenes.active_scene, duplicate);
        app.menu_action(&ctx, A::NextScene);
        assert_eq!(app.scenes.active_scene, duplicate);
        app.menu_action(&ctx, A::DeleteScene);
        assert!(app.scene_browser.dialog_open());
        assert_eq!(
            app.scenes.scenes.len(),
            2,
            "deletion still requires the existing dialog"
        );
        assert!(!app.menu_item_state(&ctx, A::DuplicateScene).enabled);
    }

    #[test]
    fn menu_availability_respects_settings_text_entry_and_component_clipboard_type() {
        use crate::project_menu::Action as A;
        let mut app = app();
        let ctx = egui::Context::default();
        assert!(!app.menu_item_state(&ctx, A::PasteSettings).enabled);
        app.menu_action(&ctx, A::CopySettings);
        assert!(app.menu_item_state(&ctx, A::PasteSettings).enabled);
        app.selected_pane = 1;
        assert!(!app.menu_item_state(&ctx, A::PasteSettings).enabled);
        app.menu_action(&ctx, A::MidiSettings);
        assert!(app.settings_window.open);
        assert_eq!(app.settings_window.category, settings::Category::Midi);
        assert!(!app.menu_item_state(&ctx, A::NewScene).enabled);
        let count = app.open_panes().len();
        app.menu_action(&ctx, A::Add(ModuleKind::Sphere));
        assert_eq!(app.open_panes().len(), count);
        app.menu_action(&ctx, A::AudioSettings);
        assert_eq!(app.settings_window.category, settings::Category::Audio);
        app.settings_window.open = false;
        let mut text = String::from("test");
        let mut output = ctx.run_ui(Default::default(), |ui| {
            ui.text_edit_singleline(&mut text).request_focus();
        });
        output.textures_delta.clear();
        assert!(app.menu_item_state(&ctx, A::Copy).enabled);
        assert!(!app.menu_item_state(&ctx, A::CloseComponent).enabled);
        app.menu_action(&ctx, A::CloseComponent);
        assert_eq!(app.open_panes().len(), count);
        ctx.memory_mut(|m| m.surrender_focus(m.focused().unwrap()));
        for id in app.open_panes() {
            app.close_pane(id);
        }
        assert!(!app.menu_item_state(&ctx, A::CopySettings).enabled);
        assert!(!app.menu_item_state(&ctx, A::PanelFullscreen).enabled);
        assert!(
            app.menu_item_state(&ctx, A::Add(ModuleKind::Sphere))
                .enabled
        );
    }
}

fn settings_header(ui: &mut egui::Ui, kind: ModuleKind, metadata: &str) -> egui::Response {
    workspace_ui::sidebar_header(
        ui,
        kind.icon(),
        kind.label(),
        kind.subtitle(),
        metadata,
        "Close the component inspector. Use a panel’s eye button to reopen it.",
    )
}

fn settings_scroll_area(pane_index: usize, pane: &ModulePane) -> egui::ScrollArea {
    // Each pane and module keeps its own scroll position across close and resize.
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

fn pane_title(ui: &mut egui::Ui, kind: ModuleKind) -> egui::Response {
    // Reserve space for the pane actions and truncate long component names.
    let width =
        (ui.available_width() - 96.0 - 4.0 * ui.spacing().item_spacing.x).clamp(30.0, 180.0);
    ui.allocate_ui_with_layout(
        Vec2::new(width, 24.0),
        Layout::left_to_right(Align::Center),
        |ui| ui.add(egui::Label::new(kind.label()).truncate().selectable(false)),
    )
    .inner
    .help_text(kind.description())
}

struct WorkspaceLayout {
    dashboard: Rect,
    sidebar: Option<Rect>,
    help: Option<Rect>,
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
        ui.set_width(180.0);
        egui::ComboBox::from_id_salt("audio-source")
            .width(180.0).height(300.0).truncate().selected_text(label)
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

fn library_width(bounds: Rect, inspector_open: bool, requested: f32) -> f32 {
    // The inspector can compact itself as the browser grows. Reserve its minimum
    // width plus useful canvas space before allowing the browser to expand.
    let inspector = if inspector_open {
        DEFAULT_SETTINGS_WIDTH + 10.0
    } else {
        12.0
    };
    let maximum =
        (bounds.width() - 12.0 - inspector - 540.0).clamp(workspace_ui::LIBRARY_WIDTH, 440.0);
    requested.clamp(workspace_ui::LIBRARY_WIDTH, maximum)
}

fn resize_library(ui: &mut egui::Ui, bounds: Rect, inspector_open: bool, width: &mut f32) {
    let edge = bounds.left() + library_width(bounds, inspector_open, *width);
    let handle = Rect::from_min_max(
        Pos2::new(edge - 3.0, bounds.top()),
        Pos2::new(edge + 5.0, bounds.bottom()),
    );
    let response = ui
        .interact(
            handle,
            ui.id().with("library-resize"),
            Sense::click_and_drag(),
        )
        .on_hover_cursor(egui::CursorIcon::ResizeHorizontal)
        .help_text(
            "Drag to resize the component browser. Double-click to restore the compact width.",
        );
    if response.double_clicked() {
        *width = workspace_ui::LIBRARY_WIDTH;
    } else if response.dragged()
        && let Some(pointer) = response.interact_pointer_pos()
    {
        *width = library_width(bounds, inspector_open, pointer.x - bounds.left());
    }
    if response.hovered() || response.dragged() {
        ui.painter().vline(
            edge + 2.0,
            bounds.y_range(),
            Stroke::new(2.0, ui.visuals().selection.bg_fill),
        );
    }
}

fn settings_width(content: Rect, requested: f32) -> f32 {
    // Preserve enough space for useful plots even at the minimum window size.
    let maximum = (content.width() - 10.0 - 540.0).clamp(DEFAULT_SETTINGS_WIDTH, 440.0);
    requested.clamp(DEFAULT_SETTINGS_WIDTH, maximum)
}

fn resize_settings(ui: &mut egui::Ui, content: Rect, open: bool, help_open: bool, width: &mut f32) {
    let layout = workspace_layout(content, open, help_open, *width);
    let Some(sidebar) = layout.sidebar else {
        return;
    };
    let handle = Rect::from_min_max(
        Pos2::new(sidebar.left() - 5.0, sidebar.top()),
        Pos2::new(sidebar.left() + 3.0, sidebar.bottom()),
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
        *width = settings_width(content, content.right() - pointer.x);
    }
    if response.hovered() || response.dragged() {
        ui.painter().vline(
            sidebar.left() - 2.0,
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
    let settings_width = settings_width(content, requested_width);
    let sidebar = sidebar_open.then(|| {
        Rect::from_min_max(
            Pos2::new(content.right() - settings_width, content.top()),
            content.max,
        )
    });
    let mut dashboard = Rect::from_min_max(
        content.min + Vec2::new(0.0, 12.0),
        Pos2::new(
            sidebar.map_or(content.right() - 12.0, |rect| rect.left() - 10.0),
            content.bottom() - 50.0,
        ),
    );
    let help = help_open.then(|| {
        Rect::from_min_max(
            Pos2::new(content.left(), dashboard.bottom() - 160.0),
            Pos2::new(
                content.left() + DEFAULT_SETTINGS_WIDTH.min(dashboard.width()),
                dashboard.bottom(),
            ),
        )
    });
    if let Some(help) = help {
        dashboard.max.y = help.top() - 10.0;
    }
    WorkspaceLayout {
        dashboard,
        sidebar,
        help,
    }
}

fn dock_performance(layout: &mut WorkspaceLayout, requested_height: f32) -> Rect {
    let bottom = layout
        .help
        .map_or(layout.dashboard.bottom(), |r| r.bottom());
    let height = requested_height.min((bottom - layout.dashboard.top() - 100.0).max(30.0));
    let mut rect = Rect::from_min_max(
        Pos2::new(layout.dashboard.left(), bottom - height),
        Pos2::new(layout.dashboard.right(), bottom),
    );
    layout.dashboard.max.y = rect.top() - 6.0;
    if let Some(help) = &mut layout.help {
        // Both docks inset their visible frame by eight points vertically.
        // Info View shares that baseline and grows with the selected dock.
        *help = Rect::from_min_max(
            Pos2::new(rect.left(), rect.top() + 8.0),
            Pos2::new(rect.left() + help.width(), rect.bottom() - 8.0),
        );
        rect.min.x = help.right() + 10.0;
    }
    rect
}

fn apply_style(context: &egui::Context, theme: &AppTheme) {
    let mut visuals = if theme.dark_mode {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    let border = mix(theme.card, theme.muted, 0.4);
    let selected_text = contrasting_color(theme.accent, theme.foreground, theme.background);
    // Widget foregrounds supply normal text; selected buttons may use contrasting text.
    visuals.override_text_color = None;
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
    visuals.text_cursor.stroke.color = theme.accent;
    visuals.widgets.noninteractive.bg_fill = theme.card;
    visuals.widgets.noninteractive.weak_bg_fill = theme.card;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, border);
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
    fn narrow_pane_title_reserves_space_for_action_buttons() {
        for width in [120.0, 160.0, 240.0, 600.0] {
            for kind in ModuleKind::ALL {
                let context = egui::Context::default();
                let mut output = context.run_ui(egui::RawInput::default(), |ui| {
                    ui.set_width(width);
                    ui.spacing_mut().item_spacing.x = 10.0;
                    ui.spacing_mut().button_padding = Vec2::new(6.0, 3.0);
                    ui.horizontal(|ui| {
                        let title = pane_title(ui, kind);
                        assert!((title.rect.left() - ui.max_rect().left()).abs() < 0.1);
                        let pause = icons::sized_button(ui, Icon::Pause, false, 24.0, "Pause");
                        let expand = icons::sized_button(ui, Icon::Eye, false, 24.0, "Inspect");
                        assert!(title.rect.right() < pause.rect.left());
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
                        });
                    });
                },
            );
            output.textures_delta.clear();
            assert!(
                (picker_rect.width() - 180.0).abs() < 0.01,
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
    fn sidebar_close_button_collapses_every_module() {
        for kind in ModuleKind::ALL {
            let context = egui::Context::default();
            let mut open = true;
            let mut close_rect = Rect::NOTHING;
            let mut render = |open: &mut bool, events| {
                let mut output = context.run_ui(
                    egui::RawInput {
                        events,
                        ..Default::default()
                    },
                    |ui| {
                        if *open {
                            let close = settings_header(ui, kind, "Panel 1 · Live · 48 kHz");
                            close_rect = close.rect;
                            if close.clicked() {
                                *open = false;
                            }
                        }
                    },
                );
                output.textures_delta.clear();
                close_rect
            };
            let close = render(&mut open, vec![]).center();
            for pressed in [true, false] {
                render(
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
        let start = Pos2::new(content.right() - width, 180.0);
        render(&mut width, pointer(start, true));
        let end = Pos2::new(content.right() - 380.0, 180.0);
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
                        assert_eq!(help.left(), content.left());
                        assert!(help.right() < sidebar.left());
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
    fn performance_dock_reserves_plot_space_and_keeps_help_and_inspector_separate() {
        for width in [1024.0, 1440.0, 1800.0] {
            for help in [false, true] {
                for height in [236.0, 320.0, 390.0, 600.0] {
                    let content = Rect::from_min_size(Pos2::ZERO, Vec2::new(width, 658.0));
                    let mut layout = workspace_layout(content, true, help, 236.0);
                    let bar = dock_performance(&mut layout, height);
                    assert!(layout.dashboard.height() >= 90.0);
                    assert!(!bar.intersects(layout.dashboard));
                    assert!(!bar.intersects(layout.sidebar.unwrap()));
                    if let Some(help) = layout.help {
                        assert!(!help.intersects(bar));
                        assert!(!help.intersects(layout.dashboard));
                        assert_eq!(help.top(), bar.top() + 8.0);
                        assert_eq!(help.bottom(), bar.bottom() - 8.0);
                        assert_eq!(help.left(), content.left());
                        assert_eq!(bar.left(), help.right() + 10.0);
                    }
                    assert!(content.contains_rect(bar));
                }
            }
        }
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
                        assert_eq!(
                            help.left_bottom(),
                            content.left_bottom() - Vec2::new(0.0, 50.0)
                        );
                        assert!(!help.intersects(layout.dashboard));
                        if let Some(sidebar) = layout.sidebar {
                            assert!(!help.intersects(sidebar));
                            assert!(sidebar.height() >= 400.0);
                        }
                    }
                    if let Some(sidebar) = layout.sidebar {
                        assert_eq!(sidebar.right(), content.right());
                        assert_eq!(sidebar.top(), content.top());
                        assert_eq!(sidebar.bottom(), content.bottom());
                        assert_eq!(layout.dashboard.right(), sidebar.left() - 10.0);
                    }
                    if !sidebar_open && !help_open {
                        assert_eq!(layout.dashboard.right(), content.right() - 12.0);
                        assert_eq!(layout.dashboard.bottom(), content.bottom() - 50.0);
                    }
                }
            }
        }
    }
}
