//! Pane actions share the tiled workspace, settings schema, and creative undo.
use super::*;
use crate::{macros::scene::SceneMacros, modules::PaneState};

#[derive(Clone)]
pub(super) struct Clipboard {
    state: PaneState,
    macros: SceneMacros,
    signals: Vec<crate::signals::Definition>,
    inputs: Vec<crate::signals::audio::InputBus>,
}
#[derive(Clone, Copy)]
pub(super) enum Action {
    Inspect,
    Copy,
    Paste,
    Duplicate,
    PasteNew(Edge),
    Delete,
    Freeze,
    Fullscreen,
    Reset,
    Position(Edge),
    Swap(usize),
    Balance,
}
impl SymphosApp {
    pub(super) fn pane_context_menu(
        &mut self,
        ui: &mut egui::Ui,
        index: usize,
        rect: Rect,
        frame: &AnalysisFrame,
        live: bool,
    ) {
        // Observe completed secondary clicks instead of adding an input overlay:
        // cameras retain right-drag, scroll, primary drag and double-click.
        let requested = ui.rect_contains_pointer(rect)
            && ui.input(|i| {
                i.pointer.button_clicked(egui::PointerButton::Secondary)
                    && i.pointer.interact_pos().is_some_and(|p| rect.contains(p))
            });
        if requested {
            self.selected_pane = index;
            self.pane_highlighted = true;
            self.inspecting_signal = false;
            self.inspecting_macro = None;
        }
        let popup_id = ui.id().with(("pane-menu", self.panes[index].instance_id()));
        if !requested && !egui::Popup::is_id_open(ui.ctx(), popup_id) {
            return;
        }
        let base = self.theme.panel;
        let text = self.theme.foreground;
        let accent = self.theme.accent;
        let mut action = None;
        egui::Popup::new(popup_id, ui.ctx().clone(), rect, ui.layer_id())
            .at_pointer_fixed()
            .open_memory(requested.then_some(egui::SetOpenCommand::Bool(true)))
            .kind(egui::PopupKind::Menu)
            .layout(Layout::top_down_justified(Align::LEFT))
            .style(egui::style::StyleModifier::new(move |style| {
                crate::dropdown::popup_style(style, base, text, accent)
            }))
            .show(|ui| {
                ui.set_min_width(190.0);
                let mut item = |ui: &mut egui::Ui, label: &str, enabled: bool, command: Action| {
                    if ui.add_enabled(enabled, egui::Button::new(label)).clicked() {
                        action = Some(command);
                        ui.close();
                    }
                };
                item(ui, "Inspect", true, Action::Inspect);
                ui.separator();
                item(ui, "Copy pane", true, Action::Copy);
                item(
                    ui,
                    "Paste pane",
                    self.pane_clipboard.is_some(),
                    Action::Paste,
                );
                let can_add = self.open_panes().len() < MAX_PANES;
                item(ui, "Duplicate pane", can_add, Action::Duplicate);
                ui.add_enabled_ui(can_add && self.pane_clipboard.is_some(), |ui| {
                    ui.menu_button("Paste as new pane", |ui| {
                        for (edge, label) in EDGES {
                            item(ui, label, true, Action::PasteNew(edge));
                        }
                    });
                });
                ui.separator();
                let frozen = self.panes[index].is_frozen();
                item(
                    ui,
                    if frozen { "Resume pane" } else { "Freeze pane" },
                    live || frozen,
                    Action::Freeze,
                );
                item(
                    ui,
                    if self.panel_fullscreen.is_some() {
                        "Exit fullscreen"
                    } else {
                        "Fullscreen pane"
                    },
                    true,
                    Action::Fullscreen,
                );
                ui.add_enabled_ui(
                    self.open_panes().len() > 1 && self.panel_fullscreen.is_none(),
                    |ui| {
                        ui.menu_button("Position", |ui| {
                            for (edge, label) in EDGES {
                                item(ui, label, true, Action::Position(edge));
                            }
                        });
                        ui.menu_button("Swap with", |ui| {
                            for other in self.open_panes().into_iter().filter(|id| *id != index) {
                                item(
                                    ui,
                                    &format!("{} · {}", other + 1, self.panes[other].kind.label()),
                                    true,
                                    Action::Swap(other),
                                );
                            }
                        });
                        item(ui, "Balance pane sizes", true, Action::Balance);
                    },
                );
                ui.separator();
                item(ui, "Reset pane settings", true, Action::Reset);
                item(ui, "Delete pane", true, Action::Delete);
            });
        if let Some(action) = action {
            egui::Popup::close_all(ui.ctx());
            self.pane_action(index, action, frame);
        }
    }

    fn copy_pane(&self, index: usize) -> Clipboard {
        let mut state = self.panes[index].snapshot();
        // MilkDrop exports its active preset text, including unsaved edits.
        let exported = self.panes[index].export_settings();
        if let Some(settings) = state.settings.iter_mut().find(|s| s.kind() == state.kind) {
            *settings = exported;
        }
        let bank = self.signal_ui.lock().unwrap_or_else(|e| e.into_inner());
        Clipboard {
            macros: self.scene_macros.for_panes(std::slice::from_ref(&state)),
            state,
            signals: bank.bank.channels.clone(),
            inputs: bank.bank.audio_inputs.clone(),
        }
    }

    fn paste_pane(&mut self, target: usize, clipboard: &Clipboard, edge: Edge) {
        if edge != Edge::Center && self.open_panes().len() >= MAX_PANES {
            return;
        }
        let mut content = PresetContent::View {
            background: Default::default(),
            layout: Tile::Pane(0),
            panes: vec![clipboard.state.clone()],
            macros: Some(Box::new(clipboard.macros.clone())),
        };
        {
            let bank = self.signal_ui.lock().unwrap_or_else(|e| e.into_inner());
            let mut definitions = clipboard.signals.clone();
            crate::project::bind_preset_inputs(
                &mut content,
                &mut definitions,
                &clipboard.inputs,
                &bank.bank,
            );
            crate::project::bind_preset_sources(&mut content, &definitions, &bank.bank);
        }
        let PresetContent::View {
            mut panes, macros, ..
        } = content
        else {
            unreachable!()
        };
        let mut state = panes.remove(0);
        let original_id = state.instance_id.clone();
        let id = if edge == Edge::Center {
            target
        } else {
            let Some(id) = self.available_slot(state.kind) else {
                return;
            };
            if !self
                .tiles
                .as_mut()
                .is_some_and(|tree| tree.insert(target, id, edge))
            {
                return;
            }
            id
        };
        // Copies always have a distinct identity; replacement retains its destination
        // identity so Undo, MIDI targets, and other panes never alias the source.
        let destination_id = self.panes[id].instance_id().to_owned();
        let reusable: Vec<_> = self
            .scene_macros
            .slots
            .iter()
            .enumerate()
            .filter(|(_, slot)| {
                slot.target
                    .as_ref()
                    .is_some_and(|t| t.pane == destination_id)
            })
            .map(|(i, _)| i)
            .collect();
        self.scene_macros.remove_component(&destination_id);
        state.instance_id = destination_id.clone();
        if let Some(macros) = &mut state.macros {
            for slot in macros.banks.iter_mut().flatten() {
                if let Some(target) = &mut slot.target
                    && target.pane == original_id
                {
                    target.pane = destination_id.clone();
                }
            }
        }
        let mut replacement = ModulePane::new(state.kind);
        replacement.restore(&state);
        self.panes[id] = replacement;
        let mut skipped = 0;
        if let Some(macros) = macros {
            for mut slot in macros
                .slots
                .into_iter()
                .filter(|s| s.target.as_ref().is_some_and(|t| t.pane == original_id))
            {
                let free = self
                    .scene_macros
                    .slots
                    .iter()
                    .enumerate()
                    .position(|(i, s)| {
                        s == &crate::macros::Slot::default()
                            || (reusable.contains(&i) && s.target.is_none())
                    });
                if let Some(free) = free {
                    slot.target.as_mut().unwrap().pane = destination_id.clone();
                    self.scene_macros.slots[free] = slot;
                } else {
                    skipped += 1;
                }
            }
            for mut mapping in macros
                .midi
                .into_iter()
                .filter(|m| m.target.pane == original_id)
            {
                if self.scene_macros.midi.len() < 256 {
                    mapping.target.pane = destination_id.clone();
                    self.scene_macros.midi.push(mapping);
                } else {
                    skipped += 1;
                }
            }
        }
        if skipped > 0 {
            self.storage.notice = Some(format!(
                "Pane pasted. {skipped} modulation/MIDI assignments could not be copied because the scene has no free mapping slots."
            ));
        }
        self.panel_fullscreen = None;
        self.selected_pane = id;
        self.pane_highlighted = true;
        self.inspecting_signal = false;
        self.inspecting_macro = None;
    }

    pub(super) fn pane_action(&mut self, index: usize, action: Action, frame: &AnalysisFrame) {
        match action {
            Action::Inspect => {
                self.inspect_panel(index);
                return;
            }
            Action::Copy => {
                self.settings_clipboard = Some(self.panes[index].export_settings());
                self.pane_clipboard = Some(self.copy_pane(index));
                return;
            }
            Action::Freeze => {
                self.panes[index].toggle_freeze(frame, Instant::now());
                return;
            }
            Action::Fullscreen => {
                self.toggle_panel_fullscreen(index, Instant::now());
                return;
            }
            _ => {}
        }
        self.begin_edit(true);
        match action {
            Action::Paste | Action::PasteNew(_) => {
                if let Some(clipboard) = self.pane_clipboard.clone() {
                    self.paste_pane(
                        index,
                        &clipboard,
                        if let Action::PasteNew(edge) = action {
                            edge
                        } else {
                            Edge::Center
                        },
                    );
                }
            }
            Action::Duplicate => {
                let clipboard = self.copy_pane(index);
                self.paste_pane(index, &clipboard, Edge::Right);
            }
            Action::Delete => self.close_pane(index),
            Action::Reset => self.panes[index].reset_settings(),
            Action::Position(edge) => {
                if let Some(tree) = self.tiles.take() {
                    self.tiles = if let Some(remaining) = tree.clone().remove(index) {
                        let (axis, first) = match edge {
                            Edge::Left => (Axis::Horizontal, true),
                            Edge::Right => (Axis::Horizontal, false),
                            Edge::Top => (Axis::Vertical, true),
                            _ => (Axis::Vertical, false),
                        };
                        Some(if first {
                            Tile::split(axis, 0.5, Tile::Pane(index), remaining)
                        } else {
                            Tile::split(axis, 0.5, remaining, Tile::Pane(index))
                        })
                    } else {
                        Some(tree)
                    };
                }
            }
            Action::Swap(other) => {
                if let Some(tree) = &mut self.tiles {
                    tree.move_pane(index, other, Edge::Center);
                }
            }
            Action::Balance => {
                if let Some(tree) = &mut self.tiles {
                    equalize_splits(tree);
                }
            }
            _ => {}
        }
        self.finish_edit();
    }
}
const EDGES: [(Edge, &str); 4] = [
    (Edge::Left, "Left"),
    (Edge::Right, "Right"),
    (Edge::Top, "Above"),
    (Edge::Bottom, "Below"),
];
