use super::*;
use crate::scenes::library::{self, Capture, CapturePurpose, Command, Selection};

impl SymphosApp {
    pub(super) fn request_scene_preview(&mut self, purpose: CapturePurpose) {
        self.scenes.store(self.capture_scene());
        self.scene_library.ensure_loaded(&self.storage.dir);
        self.scene_library.open = false;
        let Some(ctx) = self.context.as_ref() else {
            match purpose {
                CapturePurpose::Save => self.scene_library.save_dialog(&self.scenes),
                _ => self.scene_library.open(&self.scenes),
            }
            return;
        };
        let rect = self.scene_library.canvas.unwrap_or(ctx.content_rect());
        self.scene_library.capture = Some(Capture {
            purpose,
            scene: self.scenes.active_scene,
            rect,
            scale: ctx.pixels_per_point(),
            started: Instant::now(),
        });
        ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::new(
            "scene-preview",
        )));
    }

    pub(super) fn receive_scene_preview(&mut self, ctx: &egui::Context) {
        let Some(capture) = self.scene_library.capture.as_ref() else {
            return;
        };
        let image = ctx.input(|i| {
            i.events.iter().find_map(|event| {
                if let egui::Event::Screenshot {
                    image, user_data, ..
                } = event
                    && user_data
                        .data
                        .as_ref()
                        .and_then(|d| d.downcast_ref::<&str>())
                        == Some(&"scene-preview")
                {
                    Some(image.clone())
                } else {
                    None
                }
            })
        });
        if image.is_none() && capture.started.elapsed() < Duration::from_secs(3) {
            return;
        }
        let capture = self.scene_library.capture.take().unwrap();
        if let Some(preview) =
            image.and_then(|image| library::Preview::capture(&image, capture.rect, capture.scale))
        {
            if let Some(scene) = self
                .scenes
                .scenes
                .iter_mut()
                .find(|s| s.id == capture.scene)
            {
                scene.preview = Some(preview);
                self.scene_library.invalidate(Selection::Scene(scene.id));
            }
        } else {
            self.scene_library.notice =
                Some("Preview capture unavailable. You can still manage and save scenes.".into());
        }
        match capture.purpose {
            CapturePurpose::Save => self.scene_library.save_dialog(&self.scenes),
            _ => self.scene_library.open(&self.scenes),
        }
    }

    pub(super) fn scene_library_command(&mut self, command: Command) {
        self.scenes.store(self.capture_scene());
        self.scene_library.ensure_loaded(&self.storage.dir);
        let selection = self.scene_library.selection;
        let mut presets = self.scene_library.presets.clone();
        let mut persist = false;
        let mut next_selection = selection;
        match command {
            Command::New => {
                let active = self.scenes.active_scene;
                let state = crate::project::SceneState {
                    macros: Some(Default::default()),
                    signals: Default::default(),
                    panes: vec![ModulePane::new(ModuleKind::Waterfall).snapshot()],
                    layout: None,
                    selected_pane: 0,
                    analysis: self.capture_scene().analysis,
                };
                if let Some(id) = self
                    .scenes
                    .insert(format!("Scene {}", self.scenes.next_scene_id), state)
                {
                    self.scene_library.selection = Some(Selection::Scene(id));
                }
                self.scenes.active_scene = active;
            }
            Command::Load(Selection::Scene(id)) => {
                self.scene_action(crate::scenes::Action::Switch(id));
                self.scene_library.open = false;
            }
            Command::Load(Selection::Preset(id)) => {
                if let Some(preset) = presets.entries.iter().find(|s| s.id == id).cloned()
                    && let Some(scene_id) = self.scenes.insert(preset.name, preset.state.clone())
                {
                    self.scenes
                        .scenes
                        .iter_mut()
                        .find(|s| s.id == scene_id)
                        .unwrap()
                        .preview = preset.preview;
                    self.apply_scene(preset.state);
                    self.scene_library.open = false;
                }
            }
            Command::Duplicate(Selection::Scene(id)) => {
                if let Some(scene) = self.scenes.scenes.iter().find(|s| s.id == id).cloned() {
                    let active = self.scenes.active_scene;
                    self.scenes.active_scene = id;
                    if let Some(new) = self.scenes.insert(
                        format!("{} copy", scene.name.chars().take(25).collect::<String>()),
                        scene.state,
                    ) {
                        self.scenes
                            .scenes
                            .iter_mut()
                            .find(|s| s.id == new)
                            .unwrap()
                            .preview = scene.preview;
                        self.scene_library.selection = Some(Selection::Scene(new));
                    }
                    self.scenes.active_scene = active;
                }
            }
            Command::Duplicate(Selection::Preset(id)) => {
                if let Some(scene) = presets.entries.iter().find(|s| s.id == id).cloned() {
                    match presets.insert(
                        format!("{} copy", scene.name.chars().take(25).collect::<String>()),
                        scene.state,
                        scene.preview,
                    ) {
                        Ok(id) => {
                            next_selection = Some(Selection::Preset(id));
                            persist = true;
                        }
                        Err(e) => self.scene_library.notice = Some(e),
                    }
                }
            }
            Command::Rename(Selection::Scene(id), name) => {
                self.scenes.rename(id, &name);
                self.scene_library.finish_dialog();
            }
            Command::Rename(Selection::Preset(id), name) => {
                if let Some(p) = presets.entries.iter_mut().find(|p| p.id == id) {
                    p.name = name.trim().into();
                    persist = true;
                }
            }
            Command::Delete(Selection::Scene(id)) => {
                self.scene_action(crate::scenes::Action::Delete(id));
                self.scene_library.selection = Some(Selection::Scene(self.scenes.active_scene));
                self.scene_library.invalidate(Selection::Scene(id));
                self.scene_library.finish_dialog();
            }
            Command::Delete(Selection::Preset(id)) => {
                presets.entries.retain(|s| s.id != id);
                next_selection = presets.entries.first().map(|s| Selection::Preset(s.id));
                self.scene_library.invalidate(Selection::Preset(id));
                persist = true;
            }
            Command::Reorder(Selection::Scene(id), position) => {
                library::reorder(&mut self.scenes.scenes, id, position)
            }
            Command::Reorder(Selection::Preset(id), position) => {
                library::reorder(&mut presets.entries, id, position);
                persist = true;
            }
            Command::Save(name) => {
                let mut state = self.capture_scene();
                // Include portable MilkDrop assets when saving across projects.
                for (snapshot, pane) in state.panes.iter_mut().zip(&self.panes) {
                    if pane.kind == ModuleKind::Milkdrop {
                        for settings in &mut snapshot.settings {
                            if settings.kind() == ModuleKind::Milkdrop {
                                *settings = pane.export_settings();
                            }
                        }
                    }
                }
                let preview = self.scenes.current().and_then(|s| s.preview.clone());
                match presets.insert(name, state, preview) {
                    Ok(id) => {
                        next_selection = Some(Selection::Preset(id));
                        persist = true;
                    }
                    Err(e) => self.scene_library.notice = Some(e),
                }
            }
            Command::Refresh => self.request_scene_preview(CapturePurpose::Refresh),
            Command::LegacyPresets => self.manager.open_presets(Scope::View),
        }
        if persist {
            if self.scene_library.load_failed {
                return;
            }
            match presets.save(&self.storage.dir) {
                Ok(()) => {
                    self.scene_library.presets = presets;
                    self.scene_library.selection = next_selection;
                    self.scene_library.notice = Some("Scene preset library saved.".into());
                    self.scene_library.finish_dialog();
                }
                Err(error) => {
                    self.scene_library.notice =
                        Some(format!("Could not save scene presets: {error}"))
                }
            }
        }
    }
}
