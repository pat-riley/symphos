//! Track-like editing for a fixed bank of twelve performance controls.
use super::*;
use crate::help::HoverHelp;

#[derive(Clone)]
pub(super) struct MacroDrag {
    pub pane: String,
    pub kind: ModuleKind,
    pub index: usize,
}
#[derive(Clone)]
struct Clipboard {
    kind: ModuleKind,
    slot: Slot,
}
#[derive(Clone, Default)]
pub(super) struct MacroMenuState {
    rename: Option<(usize, String, bool)>,
    undo: Option<[Slot; 12]>,
    expected: Option<[Slot; 12]>,
}
impl MacroMenuState {
    pub(super) fn sync(&mut self, bank: &[Slot; 12]) {
        if self
            .expected
            .as_ref()
            .is_some_and(|previous| previous != bank)
        {
            self.undo = None;
            self.rename = None;
        }
    }
    pub(super) fn remember(&mut self, bank: &[Slot; 12]) {
        if self.expected.as_ref() != Some(bank) {
            self.expected = Some(bank.clone());
        }
    }

    pub(super) fn undo_available(&self) -> bool {
        self.undo.is_some()
    }
}
pub(super) enum MacroAction {
    Cut(usize),
    Copy(usize),
    Paste(usize),
    Duplicate(usize),
    Delete(usize),
    Rename(usize),
    RenameTo(usize, String),
    Color(usize, Option<[u8; 3]>),
    Insert(usize),
    Move(usize, usize),
    Undo,
}
const CLIPBOARD: &str = "macro-clipboard";

pub(super) fn clear_clipboard(ctx: &egui::Context) {
    ctx.data_mut(|d| d.remove::<Clipboard>(egui::Id::new(CLIPBOARD)));
}

pub(super) fn slot_title(slot: &Slot, index: usize) -> String {
    slot.name.clone().unwrap_or_else(|| {
        slot.parameter
            .as_ref()
            .map_or_else(|| format!("Macro {}", index + 1), |p| p.label.clone())
    })
}
pub(super) fn macro_menu(
    ui: &mut egui::Ui,
    index: usize,
    bank: &[Slot; 12],
    _pane: &str,
    kind: ModuleKind,
    color: egui::Color32,
    can_undo: bool,
) -> Option<MacroAction> {
    ui.set_min_width(328.0);
    if let (Some(target), Some(spec)) = (&bank[index].target, &bank[index].parameter) {
        crate::midi::mapping::menu(ui, target, spec);
    }
    let mut action = None;
    let occupied = bank[index].parameter.is_some();
    let vacant = bank.iter().any(|s| s.parameter.is_none());
    let can_paste = ui
        .data(|d| d.get_temp::<Clipboard>(egui::Id::new(CLIPBOARD)))
        .is_some_and(|clip| clip.kind == kind);
    for (label, command, enabled) in [
        ("Cut", MacroAction::Cut(index), occupied),
        ("Copy", MacroAction::Copy(index), true),
        ("Paste", MacroAction::Paste(index), can_paste),
        (
            "Duplicate",
            MacroAction::Duplicate(index),
            occupied && vacant,
        ),
        ("Delete", MacroAction::Delete(index), true),
    ] {
        let response = ui.add_enabled(enabled, egui::Button::new(label));
        let response = if label == "Duplicate" {
            response.help_text("Copy to an empty slot. The copy's modulation starts bypassed; both knobs control the same parameter.")
        } else {
            response
        };
        if response.clicked() {
            action = Some(command);
            ui.close();
        }
    }
    crate::dropdown::menu_separator(ui);
    for (label, command, enabled) in [
        ("Rename", MacroAction::Rename(index), true),
        ("Insert empty macro", MacroAction::Insert(index), vacant),
    ] {
        if ui.add_enabled(enabled, egui::Button::new(label)).clicked() {
            action = Some(command);
            ui.close();
        }
    }
    if ui
        .add_enabled(can_undo, egui::Button::new("Undo last edit"))
        .clicked()
    {
        action = Some(MacroAction::Undo);
        ui.close();
    }
    crate::dropdown::menu_separator(ui);
    ui.small("Macro color");
    if let Some(color) = signals::ui::color_palette(ui, color) {
        action = Some(MacroAction::Color(index, Some(color)));
    }
    if ui.button("Automatic color").clicked() {
        action = Some(MacroAction::Color(index, None));
        ui.close();
    }
    action
}

fn insert(bank: &mut [Slot; 12], boundary: usize, slot: Slot) {
    if let Some(empty) = bank.iter().rposition(|s| s.parameter.is_none()) {
        let mut slots = bank.to_vec();
        slots.remove(empty);
        slots.insert(
            boundary
                .saturating_sub(usize::from(empty < boundary))
                .min(11),
            slot,
        );
        *bank = slots.try_into().expect("twelve macro slots");
    }
}
pub(super) fn apply_macro_action(
    ui: &egui::Ui,
    _pane: &str,
    kind: ModuleKind,
    state: &mut State,
    action: MacroAction,
    menu: &mut MacroMenuState,
) {
    let bank = &mut state.banks[kind as usize];
    let before = bank.clone();
    match action {
        MacroAction::Copy(i) | MacroAction::Cut(i) => {
            ui.data_mut(|d| {
                d.insert_temp(
                    egui::Id::new(CLIPBOARD),
                    Clipboard {
                        kind,
                        slot: bank[i].clone(),
                    },
                )
            });
            if matches!(action, MacroAction::Cut(_)) {
                bank[i] = Slot::default();
            }
        }
        MacroAction::Paste(i) => {
            if let Some(clip) = ui.data(|d| d.get_temp::<Clipboard>(egui::Id::new(CLIPBOARD)))
                && clip.kind == kind
            {
                bank[i] = clip.slot;
            }
        }
        MacroAction::Duplicate(i) => {
            let mut slot = bank[i].clone();
            slot.name = Some(
                format!("{} copy", slot_title(&slot, i))
                    .chars()
                    .take(80)
                    .collect(),
            );
            slot.enabled = false;
            insert(bank, i + 1, slot);
        }
        MacroAction::Delete(i) => bank[i] = Slot::default(),
        MacroAction::Rename(i) => {
            menu.rename = Some((i, slot_title(&bank[i], i), true));
            ui.data_mut(|d| d.insert_temp(egui::Id::new(INSPECT_REQUEST), i));
        }
        MacroAction::RenameTo(i, name) => bank[i].name = Some(name),
        MacroAction::Color(i, color) => {
            bank[i].color = color;
            bank[i].color_alpha = 255;
        }
        MacroAction::Insert(i) => insert(bank, i, Slot::default()),
        MacroAction::Undo => {
            if let Some(previous) = menu.undo.take() {
                *bank = previous;
            }
        }
        MacroAction::Move(from, boundary) => {
            let mut slots = bank.to_vec();
            let slot = slots.remove(from);
            slots.insert(
                boundary
                    .saturating_sub(usize::from(boundary > from))
                    .min(11),
                slot,
            );
            *bank = slots.try_into().expect("twelve macro slots");
        }
    }
    if *bank != before {
        menu.undo = Some(before);
    }
}

pub(super) fn macro_rename_field(
    ui: &mut egui::Ui,
    rect: Rect,
    index: usize,
    color: egui::Color32,
    menu: &mut MacroMenuState,
) -> Option<MacroAction> {
    let (target, name, focus) = menu.rename.as_mut().filter(|(i, _, _)| *i == index)?;
    let commit = crate::workspace_ui::inline_strip_name(ui, rect, name, focus, color)?;
    let action = commit.then(|| MacroAction::RenameTo(*target, name.trim().to_owned()));
    menu.rename = None;
    action
}

pub(super) fn delete_scene_slot(
    ctx: &egui::Context,
    scope: &str,
    slots: &mut [Slot; 12],
    index: usize,
) {
    if index >= slots.len() || slots[index] == Slot::default() {
        return;
    }
    let key = egui::Id::new(("macro-menu-ui", scope, ModuleKind::Waterfall as usize));
    let mut menu = ctx
        .data(|d| d.get_temp::<MacroMenuState>(key))
        .unwrap_or_default();
    menu.sync(slots);
    menu.undo = Some(slots.clone());
    menu.rename = None;
    slots[index] = Slot::default();
    menu.remember(slots);
    ctx.data_mut(|d| d.insert_temp(key, menu));
}

#[cfg(test)]
mod tests {
    use super::*;
    fn parameter(field: &str) -> ParameterSpec {
        ParameterSpec {
            field: field.into(),
            label: field.into(),
            min: 0.0,
            max: 2.0,
            safe_min: 0.0,
            safe_max: 2.0,
            default: 1.0,
            integer: false,
            logarithmic: false,
            scale: 1.0,
        }
    }
    #[test]
    fn editing_moves_complete_slots_and_roundtrips_without_affecting_parameters() {
        let kind = ModuleKind::Flux;
        let mut state = State::default();
        let mut settings = crate::modules::ModulePane::new(kind).copy_settings();
        let original = settings.clone();
        state.assign(kind, 0, parameter("brightness"));
        state.assign(kind, 1, parameter("speed"));
        let slot = &mut state.banks[kind as usize][0];
        slot.signal = Some(2);
        slot.depth = -0.4;
        slot.name = Some("Drop lights".into());
        slot.color = Some([255, 148, 166]);
        let source = slot.clone();
        let ctx = egui::Context::default();
        let mut menu = MacroMenuState::default();
        let mut out = ctx.run_ui(Default::default(), |ui| {
            for action in [MacroAction::Copy(0), MacroAction::Move(0, 3)] {
                apply_macro_action(ui, "test", kind, &mut state, action, &mut menu);
            }
            assert_eq!(state.banks[kind as usize][2], source);
            apply_macro_action(
                ui,
                "test",
                kind,
                &mut state,
                MacroAction::Paste(4),
                &mut menu,
            );
            assert_eq!(state.banks[kind as usize][4], source);
            apply_macro_action(
                ui,
                "test",
                kind,
                &mut state,
                MacroAction::Duplicate(2),
                &mut menu,
            );
            assert!(!state.banks[kind as usize][3].enabled);
            assert_eq!(state.banks[kind as usize][3].parameter, source.parameter);
            assert_eq!(
                state.banks[kind as usize][3].name.as_deref(),
                Some("Drop lights copy")
            );
            let before = state.banks[kind as usize].clone();
            apply_macro_action(ui, "test", kind, &mut state, MacroAction::Cut(2), &mut menu);
            assert_eq!(state.banks[kind as usize][2], Slot::default());
            apply_macro_action(ui, "test", kind, &mut state, MacroAction::Undo, &mut menu);
            assert_eq!(state.banks[kind as usize], before);
        });
        out.textures_delta.clear();
        assert!(state.valid(&[settings.clone()]));
        let restored: State = serde_json::from_slice(&serde_json::to_vec(&state).unwrap()).unwrap();
        assert_eq!(restored, state);
        let mut snapshot = signals::Snapshot::default();
        snapshot.values.insert(
            2,
            signals::Sample {
                value: 0.2,
                availability: 1.0,
                ..Default::default()
            },
        );
        let active = state.banks[kind as usize]
            .iter()
            .filter(|s| s.enabled && s.signal == Some(2))
            .count();
        let base = get(&settings, "brightness").unwrap();
        let values = state.effective(kind, &settings, Some(&snapshot));
        assert_eq!(values.len(), 1);
        assert!(
            (values[0].1 - (base - active as f64 * 0.4 * 0.2 * 2.0).clamp(0.0, 2.0)).abs() < 1e-6
        );
        let restore = state.apply(kind, &mut settings, Some(&snapshot));
        assert_eq!(restore.len(), 1);
        set_many(&mut settings, &restore);
        assert_eq!(settings, original);
        let mut legacy = serde_json::to_value(&source).unwrap();
        legacy.as_object_mut().unwrap().remove("name");
        legacy.as_object_mut().unwrap().remove("color");
        let legacy: Slot = serde_json::from_value(legacy).unwrap();
        assert!(legacy.name.is_none() && legacy.color.is_none());
    }
}
