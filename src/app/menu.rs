//! Menu commands reuse the same scene, dock, inspector and component operations
//! as the workspace. Availability is also checked at dispatch, so stale native
//! menu state cannot act on a deleted selection or a newly opened modal.
use super::*;
use crate::project_menu::{Action, ItemState};

impl SymphosApp {
    fn workspace_menu_blocked(&self, ctx: &egui::Context) -> bool {
        self.settings_window.open
            || self.project_dialog.open
            || self.project_dialog.pending_new
            || self.project_dialog.pending_open.is_some()
            || self.show_shortcuts
            || self.show_issues
            || self.show_inspector
            || self.manager.modal_open()
            || self.scene_browser.dialog_open()
            || self.scene_library.busy()
            || ctx.memory(|m| m.top_modal_layer().is_some())
    }
    pub(super) fn menu_item_state(&self, ctx: &egui::Context, action: Action) -> ItemState {
        use Action::*;
        let editing = crate::shortcuts::editing_text(ctx);
        let blocked = self.workspace_menu_blocked(ctx);
        if matches!(action, Cut | Copy | Paste | SelectAll) {
            return ItemState::enabled(editing);
        }
        // Settings categories can be switched from the app menu while Settings
        // is already open, but must not replace an unrelated confirmation.
        if matches!(
            action,
            Settings
                | DisplaySettings
                | ThemeSettings
                | AudioSettings
                | MidiSettings
                | AnalysisSettings
                | About
        ) {
            return ItemState::enabled(!blocked || self.settings_window.open);
        }
        let ids = self.open_panes();
        let selected = ids.contains(&self.selected_pane);
        let pane = &self.panes[self.selected_pane];
        let panel_fullscreen = self.panel_fullscreen.is_some();
        let signal_panel = self.signal_ui.lock().unwrap_or_else(|e| e.into_inner());
        let signals_open = signal_panel.show_signals && !panel_fullscreen;
        let mut checked = false;
        let available = match action {
            CopySettings | ResetComponent | CloseComponent => selected,
            PasteSettings => {
                selected
                    && self
                        .settings_clipboard
                        .as_ref()
                        .is_some_and(|s| s.kind() == pane.kind)
            }
            Add(_) => ids.len() < MAX_PANES && !panel_fullscreen,
            Signal(_) => {
                signal_panel.bank.channels.len() < crate::signals::MAX_SIGNALS && !panel_fullscreen
            }
            NewScene | DuplicateScene => self.scenes.scenes.len() < crate::scenes::MAX_SCENES,
            RenameScene => self.scenes.current().is_some(),
            DeleteScene | PreviousScene | NextScene => self.scenes.scenes.len() > 1,
            Components => {
                checked = self.manager.library_open && !panel_fullscreen;
                !panel_fullscreen
            }
            Inspector => {
                checked = self.sidebar_open && !panel_fullscreen;
                !panel_fullscreen && (selected || signals_open || self.inspecting_macro.is_some())
            }
            Macros => {
                checked = if panel_fullscreen {
                    self.fullscreen_macros
                } else {
                    self.scene_macros.open && !signals_open
                };
                true
            }
            Signals => {
                checked = signals_open;
                !panel_fullscreen
            }
            Info => {
                checked = self.help_open && !panel_fullscreen;
                !panel_fullscreen
            }
            PanelFullscreen => {
                checked = panel_fullscreen;
                selected
            }
            Fullscreen => {
                checked = self.fullscreen;
                true
            }
            Balance => ids.len() > 1 && !panel_fullscreen,
            ZoomIn => ctx.zoom_factor() < 2.0,
            ZoomOut => ctx.zoom_factor() > 0.6,
            ActualSize => (ctx.zoom_factor() - 1.0).abs() > 0.001,
            Pause | PauseAll => {
                checked = if action == Pause {
                    selected && pane.is_frozen()
                } else {
                    !ids.is_empty() && ids.iter().all(|id| self.panes[*id].is_frozen())
                };
                let frame = self.engine.analysis().snapshot.load();
                let live = matches!(self.status, AudioStatus::Streaming)
                    && self.selected_node.is_some()
                    && frame.sequence > 0
                    && self.engine.analysis().is_current(&frame);
                selected
                    && (live
                        || if action == Pause {
                            pane.is_frozen()
                        } else {
                            ids.iter().any(|id| self.panes[*id].is_frozen())
                        })
            }
            FftDiagnostics => {
                checked = self.show_inspector;
                true
            }
            _ => true,
        };
        // Bare-letter shortcuts must keep working as text inside editable fields.
        // Explicit menu commands also avoid mutating a component mid-entry.
        ItemState {
            enabled: available && !blocked && !editing,
            checked,
        }
    }

    pub(super) fn toggle_macros(&mut self) {
        if self.panel_fullscreen.is_some() {
            self.fullscreen_macros = !self.fullscreen_macros;
            if self.fullscreen_macros {
                self.scene_macros.open = true;
            }
            self.fullscreen_hint_started = None;
        } else {
            let mut panel = self.signal_ui.lock().unwrap_or_else(|e| e.into_inner());
            if panel.show_signals {
                panel.show_signals = false;
                self.scene_macros.open = true;
            } else {
                self.scene_macros.open = !self.scene_macros.open;
            }
            self.inspecting_signal = false;
        }
    }
    pub(super) fn toggle_signals(&mut self) {
        let mut panel = self.signal_ui.lock().unwrap_or_else(|e| e.into_inner());
        panel.show_signals = !panel.show_signals;
        if panel.show_signals {
            self.scene_macros.open = false;
            self.inspecting_signal = true;
            self.inspecting_macro = None;
        } else {
            self.inspecting_signal = false;
        }
    }
    pub(super) fn menu_action(&mut self, ctx: &egui::Context, action: Action) {
        if !self.menu_item_state(ctx, action).enabled {
            return;
        }
        use Action::*;
        let category = match action {
            DisplaySettings => Some(settings::Category::Display),
            ThemeSettings => Some(settings::Category::Theme),
            AudioSettings => Some(settings::Category::Audio),
            MidiSettings => Some(settings::Category::Midi),
            AnalysisSettings => Some(settings::Category::Analysis),
            About => Some(settings::Category::Help),
            _ => None,
        };
        if let Some(category) = category {
            self.settings_window.category = category;
            self.settings_window.open = true;
            return;
        }
        match action {
            New | Open | Save | SaveAs | Settings => self.project_action(ctx, action),
            Cut => ctx.send_viewport_cmd(egui::ViewportCommand::RequestCut),
            Copy => ctx.send_viewport_cmd(egui::ViewportCommand::RequestCopy),
            Paste => ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaste),
            SelectAll => {
                ctx.input_mut(|i| action.append_text_events(&mut i.events));
            }
            CopySettings => {
                self.settings_clipboard = Some(self.panes[self.selected_pane].export_settings())
            }
            PasteSettings => self.paste_module_settings(),
            ResetComponent => self.panes[self.selected_pane].reset_settings(),
            CloseComponent => self.close_pane(self.selected_pane),
            Components => self.manager.library_open = !self.manager.library_open,
            Inspector => self.sidebar_open = !self.sidebar_open,
            Macros => self.toggle_macros(),
            Signals => self.toggle_signals(),
            Info => self.help_open = !self.help_open,
            PanelFullscreen => self.toggle_panel_fullscreen(self.selected_pane, Instant::now()),
            Fullscreen => self.toggle_fullscreen(ctx),
            Balance => {
                if let Some(tree) = &mut self.tiles {
                    equalize_splits(tree);
                }
            }
            ZoomIn | ZoomOut | ActualSize => {
                let zoom = match action {
                    ZoomIn => (ctx.zoom_factor() + 0.1).min(2.0),
                    ZoomOut => (ctx.zoom_factor() - 0.1).max(0.6),
                    _ => 1.0,
                };
                self.pending_zoom = None;
                self.appearance.ui_scale = Some(zoom);
                self.apply_appearance(ctx);
            }
            Pause | PauseAll => {
                let frame = self.engine.analysis().snapshot.load_full();
                let now = Instant::now();
                if action == Pause {
                    self.panes[self.selected_pane].toggle_freeze(&frame, now);
                } else {
                    let ids = self.open_panes();
                    let pause = ids.iter().any(|id| !self.panes[*id].is_frozen());
                    for id in ids {
                        if self.panes[id].is_frozen() != pause {
                            self.panes[id].toggle_freeze(&frame, now);
                        }
                    }
                }
            }
            ClearHistory => self.reset_capture(),
            ResetMeters => self.global_bar.reset_meter_hold(),
            NewScene => self.scene_action(crate::scenes::Action::New),
            DuplicateScene => self.scene_action(crate::scenes::Action::Duplicate),
            RenameScene => self.scene_browser.rename_current(&self.scenes),
            DeleteScene => self.scene_browser.delete_current(&self.scenes),
            PreviousScene | NextScene => self.scene_action(crate::scenes::Action::Switch(
                self.scenes
                    .cycle(if action == PreviousScene { -1 } else { 1 }),
            )),
            Presets => self.request_scene_preview(crate::scenes::library::CapturePurpose::Open),
            Add(kind) => self.add_component(kind, self.selected_pane, Edge::Right),
            Signal(method) => {
                let mut panel = self.signal_ui.lock().unwrap_or_else(|e| e.into_inner());
                if let Some(id) = panel.bank.add(method) {
                    panel.bank.sync_slots();
                    panel.selected = Some(id);
                    panel.show_signals = true;
                    panel.inspect_requested = true;
                    self.scene_macros.open = false;
                    self.inspecting_macro = None;
                    self.inspecting_signal = true;
                    self.sidebar_open = true;
                }
            }
            Shortcuts => self.show_shortcuts = true,
            Issues => self.show_issues = true,
            FftDiagnostics => self.show_inspector = true,
            _ => {}
        }
        ctx.request_repaint();
    }
}
