//! Creative undo transactions. Runtime audio, simulation and device connections
//! never enter the journal; continuous drags and text edits form one transaction.
use super::*;
use std::{collections::VecDeque, sync::Arc};

#[derive(Clone)]
struct Snapshot {
    scenes: Arc<crate::scenes::Book>,
    current: crate::project::SceneState,
}
impl PartialEq for Snapshot {
    fn eq(&self, other: &Self) -> bool {
        let canonical = |state: &crate::project::SceneState| {
            let mut state = state.clone();
            state.selected_pane = 0;
            if let Some(macros) = &mut state.macros {
                macros.open = false;
            }
            for pane in &mut state.panes {
                pane.sections.fill(0);
                if let Some(macros) = &mut pane.macros {
                    macros.open = false;
                }
                for settings in &mut pane.settings {
                    if let Some(yaw) = settings.automatic_yaw() {
                        *yaw = 0.0;
                    }
                }
            }
            state
        };
        // The active book entry is a cache, refreshed by Library/Save/scene
        // switching. Compare its live state instead, so refreshing that cache
        // cannot create an edit. Thumbnails and navigation are presentation.
        (Arc::ptr_eq(&self.scenes, &other.scenes)
            || (self.scenes.active_scene == other.scenes.active_scene
                && self.scenes.next_scene_id == other.scenes.next_scene_id
                && self.scenes.scenes.len() == other.scenes.scenes.len()
                && self
                    .scenes
                    .scenes
                    .iter()
                    .zip(&other.scenes.scenes)
                    .all(|(before, after)| {
                        before.id == after.id
                            && before.name == after.name
                            && (before.id == self.scenes.active_scene
                                || canonical(&before.state) == canonical(&after.state))
                    })))
            && canonical(&self.current) == canonical(&other.current)
    }
}
struct Edit {
    before: Snapshot,
    after: Snapshot,
}
#[derive(Default)]
pub(super) struct History {
    undo: VecDeque<Edit>,
    redo: Vec<Edit>,
    pending: Option<Snapshot>,
    last_input: Option<Instant>,
    book_cache: Option<Arc<crate::scenes::Book>>,
}
impl History {
    fn book_snapshot(&mut self, book: &crate::scenes::Book) -> Arc<crate::scenes::Book> {
        if self
            .book_cache
            .as_ref()
            .is_none_or(|saved| saved.as_ref() != book)
        {
            self.book_cache = Some(Arc::new(book.clone()));
        }
        self.book_cache.as_ref().unwrap().clone()
    }
    fn finish(&mut self, after: Snapshot) {
        if let Some(before) = self.pending.take()
            && before != after
        {
            self.undo.push_back(Edit { before, after });
            if self.undo.len() > 64 {
                self.undo.pop_front();
            }
            self.redo.clear();
        }
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty() || self.pending.is_some()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}
impl SymphosApp {
    fn history_snapshot(&mut self) -> Snapshot {
        let mut current = self.capture_scene();
        // Inspector navigation and dock visibility are not creative edits.
        current.selected_pane = 0;
        if let Some(macros) = &mut current.macros {
            macros.open = false;
        }
        for pane in &mut current.panes {
            pane.sections.fill(0);
            if let Some(macros) = &mut pane.macros {
                macros.open = false;
            }
        }
        Snapshot {
            scenes: self.history.book_snapshot(&self.scenes),
            current,
        }
    }
    pub(super) fn begin_edit(&mut self, separate: bool) {
        if separate {
            self.finish_edit();
        }
        if self.history.pending.is_none() {
            self.history.pending = Some(self.history_snapshot());
        }
        self.history.last_input = Some(Instant::now());
    }
    pub(super) fn finish_edit(&mut self) {
        if self.history.pending.is_some() {
            let state = self.history_snapshot();
            self.history.finish(state);
        }
    }
    pub(super) fn settle_edit(&mut self, ctx: &egui::Context) {
        if self.history.pending.is_some()
            && !ctx.input(|i| i.pointer.any_down())
            && !self
                .signal_ui
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .learning_range()
            && self
                .history
                .last_input
                .is_some_and(|t| t.elapsed() >= Duration::from_millis(350))
        {
            self.finish_edit();
        }
    }
    pub(super) fn undo_edit(&mut self, redo: bool) {
        self.finish_edit();
        let edit = if redo {
            self.history.redo.pop()
        } else {
            self.history.undo.pop_back()
        };
        let Some(edit) = edit else {
            return;
        };
        let snapshot = if redo {
            edit.after.clone()
        } else {
            edit.before.clone()
        };
        self.restore_edit(snapshot);
        if redo {
            self.history.undo.push_back(edit);
        } else {
            self.history.redo.push(edit);
        }
    }
    fn restore_edit(&mut self, snapshot: Snapshot) {
        let scene_changed = self.scenes.active_scene != snapshot.scenes.active_scene;
        if self.scenes != *snapshot.scenes {
            self.scenes = Arc::unwrap_or_clone(snapshot.scenes);
        }
        let mut state = snapshot.current;
        let open = self.scene_macros.open;
        if scene_changed {
            state.selected_pane = state
                .layout
                .as_ref()
                .map_or(0, |layout| layout.ids().first().copied().unwrap_or(0));
            self.apply_scene(state);
        } else {
            // Reuse surviving visualizers so undoing a knob does not clear GPU
            // particles, captured history, or a running audio stream.
            let mut old = std::mem::take(&mut self.panes);
            self.panes = state
                .panes
                .iter()
                .map(|saved| {
                    if let Some(index) = old
                        .iter()
                        .position(|p| p.instance_id() == saved.instance_id)
                    {
                        let mut pane = old.remove(index);
                        pane.restore_edit(saved);
                        pane
                    } else {
                        let mut pane = ModulePane::new(saved.kind);
                        pane.restore(saved);
                        pane
                    }
                })
                .collect();
            self.tiles = state.layout.take();
            self.selected_pane = self.selected_pane.min(self.panes.len().saturating_sub(1));
            if let Some(first) = self.open_panes().first()
                && !self.open_panes().contains(&self.selected_pane)
            {
                self.selected_pane = *first;
            }
            self.scene_macros = state.macros.take().unwrap_or_default();
            let mut panel = self.signal_ui.lock().unwrap_or_else(|e| e.into_inner());
            if panel.bank != state.signals {
                panel.replace_bank(state.signals);
            }
            *self
                .engine
                .analysis()
                .settings
                .lock()
                .unwrap_or_else(|e| e.into_inner()) = state.analysis;
        }
        self.scene_macros.open = open;
        self.inspecting_macro = None;
        self.pending_close = None;
        self.panel_fullscreen = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn snapshot(width: f32) -> Snapshot {
        let pane = ModulePane::new(ModuleKind::Spectrum);
        let state = crate::project::SceneState {
            macros: Some(Default::default()),
            signals: Default::default(),
            panes: vec![pane.snapshot()],
            layout: Some(Tile::Pane(0)),
            selected_pane: 0,
            analysis: Default::default(),
        };
        let mut s = Snapshot {
            scenes: Arc::new(crate::scenes::Book::new(state.clone())),
            current: state,
        };
        s.current.analysis.gain_db = width;
        s
    }
    #[test]
    fn coalesces_gestures_and_invalidates_redo_only_on_real_edits() {
        let mut h = History::default();
        let a = snapshot(-80.0);
        let mut b = a.clone();
        b.current.analysis.gain_db = -60.0;
        h.pending = Some(a.clone());
        h.finish(b.clone());
        assert_eq!(h.undo.len(), 1);
        h.redo.push(h.undo.pop_back().unwrap());
        h.pending = Some(a.clone());
        h.finish(a.clone());
        assert!(h.can_redo());
        h.pending = Some(a);
        h.finish(b);
        assert!(!h.can_redo());
    }

    #[test]
    fn parameter_history_shares_unchanged_books_and_preserves_renamed_history() {
        let mut history = History::default();
        let mut book = (*snapshot(-80.0).scenes).clone();
        let original = history.book_snapshot(&book);
        for _ in 0..64 {
            assert!(Arc::ptr_eq(&original, &history.book_snapshot(&book)));
        }
        book.rename(book.active_scene, "Renamed scene");
        let renamed = history.book_snapshot(&book);
        assert!(!Arc::ptr_eq(&original, &renamed));
        assert_eq!(original.current().unwrap().name, "Scene 1");
        assert_eq!(renamed.current().unwrap().name, "Renamed scene");
        assert!(Arc::ptr_eq(&renamed, &history.book_snapshot(&book)));
    }
}
