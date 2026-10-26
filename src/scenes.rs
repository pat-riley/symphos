//! Project scene snapshots and the shared scene selector. No audio/device runtime is serialized.
use crate::help::HoverHelp;
use crate::project::SceneState;
use serde::{Deserialize, Serialize};
mod fingerprint;
pub mod library;
mod names;

pub const MAX_SCENES: usize = 64;
pub const MAX_DOCUMENT_BYTES: usize = 64_000_000;
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct Scene {
    pub id: u64,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preset: Option<PresetLink>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview: Option<library::Preview>,
    #[serde(flatten)]
    pub state: SceneState,
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct PresetLink {
    pub id: u64,
    #[serde(with = "fingerprint")]
    pub baseline: u64,
    pub edited: bool,
}
impl Scene {
    pub fn display_name(&self) -> String {
        format!(
            "{}{}",
            self.name,
            if self.preset.as_ref().is_some_and(|p| p.edited) {
                " (edited)"
            } else {
                ""
            }
        )
    }
}
#[derive(Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Book {
    pub scenes: Vec<Scene>,
    pub active_scene: u64,
    pub next_scene_id: u64,
    /// Imported media not currently referenced by saved scene settings.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub media: Vec<crate::media::Resource>,
}
impl Book {
    pub fn new(state: SceneState) -> Self {
        Self {
            scenes: vec![Scene {
                id: 1,
                name: "Scene 1".into(),
                preset: None,
                preview: None,
                state,
            }],
            active_scene: 1,
            next_scene_id: 2,
            media: Vec::new(),
        }
    }
    pub fn current(&self) -> Option<&Scene> {
        self.scenes.iter().find(|s| s.id == self.active_scene)
    }
    pub fn current_mut(&mut self) -> Option<&mut Scene> {
        self.scenes.iter_mut().find(|s| s.id == self.active_scene)
    }
    pub fn store(&mut self, state: SceneState) {
        // Archive only resources no longer referenced by the new scene book.
        let previous = self.current().map(|scene| scene.state.clone());
        if let Some(scene) = self.scenes.iter_mut().find(|s| s.id == self.active_scene) {
            scene.state = state;
        } else if self.scenes.is_empty() {
            let media = std::mem::take(&mut self.media);
            *self = Self::new(state);
            self.media = media;
        }
        if let Some(previous) = previous {
            for image in crate::media::scene_images(&previous) {
                if !self
                    .scenes
                    .iter()
                    .any(|scene| crate::media::scene_images(&scene.state).any(|used| used == image))
                {
                    crate::media::remember(&mut self.media, image.clone());
                }
            }
            for (name, data) in crate::media::scene_files(&previous) {
                if !self.scenes.iter().any(|scene| {
                    crate::media::scene_files(&scene.state).any(|used| used == (name, data))
                }) {
                    crate::media::remember_file(&mut self.media, name, data);
                }
            }
        }
        self.media.retain(|resource| match resource {
            crate::media::Resource::Image(image) => !self
                .scenes
                .iter()
                .any(|scene| crate::media::scene_images(&scene.state).any(|used| used == image)),
            crate::media::Resource::File(file) => !self.scenes.iter().any(|scene| {
                crate::media::scene_files(&scene.state)
                    .any(|(name, data)| name == file.name && data == file.data)
            }),
        });
    }
    pub fn switch(&mut self, id: u64) -> Option<SceneState> {
        let state = self.scenes.iter().find(|s| s.id == id)?.state.clone();
        self.active_scene = id;
        Some(state)
    }
    pub fn insert(&mut self, name: String, state: SceneState) -> Option<u64> {
        if self.scenes.len() >= MAX_SCENES || !valid_name(&name) {
            return None;
        }
        let next = self.next_scene_id.checked_add(1)?;
        let id = self.next_scene_id;
        self.next_scene_id = next;
        let position = self
            .scenes
            .iter()
            .position(|s| s.id == self.active_scene)
            .map_or(self.scenes.len(), |i| i + 1);
        self.scenes.insert(
            position,
            Scene {
                id,
                name: name.trim().into(),
                preset: None,
                preview: None,
                state,
            },
        );
        self.active_scene = id;
        Some(id)
    }
    pub fn rename(&mut self, id: u64, name: &str) {
        if valid_name(name)
            && let Some(scene) = self.scenes.iter_mut().find(|s| s.id == id)
        {
            scene.name = name.trim().into();
            scene.preset = None;
        }
    }
    pub fn move_by(&mut self, id: u64, delta: isize) {
        if let Some(i) = self.scenes.iter().position(|s| s.id == id) {
            let target = (i as isize + delta).clamp(0, self.scenes.len() as isize - 1) as usize;
            let scene = self.scenes.remove(i);
            self.scenes.insert(target, scene);
        }
    }
    pub fn remove(&mut self, id: u64) -> Option<SceneState> {
        if self.scenes.len() <= 1 {
            return None;
        }
        let i = self.scenes.iter().position(|s| s.id == id)?;
        let removed = self.scenes.remove(i);
        crate::media::preserve_scene(&mut self.media, &removed.state);
        if self.active_scene == id {
            self.active_scene = self.scenes[i.min(self.scenes.len() - 1)].id;
            return self.current().map(|s| s.state.clone());
        }
        None
    }
    pub fn cycle(&self, step: isize) -> u64 {
        if self.scenes.is_empty() {
            return self.active_scene;
        }
        let i = self
            .scenes
            .iter()
            .position(|s| s.id == self.active_scene)
            .unwrap_or(0);
        self.scenes[(i as isize + step).rem_euclid(self.scenes.len() as isize) as usize].id
    }
    pub fn valid(&self) -> bool {
        let mut ids = std::collections::HashSet::new();
        !self.scenes.is_empty()
            && self.media.len() <= crate::media::MAX_ASSETS
            && self.media.iter().all(crate::media::Resource::valid)
            && self.scenes.len() <= MAX_SCENES
            && self.current().is_some()
            && self.scenes.iter().all(|s| {
                s.id > 0
                    && s.id < self.next_scene_id
                    && ids.insert(s.id)
                    && valid_name(&s.name)
                    && s.preset.as_ref().is_none_or(|p| p.id > 0)
                    && s.state.validate()
                    && s.preview.as_ref().is_none_or(library::Preview::valid)
            })
    }
}
fn valid_name(name: &str) -> bool {
    !name.trim().is_empty() && name.len() <= 120 && !name.chars().any(char::is_control)
}

pub enum Action {
    Switch(u64),
    New,
    Duplicate,
    Rename(u64, String),
    Move(u64, isize),
    Delete(u64),
    Presets,
    Library,
    SavePreset,
}
#[derive(Default)]
pub struct Browser {
    rename: Option<(u64, String)>,
    delete: Option<u64>,
    last_change_check: Option<std::time::Instant>,
}
impl Browser {
    pub fn check_edits_due(&mut self) -> bool {
        let now = std::time::Instant::now();
        if self
            .last_change_check
            .is_some_and(|last| now.duration_since(last).as_millis() < 100)
        {
            return false;
        }
        self.last_change_check = Some(now);
        true
    }
    pub fn rename_current(&mut self, book: &Book) {
        if let Some(scene) = book.current() {
            self.rename = Some((scene.id, scene.name.clone()));
        }
    }
    pub fn delete_current(&mut self, book: &Book) {
        if book.scenes.len() > 1 {
            self.delete = Some(book.active_scene);
        }
    }
    pub fn dialog_open(&self) -> bool {
        self.rename.is_some() || self.delete.is_some()
    }

    pub fn toolbar(
        &mut self,
        ui: &mut egui::Ui,
        book: &Book,
        theme: &crate::theme::AppTheme,
    ) -> Option<Action> {
        use crate::workspace_ui::NAV_CONTROL_HEIGHT as H;
        let mut action = None;
        let width = ui.available_width().min(590.0);
        let rect = egui::Rect::from_center_size(ui.max_rect().center(), egui::vec2(width, H));
        let mut row = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("scene-selector")
                .max_rect(rect),
        );
        // One quiet surface, with adjacent segments instead of separate buttons.
        let base = theme.navigation;
        let text = base.lerp_to_gamma(theme.foreground, 0.80);
        let divider = egui::Stroke::new(1.0, base.lerp_to_gamma(theme.foreground, 0.16));
        row.painter().rect_filled(rect, 2.0, base);
        row.spacing_mut().item_spacing.x = 0.0;
        row.spacing_mut().button_padding = egui::vec2(6.0, 3.0);
        let visuals = row.visuals_mut();
        visuals.override_text_color = Some(text);
        visuals.panel_fill = base;
        visuals.window_fill = base;
        visuals.window_stroke = divider;
        visuals.selection.bg_fill = base.lerp_to_gamma(theme.foreground, 0.10);
        visuals.selection.stroke = egui::Stroke::new(1.0, text);
        for (widget, amount) in [
            (&mut visuals.widgets.inactive, 0.0),
            (&mut visuals.widgets.hovered, 0.055),
            (&mut visuals.widgets.active, 0.09),
            (&mut visuals.widgets.open, 0.055),
        ] {
            widget.bg_fill = base.lerp_to_gamma(theme.foreground, amount);
            widget.weak_bg_fill = widget.bg_fill;
            widget.bg_stroke = egui::Stroke::NONE;
            widget.fg_stroke.color = text;
            widget.corner_radius = egui::CornerRadius::ZERO;
            widget.expansion = 0.0;
        }
        let picker_width = (width - H * 3.0 - 64.0).max(1.0);
        row.horizontal_centered(|ui| {
            if ui.add_enabled(book.scenes.len()>1, egui::Button::new("‹").min_size(egui::Vec2::splat(H))).help_text("Previous scene · recalls Signals and Visualize together").clicked() { action=Some(Action::Switch(book.cycle(-1))); }
            let current = book.current();
            let number = book.scenes.iter().position(|s| s.id == book.active_scene).unwrap_or(0)+1;
            let label = format!("{number:02} · {}", current.map_or_else(|| "Scene 1".into(), Scene::display_name));
            let button = ui.add_sized([picker_width,H],egui::Button::new(label).truncate()).help_text("Scenes include signals, mappings, components and macros. Switching keeps your edits; File → Save Project saves every scene.");
            let arrow = button.rect.right_center() - egui::vec2(11.0, 0.0);
            ui.painter().add(egui::Shape::convex_polygon(
                vec![arrow + egui::vec2(-3.5, -2.0), arrow + egui::vec2(3.5, -2.0), arrow + egui::vec2(0.0, 2.5)],
                text, egui::Stroke::NONE,
            ));
            egui::Popup::menu(&button).show(|ui| {
                ui.set_min_width(300.0);
                ui.strong("Project scenes");
                egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                    for (i,scene) in book.scenes.iter().enumerate() {
                        if ui.selectable_label(scene.id==book.active_scene,format!("{:02}   {}",i+1,scene.display_name())).clicked() { action=Some(Action::Switch(scene.id));ui.close(); }
                    }
                });
                ui.separator();
                for (label, command) in [("New empty scene",Action::New),("Duplicate current scene",Action::Duplicate)] {
                    if ui.add_enabled(book.scenes.len()<MAX_SCENES,egui::Button::new(label)).clicked(){action=Some(command);ui.close();}
                }
                if let Some(scene)=current {
                    if ui.button("Rename scene…").clicked(){self.rename=Some((scene.id,scene.name.clone()));ui.close();}
                    ui.horizontal(|ui| {
                        if ui.add_enabled(number>1,egui::Button::new("Move earlier")).clicked(){action=Some(Action::Move(scene.id,-1));ui.close();}
                        if ui.add_enabled(number<book.scenes.len(),egui::Button::new("Move later")).clicked(){action=Some(Action::Move(scene.id,1));ui.close();}
                    });
                    if ui.add_enabled(book.scenes.len()>1,egui::Button::new("Delete scene…")).clicked(){self.delete=Some(scene.id);ui.close();}
                }
                ui.separator();
                if ui.button("Visual preset library…").help_text("Apply an existing visual preset to this scene. It does not replace this scene's channels.").clicked(){action=Some(Action::Presets);ui.close();}
                ui.small("Edits stay with each scene. Save the project to keep them on disk.");
            });
            if ui.add_enabled(book.scenes.len()>1,egui::Button::new("›").min_size(egui::Vec2::splat(H))).help_text("Next scene").clicked(){action=Some(Action::Switch(book.cycle(1)));}
            if ui.add_sized([64.0,H],egui::Button::new("Library")).help_text("Manage project scenes and reusable scene presets.").clicked(){action=Some(Action::Library);}
            if crate::icons::sized_button(ui, crate::icons::Icon::Save, false, H, "Save the current scene as a named preset, including signals and macros.").clicked(){action=Some(Action::SavePreset);}
        });
        for offset in [H, H + picker_width, H * 2.0 + picker_width, width - H] {
            row.painter()
                .vline(rect.left() + offset, rect.y_range(), divider);
        }
        row.painter()
            .rect_stroke(rect, 2.0, divider, egui::StrokeKind::Inside);
        action
    }
    pub fn dialogs(&mut self, ctx: &egui::Context, book: &Book) -> Option<Action> {
        let mut action = None;
        let mut close = false;
        if let Some((id, name)) = &mut self.rename {
            egui::Window::new("Rename scene")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    let field = ui.text_edit_singleline(name);
                    let enter = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    ui.horizontal(|ui| {
                        if (ui
                            .add_enabled(valid_name(name), egui::Button::new("Rename"))
                            .clicked()
                            || enter)
                            && valid_name(name)
                        {
                            action = Some(Action::Rename(*id, name.clone()));
                            close = true;
                        }
                        if ui.button("Cancel").clicked() {
                            close = true;
                        }
                    });
                });
        }
        if close {
            self.rename = None;
        }
        close = false;
        if let Some(id) = self.delete {
            let name = book
                .scenes
                .iter()
                .find(|s| s.id == id)
                .map_or("this scene", |s| s.name.as_str());
            egui::Window::new("Delete scene?")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label(format!(
                        "Delete “{name}” and its saved setup from this project?"
                    ));
                    ui.horizontal(|ui| {
                        if ui.button("Delete scene").clicked() {
                            action = Some(Action::Delete(id));
                            close = true;
                        }
                        if ui.button("Cancel").clicked() {
                            close = true;
                        }
                    });
                });
        }
        if close {
            self.delete = None;
        }
        action
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        modules::{ModuleKind, ModulePane},
        project::Project,
        workspace::Tile,
    };
    fn state() -> SceneState {
        SceneState {
            background: Default::default(),
            macros: None,
            signals: Default::default(),
            panes: vec![ModulePane::new(ModuleKind::Flux).snapshot()],
            layout: Some(Tile::Pane(0)),
            selected_pane: 0,
            analysis: Default::default(),
        }
    }
    fn json(value: &impl Serialize) -> serde_json::Value {
        serde_json::to_value(value).unwrap()
    }
    #[test]
    fn scene_changes_are_independent_and_duplicate_keeps_mappings() {
        let mut first = state();
        let id = first.signals.add(crate::signals::Method::Lfo).unwrap();
        let bank = &mut first.panes[0].macros.as_mut().unwrap().banks[ModuleKind::Flux as usize];
        bank[0].signal = Some(id);
        bank[0].depth = 0.42;
        let mut book = Book::new(first.clone());
        let second = book.insert("Drop".into(), first.clone()).unwrap();
        let mut edit = book.current().unwrap().state.clone();
        edit.signals.channels[0].name = "Drop LFO".into();
        edit.signals.channels[0].smoothing_ms = 250.0;
        edit.panes[0].macros.as_mut().unwrap().banks[ModuleKind::Flux as usize][0].depth = -0.8;
        book.store(edit.clone());
        assert_eq!(json(&book.switch(1).unwrap()), json(&first));
        assert_eq!(json(&book.switch(second).unwrap()), json(&edit));
        assert_eq!(book.current().unwrap().state.signals.channels.len(), 1);
    }
    #[test]
    fn reordering_and_deleting_keep_active_identity_and_last_scene() {
        let mut book = Book::new(state());
        let second = book.insert("Build".into(), state()).unwrap();
        let third = book.insert("Drop".into(), state()).unwrap();
        book.move_by(third, -2);
        assert_eq!(book.active_scene, third);
        assert_eq!(
            book.scenes.iter().map(|s| s.id).collect::<Vec<_>>(),
            vec![third, 1, second]
        );
        assert_eq!(book.cycle(-1), second);
        assert!(book.remove(second).is_none());
        assert_eq!(book.active_scene, third);
        assert!(book.remove(third).is_some());
        assert_eq!(book.active_scene, 1);
        assert!(book.remove(1).is_none());
        assert!(book.valid());
        book.rename(1, "");
        assert_eq!(book.current().unwrap().name, "Scene 1");
    }
    #[test]
    fn v1_project_migrates_to_one_scene_and_v2_keeps_all_scenes() {
        let mut old = json(&state());
        old["format"] = "symphos-project".into();
        old["version"] = 1.into();
        let mut project = Project::decode(&serde_json::to_vec(&old).unwrap()).unwrap();
        assert_eq!(project.scenes.scenes.len(), 1);
        assert_eq!(json(&project.scenes.current().unwrap().state), {
            let mut v = old.clone();
            v.as_object_mut().unwrap().remove("format");
            v.as_object_mut().unwrap().remove("version");
            v
        });
        let id = project.scenes.insert("Finale".into(), state()).unwrap();
        let path = std::env::temp_dir().join(format!("symphos-scenes-{}.json", std::process::id()));
        project.save(&path).unwrap();
        let loaded = Project::open(&path).unwrap();
        assert_eq!(json(&loaded), json(&project));
        assert_eq!(loaded.scenes.active_scene, id);
        let _ = std::fs::remove_file(path);
    }
    #[test]
    fn invalid_scene_sets_are_rejected_without_accepting_partial_projects() {
        let base = Project {
            format: "symphos-project".into(),
            version: 2,
            scenes: Book::new(state()),
        };
        for mutation in 0..6 {
            let mut p = base.clone();
            match mutation {
                0 => p.scenes.scenes.clear(),
                1 => p.scenes.active_scene = 999,
                2 => p.scenes.scenes.push(p.scenes.scenes[0].clone()),
                3 => p.scenes.next_scene_id = 1,
                4 => p.scenes.scenes[0].name = "\n".into(),
                _ => p.scenes.scenes[0].state.panes.clear(),
            }
            assert!(Project::decode(&serde_json::to_vec(&p).unwrap()).is_err());
        }
        let mut unknown = json(&base);
        unknown["version"] = 999.into();
        assert!(Project::decode(&serde_json::to_vec(&unknown).unwrap()).is_err());
    }
    #[test]
    fn scene_limit_and_invalid_names_leave_the_book_unchanged() {
        let mut book = Book::new(state());
        assert!(book.insert(" ".into(), state()).is_none());
        for i in 1..MAX_SCENES {
            assert!(book.insert(format!("Scene {i}"), state()).is_some());
        }
        let before = json(&book);
        assert!(book.insert("One more".into(), state()).is_none());
        assert_eq!(json(&book), before);
        assert!(book.valid());
    }
}
