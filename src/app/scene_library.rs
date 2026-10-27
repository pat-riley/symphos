use super::*;
use crate::scenes::library::{self, Capture, CapturePurpose, Command, Selection};

impl SymphosApp {
    pub(super) fn activate_scene_preset(&mut self, id: u64, name: String, note: String) {
        let baseline = self.capture_scene().creative_fingerprint();
        if let Some(scene) = self.scenes.current_mut() {
            scene.name = name;
            scene.note = note;
            scene.preset = baseline.map(|baseline| crate::scenes::PresetLink {
                id,
                baseline,
                edited: false,
            });
        }
    }
    pub(super) fn refresh_scene_preset_edited(&mut self) {
        if self.scenes.current().is_none_or(|s| s.preset.is_none()) {
            return;
        }
        let fingerprint = self.capture_scene().creative_fingerprint();
        if let Some(link) = self.scenes.current_mut().and_then(|s| s.preset.as_mut()) {
            link.edited = fingerprint != Some(link.baseline);
        }
    }
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
        let mut saved_preset = None;
        let mut renamed_preset = None;
        match command {
            Command::New => {
                let active = self.scenes.active_scene;
                let state = crate::project::SceneState {
                    background: Default::default(),
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
                    && let Some(scene_id) = self
                        .scenes
                        .insert(preset.name.clone(), preset.state.clone())
                {
                    self.scenes
                        .scenes
                        .iter_mut()
                        .find(|s| s.id == scene_id)
                        .unwrap()
                        .preview = preset.preview;
                    self.apply_scene(preset.state);
                    self.activate_scene_preset(id, preset.name, preset.note);
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
                        self.scenes.current_mut().unwrap().note = scene.note;
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
                            presets.entries.last_mut().unwrap().note = scene.note;
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
                    renamed_preset = Some((id, p.name.clone(), name.trim().to_owned()));
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
            Command::Save(name, note) => {
                if !crate::scenes::valid_note(&note) {
                    self.scene_library.notice = Some(
                        "Enter a note of up to 2,000 characters, without control characters."
                            .into(),
                    );
                    return;
                }
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
                match presets.insert(name.clone(), state, preview) {
                    Ok(id) => {
                        presets.entries.last_mut().unwrap().note = note.trim().into();
                        next_selection = Some(Selection::Preset(id));
                        saved_preset = Some((id, name.trim().to_owned(), note.trim().to_owned()));
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
                    // Saving is a library operation, not an undoable visual edit.
                    // Finish the save-click transaction before adopting its name.
                    if saved_preset.is_some() {
                        self.finish_edit();
                    }
                    self.scene_library.presets = presets;
                    self.scene_library.selection = next_selection;
                    if let Some((id, old, new)) = renamed_preset {
                        for scene in &mut self.scenes.scenes {
                            if scene.name == old
                                && scene.preset.as_ref().is_some_and(|p| p.id == id)
                            {
                                scene.name = new.clone();
                            }
                        }
                    }
                    if let Some((id, name, note)) = saved_preset {
                        self.activate_scene_preset(id, name.clone(), note);
                        self.scene_library.select_saved(id);
                        self.scene_library.notice = Some(format!(
                            "Saved “{name}” as a new preset · {} saved presets.",
                            self.scene_library.presets.entries.len()
                        ));
                    } else {
                        self.scene_library.notice = Some("Scene preset library saved.".into());
                    }
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
