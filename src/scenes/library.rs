//! Full-scene presets and their management view. Device and GUI preferences stay local.
use super::{Book, Scene, valid_name};
use crate::{help::HoverHelp, project::SceneState};
use egui::{Rect, Vec2};
mod view;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::Path};

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct Preview {
    width: usize,
    height: usize,
    rgb: std::sync::Arc<[u8]>,
}
impl Preview {
    pub fn valid(&self) -> bool {
        self.width > 0
            && self.width <= 240
            && self.height > 0
            && self.height <= 135
            && self.rgb.len() == self.width * self.height * 3
    }
    pub fn capture(image: &egui::ColorImage, rect: Rect, scale: f32) -> Option<Self> {
        let min = (rect.min.to_vec2() * scale).max(Vec2::ZERO);
        let max = (rect.max.to_vec2() * scale)
            .min(egui::vec2(image.size[0] as f32, image.size[1] as f32));
        let size = max - min;
        if size.x < 1.0 || size.y < 1.0 {
            return None;
        }
        let factor = (240.0 / size.x).min(135.0 / size.y).min(1.0);
        let width = (size.x * factor).round().max(1.0) as usize;
        let height = (size.y * factor).round().max(1.0) as usize;
        let mut rgb = Vec::with_capacity(width * height * 3);
        for y in 0..height {
            for x in 0..width {
                let px = (min.x + (x as f32 + 0.5) * size.x / width as f32) as usize;
                let py = (min.y + (y as f32 + 0.5) * size.y / height as f32) as usize;
                let c = image[(px.min(image.size[0] - 1), py.min(image.size[1] - 1))];
                rgb.extend_from_slice(&[c.r(), c.g(), c.b()]);
            }
        }
        Some(Self {
            width,
            height,
            rgb: rgb.into(),
        })
    }
    fn image(&self) -> egui::ColorImage {
        egui::ColorImage::from_rgb([self.width, self.height], &self.rgb)
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Presets {
    version: u32,
    pub entries: Vec<Scene>,
    next_id: u64,
}
impl Default for Presets {
    fn default() -> Self {
        Self {
            version: 1,
            entries: Vec::new(),
            next_id: 1,
        }
    }
}
impl Presets {
    pub fn load(dir: &Path) -> Result<Self, String> {
        let path = dir.join("scene-presets.json");
        if !path.exists() {
            return Ok(Self::default());
        }
        if std::fs::metadata(&path).map_err(|e| e.to_string())?.len()
            > super::MAX_DOCUMENT_BYTES as u64
        {
            return Err(
                "Scene preset library exceeds the 64 MB limit. Original file preserved.".into(),
            );
        }
        let library: Self =
            serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        if !library.valid() {
            return Err("Invalid scene preset library. Original file preserved.".into());
        }
        Ok(library)
    }
    fn valid(&self) -> bool {
        let mut ids = std::collections::HashSet::new();
        self.version == 1
            && self.entries.len() <= 256
            && self.next_id > 0
            && self.entries.iter().all(|s| {
                s.id > 0
                    && s.id < self.next_id
                    && ids.insert(s.id)
                    && valid_name(&s.name)
                    && s.state.validate()
                    && s.preview.as_ref().is_none_or(Preview::valid)
            })
    }
    pub fn save(&self, dir: &Path) -> Result<(), String> {
        if !self.valid() {
            return Err("Invalid scene preset; no file was changed.".into());
        }
        let bytes = serde_json::to_vec(self).map_err(|e| e.to_string())?;
        if bytes.len() > super::MAX_DOCUMENT_BYTES {
            return Err("Scene preset library exceeds the 64 MB limit.".into());
        }
        crate::workspace::atomic_write(&dir.join("scene-presets.json"), &bytes)
            .map_err(|e| e.to_string())
    }
    pub fn insert(
        &mut self,
        name: String,
        state: SceneState,
        preview: Option<Preview>,
    ) -> Result<u64, String> {
        if !valid_name(&name) || self.entries.len() >= 256 {
            return Err("Enter a short name. The library supports up to 256 presets.".into());
        }
        let next = self
            .next_id
            .checked_add(1)
            .ok_or("Preset ID limit reached")?;
        let id = self.next_id;
        self.next_id = next;
        self.entries.push(Scene {
            id,
            name: name.trim().into(),
            preset: None,
            state,
            preview,
        });
        Ok(id)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Selection {
    Scene(u64),
    Preset(u64),
}
#[derive(Clone, Copy)]
pub enum CapturePurpose {
    Open,
    Save,
    Refresh,
}
pub struct Capture {
    pub purpose: CapturePurpose,
    pub scene: u64,
    pub rect: Rect,
    pub scale: f32,
    pub started: std::time::Instant,
}
pub enum Command {
    New,
    Load(Selection),
    Duplicate(Selection),
    Rename(Selection, String),
    Delete(Selection),
    Reorder(Selection, usize),
    Save(String),
    Refresh,
    LegacyPresets,
}
enum Dialog {
    Save(String),
    Rename(Selection, String),
    Delete(Selection, String),
}
#[derive(Default)]
pub struct Library {
    pub open: bool,
    pub presets: Presets,
    pub selection: Option<Selection>,
    pub capture: Option<Capture>,
    pub canvas: Option<Rect>,
    pub notice: Option<String>,
    pub load_failed: bool,
    loaded: bool,
    preset_tab: bool,
    list_view: bool,
    view_path: Option<std::path::PathBuf>,
    query: String,
    dialog: Option<Dialog>,
    focus_dialog: bool,
    last_suggested_name: Option<String>,
    textures: HashMap<Selection, egui::TextureHandle>,
}
impl Library {
    pub fn select_saved(&mut self, id: u64) {
        self.preset_tab = true;
        self.query.clear();
        self.selection = Some(Selection::Preset(id));
    }
    pub fn ensure_loaded(&mut self, dir: &Path) {
        if self.loaded {
            return;
        }
        self.loaded = true;
        self.view_path = Some(dir.join("scene-library-view.json"));
        self.list_view = self
            .view_path
            .as_ref()
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|b| serde_json::from_slice::<bool>(&b).ok())
            .unwrap_or(false);
        match Presets::load(dir) {
            Ok(p) => self.presets = p,
            Err(e) => {
                self.notice = Some(e);
                self.load_failed = true;
            }
        }
    }
    pub fn busy(&self) -> bool {
        self.open || self.dialog.is_some() || self.capture.is_some()
    }
    pub fn invalidate(&mut self, item: Selection) {
        self.textures.remove(&item);
    }
    pub fn open(&mut self, book: &Book) {
        self.open = true;
        self.query.clear();
        let preset = book.current().and_then(|s| {
            s.preset.as_ref().and_then(|link| {
                self.presets
                    .entries
                    .iter()
                    .find(|p| p.id == link.id && p.name == s.name)
            })
        });
        self.preset_tab = preset.is_some();
        self.selection = Some(preset.map_or(Selection::Scene(book.active_scene), |p| {
            Selection::Preset(p.id)
        }));
    }
    pub fn save_dialog(&mut self, book: &Book) {
        let name = super::names::suggest(book, &self.presets, self.last_suggested_name.as_deref());
        self.last_suggested_name = Some(name.clone());
        self.dialog = Some(Dialog::Save(name));
        self.focus_dialog = true;
    }
    pub fn finish_dialog(&mut self) {
        self.dialog = None;
    }
    pub fn reset_project(&mut self) {
        self.open = false;
        self.selection = None;
        self.capture = None;
        self.dialog = None;
        self.textures
            .retain(|k, _| matches!(k, Selection::Preset(_)));
    }
    pub fn dialogs(&mut self, ctx: &egui::Context) -> Option<Command> {
        let mut command = None;
        let mut cancel = false;
        let Some(dialog) = &mut self.dialog else {
            return None;
        };
        let can_save =
            matches!(dialog, Dialog::Rename(Selection::Scene(_), _)) || !self.load_failed;
        let title = if matches!(dialog, Dialog::Save(_)) {
            "Save scene preset"
        } else {
            "Rename scene"
        };
        egui::Modal::new(egui::Id::new("scene-library-dialog")).show(ctx, |ui| {
            ui.set_width(340.0);
            match dialog {
                Dialog::Save(name) | Dialog::Rename(_, name) => {
                    ui.heading(title);
                    let field =
                        ui.add(egui::TextEdit::singleline(name).desired_width(f32::INFINITY));
                    if self.focus_dialog {
                        field.request_focus();
                        self.focus_dialog = false;
                    }
                    let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
                    let valid = valid_name(name);
                    if !valid {
                        ui.small("Enter a short name, without line breaks.");
                    }
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(valid && can_save, egui::Button::new("Save"))
                            .clicked()
                            || (enter && valid && can_save)
                        {
                            command = Some(match dialog {
                                Dialog::Save(name) => Command::Save(name.clone()),
                                Dialog::Rename(key, name) => Command::Rename(*key, name.clone()),
                                _ => unreachable!(),
                            });
                        }
                        if ui.button("Cancel").clicked() {
                            cancel = true;
                        }
                    });
                }
                Dialog::Delete(key, name) => {
                    ui.heading("Delete this scene?");
                    ui.label(format!(
                        "Remove “{name}” from {}?",
                        if matches!(key, Selection::Preset(_)) {
                            "your preset library"
                        } else {
                            "this project"
                        }
                    ));
                    ui.horizontal(|ui| {
                        if ui.button("Delete").clicked() {
                            command = Some(Command::Delete(*key));
                        }
                        if ui.button("Cancel").clicked() {
                            cancel = true;
                        }
                    });
                }
            }
            if let Some(notice) = &self.notice {
                ui.label(notice);
            }
            if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
                cancel = true;
            }
        });
        if cancel {
            self.dialog = None;
        }
        command
    }
}

pub fn reorder(entries: &mut Vec<Scene>, id: u64, insertion: usize) {
    if let Some(from) = entries.iter().position(|s| s.id == id) {
        let insertion = insertion.min(entries.len());
        let to = insertion.saturating_sub(usize::from(from < insertion));
        let entry = entries.remove(from);
        entries.insert(to, entry);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::Color32;
    fn state() -> SceneState {
        SceneState {
            macros: Some(Default::default()),
            signals: Default::default(),
            panes: vec![
                crate::modules::ModulePane::new(crate::modules::ModuleKind::Flux).snapshot(),
            ],
            layout: Some(crate::workspace::Tile::Pane(0)),
            selected_pane: 0,
            analysis: Default::default(),
        }
    }

    #[test]
    fn save_dialog_prefills_three_words_regenerates_and_accepts_enter() {
        let book = Book::new(state());
        let mut library = Library::default();
        library.save_dialog(&book);
        let first = match &library.dialog {
            Some(Dialog::Save(name)) => name.clone(),
            _ => panic!(),
        };
        assert_eq!(first.split_whitespace().count(), 3);
        library
            .presets
            .insert(first.clone(), state(), None)
            .unwrap();
        library.finish_dialog();
        library.save_dialog(&book);
        let second = match &library.dialog {
            Some(Dialog::Save(name)) => name.clone(),
            _ => panic!(),
        };
        assert_ne!(first, second);
        assert_eq!(second.split_whitespace().count(), 3);
        let ctx = egui::Context::default();
        let mut out = ctx.run_ui(Default::default(), |ui| {
            library.dialogs(ui.ctx());
        });
        out.textures_delta.clear();
        let mut command = None;
        let mut out = ctx.run_ui(
            egui::RawInput {
                events: vec![egui::Event::Key {
                    key: egui::Key::Enter,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
                ..Default::default()
            },
            |ui| {
                command = library.dialogs(ui.ctx());
            },
        );
        out.textures_delta.clear();
        assert!(matches!(command, Some(Command::Save(name)) if name == second));
    }
    #[test]
    fn presets_roundtrip_complete_scene_and_reject_corrupt_library_without_rewriting() {
        let dir =
            std::env::temp_dir().join(format!("symphos-scene-presets-{}", std::process::id()));
        let mut state = state();
        state.signals.add(crate::signals::Method::Lfo).unwrap();
        let mut library = Presets::default();
        let image = egui::ColorImage::filled([10, 10], Color32::RED);
        let preview = Preview::capture(
            &image,
            Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(10.0, 10.0)),
            1.0,
        )
        .unwrap();
        library
            .insert("Full setup".into(), state.clone(), Some(preview))
            .unwrap();
        library.save(&dir).unwrap();
        let restored = Presets::load(&dir).unwrap();
        assert_eq!(
            serde_json::to_value(&restored.entries[0].state).unwrap(),
            serde_json::to_value(state).unwrap()
        );
        assert_eq!(
            restored.entries[0].preview.as_ref().unwrap().rgb[0..3],
            [255, 0, 0]
        );
        std::fs::write(dir.join("scene-presets.json"), b"broken").unwrap();
        assert!(Presets::load(&dir).is_err());
        assert_eq!(
            std::fs::read(dir.join("scene-presets.json")).unwrap(),
            b"broken"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn preview_crop_excludes_chrome_and_preserves_aspect_ratio() {
        let mut image = egui::ColorImage::filled([800, 600], Color32::RED);
        for y in 100..500 {
            for x in 100..700 {
                image[(x, y)] = Color32::BLUE;
            }
        }
        let preview = Preview::capture(
            &image,
            Rect::from_min_max(egui::pos2(50.0, 50.0), egui::pos2(350.0, 250.0)),
            2.0,
        )
        .unwrap();
        assert!(preview.valid());
        assert_eq!((preview.width, preview.height), (203, 135));
        assert!(
            preview
                .rgb
                .as_chunks::<3>()
                .0
                .iter()
                .all(|c| *c == [0, 0, 255])
        );
        assert!(Preview::capture(&image, Rect::NOTHING, 1.0).is_none());
    }
    #[test]
    fn card_click_selects_without_loading_and_drag_emits_insertion() {
        let mut book = Book::new(state());
        let second = book.insert("Second".into(), state()).unwrap();
        book.insert("Third".into(), state()).unwrap();
        let active = book.active_scene;
        let mut library = Library::default();
        library.open(&book);
        let ctx = egui::Context::default();
        let mut time = 0.0;
        let mut render = |library: &mut Library, events: Vec<egui::Event>| {
            time += 0.05;
            let mut result = None;
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1000.0, 700.0),
                    )),
                    time: Some(time),
                    events,
                    ..Default::default()
                },
                |ui| {
                    result = library.show(ui, ui.max_rect(), &book);
                },
            );
            output.textures_delta.clear();
            result
        };
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        render(&mut library, vec![]);
        let second_pos = item_rect(&ctx, Selection::Scene(second)).center();
        render(
            &mut library,
            vec![
                egui::Event::PointerMoved(second_pos),
                button(second_pos, true),
            ],
        );
        assert!(render(&mut library, vec![button(second_pos, false)]).is_none());
        assert!(library.selection == Some(Selection::Scene(second)));
        assert_eq!(book.active_scene, active);
        let from = item_rect(&ctx, Selection::Scene(1)).center();
        let third = item_rect(&ctx, Selection::Scene(3));
        let to = third.left_center() + egui::vec2(20.0, 0.0);
        render(
            &mut library,
            vec![egui::Event::PointerMoved(from), button(from, true)],
        );
        render(
            &mut library,
            vec![egui::Event::PointerMoved(from + egui::vec2(20.0, 0.0))],
        );
        assert!(egui::DragAndDrop::has_payload_of_type::<Selection>(&ctx));
        render(&mut library, vec![egui::Event::PointerMoved(to)]);
        assert!(matches!(
            render(&mut library, vec![button(to, false)]),
            Some(Command::Reorder(Selection::Scene(1), 2))
        ));
    }

    fn item_rect(ctx: &egui::Context, key: Selection) -> Rect {
        ctx.data(|d| {
            d.get_temp::<Rect>(egui::Id::new(("library-item-rect", key)))
                .unwrap()
        })
    }
    #[test]
    fn both_views_select_then_double_click_load_and_list_drag_uses_vertical_order() {
        for list in [false, true] {
            let mut book = Book::new(state());
            let second = book.insert("Second".into(), state()).unwrap();
            book.insert("Third".into(), state()).unwrap();
            let mut library = Library {
                list_view: list,
                ..Default::default()
            };
            library.open(&book);
            let ctx = egui::Context::default();
            let mut time = 0.0;
            let mut render = |library: &mut Library, events: Vec<egui::Event>| {
                time += 0.05;
                let mut result = None;
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(1000.0, 700.0),
                        )),
                        time: Some(time),
                        events,
                        ..Default::default()
                    },
                    |ui| {
                        result = library.show(ui, ui.max_rect(), &book);
                    },
                );
                output.textures_delta.clear();
                result
            };
            let button = |pos, pressed| egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Default::default(),
            };
            render(&mut library, vec![]);
            let pos = item_rect(&ctx, Selection::Scene(second)).center();
            render(
                &mut library,
                vec![egui::Event::PointerMoved(pos), button(pos, true)],
            );
            assert!(render(&mut library, vec![button(pos, false)]).is_none());
            assert!(library.selection == Some(Selection::Scene(second)));
            render(&mut library, vec![button(pos, true)]);
            assert!(
                matches!(render(&mut library,vec![button(pos,false)]),Some(Command::Load(Selection::Scene(id))) if id == second)
            );
            if list {
                for _ in 0..10 {
                    render(&mut library, vec![]);
                }
                let from = item_rect(&ctx, Selection::Scene(1)).center();
                let to = item_rect(&ctx, Selection::Scene(3)).center() - egui::vec2(0.0, 12.0);
                render(
                    &mut library,
                    vec![egui::Event::PointerMoved(from), button(from, true)],
                );
                render(
                    &mut library,
                    vec![egui::Event::PointerMoved(from + egui::vec2(0.0, 15.0))],
                );
                assert!(egui::DragAndDrop::has_payload_of_type::<Selection>(&ctx));
                render(&mut library, vec![egui::Event::PointerMoved(to)]);
                assert!(matches!(
                    render(&mut library, vec![button(to, false)]),
                    Some(Command::Reorder(Selection::Scene(1), 2))
                ));
            }
        }
    }

    #[test]
    fn list_view_preference_is_local_and_survives_reopening() {
        let dir = std::env::temp_dir().join(format!("symphos-library-view-{}", std::process::id()));
        crate::workspace::atomic_write(&dir.join("scene-library-view.json"), b"true").unwrap();
        let mut library = Library::default();
        library.ensure_loaded(&dir);
        assert!(library.list_view);
        library.open(&Book::new(state()));
        assert!(library.list_view);
        assert!(
            !serde_json::to_string(&Book::new(state()))
                .unwrap()
                .contains("list_view")
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn drag_insertion_boundaries_keep_scene_identity() {
        let mut book = Book::new(state());
        let b = book.insert("B".into(), state()).unwrap();
        let c = book.insert("C".into(), state()).unwrap();
        reorder(&mut book.scenes, 1, 3);
        assert_eq!(
            book.scenes.iter().map(|s| s.id).collect::<Vec<_>>(),
            vec![b, c, 1]
        );
        reorder(&mut book.scenes, 1, 0);
        assert_eq!(
            book.scenes.iter().map(|s| s.id).collect::<Vec<_>>(),
            vec![1, b, c]
        );
        reorder(&mut book.scenes, b, 2);
        assert_eq!(
            book.scenes.iter().map(|s| s.id).collect::<Vec<_>>(),
            vec![1, b, c]
        );
        assert_eq!(book.active_scene, c);
        assert!(book.valid());
    }
}
