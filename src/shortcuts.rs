use eframe::egui::{self, Key, Modifiers};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Pause,
    PauseAll,
    Sidebar,
    Expand,
    Fullscreen,
    Restore,
    Help,
    Bindings,
    Pane(usize),
}
struct Binding {
    action: Action,
    key: Key,
    modifiers: Modifiers,
    label: &'static str,
    description: &'static str,
}
const BINDINGS: &[Binding] = &[
    Binding {
        action: Action::PauseAll,
        key: Key::Space,
        modifiers: Modifiers::SHIFT,
        label: "Shift + Space",
        description: "Pause all panes; resume when all are paused",
    },
    Binding {
        action: Action::Pause,
        key: Key::Space,
        modifiers: Modifiers::NONE,
        label: "Space",
        description: "Pause / resume selected pane",
    },
    Binding {
        action: Action::Sidebar,
        key: Key::N,
        modifiers: Modifiers::NONE,
        label: "N",
        description: "Show / hide module settings",
    },
    Binding {
        action: Action::Expand,
        key: Key::F,
        modifiers: Modifiers::NONE,
        label: "F",
        description: "Expand / restore selected pane",
    },
    Binding {
        action: Action::Fullscreen,
        key: Key::F11,
        modifiers: Modifiers::NONE,
        label: "F11",
        description: "Toggle application fullscreen",
    },
    Binding {
        action: Action::Restore,
        key: Key::Escape,
        modifiers: Modifiers::NONE,
        label: "Escape",
        description: "Restore dashboard (or close the active modal/menu)",
    },
    Binding {
        action: Action::Help,
        key: Key::F1,
        modifiers: Modifiers::NONE,
        label: "F1",
        description: "Show / hide Info View",
    },
    Binding {
        action: Action::Bindings,
        key: Key::K,
        modifiers: Modifiers::COMMAND,
        label: "Ctrl + K",
        description: "Open keyboard shortcuts",
    },
    Binding {
        action: Action::Pane(0),
        key: Key::Num1,
        modifiers: Modifiers::NONE,
        label: "1",
        description: "Select pane 1",
    },
    Binding {
        action: Action::Pane(1),
        key: Key::Num2,
        modifiers: Modifiers::NONE,
        label: "2",
        description: "Select pane 2",
    },
    Binding {
        action: Action::Pane(2),
        key: Key::Num3,
        modifiers: Modifiers::NONE,
        label: "3",
        description: "Select pane 3",
    },
    Binding {
        action: Action::Pane(3),
        key: Key::Num4,
        modifiers: Modifiers::NONE,
        label: "4",
        description: "Select pane 4",
    },
];

fn editing_text(ctx: &egui::Context) -> bool {
    ctx.memory(|m| m.focused())
        .is_some_and(|id| egui::text_edit::TextEditState::load(ctx, id).is_some())
}

pub fn take_action(ctx: &egui::Context, modal_open: bool) -> Option<Action> {
    if modal_open || editing_text(ctx) || egui::Popup::is_any_open(ctx) {
        return None;
    }
    ctx.input_mut(|input| {
        if !input.focused { return None; }
        let binding = BINDINGS.iter().find(|binding| input.events.iter().any(|event| matches!(event,
            egui::Event::Key { key, pressed: true, repeat: false, modifiers, .. }
            if *key == binding.key && modifiers.shift == binding.modifiers.shift
                && modifiers.alt == binding.modifiers.alt && modifiers.matches_logically(binding.modifiers))));
        if let Some(binding) = binding {
            input.consume_key(binding.modifiers, binding.key);
            Some(binding.action)
        } else { None }
    })
}

pub fn show(ctx: &egui::Context, open: &mut bool) {
    if !*open {
        return;
    }
    let modal = egui::Modal::new(egui::Id::new("keybindings")).show(ctx, |ui| {
        ui.set_width(490.0_f32.min(ctx.content_rect().width() - 48.0));
        ui.heading("Keyboard shortcuts");
        ui.label("Application shortcuts are disabled while typing or using a modal/menu. Bindings are currently fixed.");
        egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
            egui::Grid::new("bindings-list").striped(true).spacing([18.0, 8.0]).show(ui, |ui| {
                for binding in BINDINGS { ui.monospace(binding.label); ui.label(binding.description); ui.end_row(); }
            });
            ui.separator();
            ui.strong("Parameter editing");
            ui.label("Click number: exact entry · Enter: apply · Escape: cancel\nShift-drag slider: fine adjustment · Double-click slider: reset");
            ui.separator();
            ui.strong("Waterfall camera");
            ui.label("Left-drag: orbit · Right/middle-drag or Shift-left-drag: pan\nScroll: zoom · Double-click viewport: reset camera\nClick axis: align / flip · Drag axis navigator: orbit");
        });
        if ui.button("Close").clicked() { *open = false; }
    });
    if modal.should_close() {
        *open = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn event(key: Key, modifiers: Modifiers, repeat: bool) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat,
            modifiers,
        }
    }
    #[test]
    fn shortcuts_distinguish_modifiers_ignore_repeats_and_respect_modals() {
        for (events, modal, expected) in [
            (
                vec![event(Key::Space, Modifiers::NONE, false)],
                false,
                Some(Action::Pause),
            ),
            (
                vec![event(Key::Space, Modifiers::SHIFT, false)],
                false,
                Some(Action::PauseAll),
            ),
            (vec![event(Key::Space, Modifiers::NONE, true)], false, None),
            (vec![event(Key::F11, Modifiers::NONE, false)], true, None),
        ] {
            let ctx = egui::Context::default();
            if events
                .iter()
                .any(|event| matches!(event, egui::Event::Key { repeat: true, .. }))
            {
                let mut warmup = ctx.run_ui(
                    egui::RawInput {
                        events: vec![event(Key::Space, Modifiers::NONE, false)],
                        ..Default::default()
                    },
                    |_| {},
                );
                warmup.textures_delta.clear();
            }
            let mut output = ctx.run_ui(
                egui::RawInput {
                    events,
                    ..Default::default()
                },
                |ui| assert_eq!(take_action(ui.ctx(), modal), expected),
            );
            output.textures_delta.clear();
        }
    }
    #[test]
    fn shortcuts_do_not_fire_in_text_fields() {
        let ctx = egui::Context::default();
        let mut text = String::new();
        let mut output = ctx.run_ui(Default::default(), |ui| {
            ui.text_edit_singleline(&mut text).request_focus();
        });
        output.textures_delta.clear();
        let mut output = ctx.run_ui(
            egui::RawInput {
                events: vec![event(Key::Space, Modifiers::NONE, false)],
                ..Default::default()
            },
            |ui| {
                assert_eq!(take_action(ui.ctx(), false), None);
                ui.text_edit_singleline(&mut text);
            },
        );
        output.textures_delta.clear();
    }
}
