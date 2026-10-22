//! Direct controller mappings are scene data, independent from signal channels.
use super::{Binding, Message, Snapshot};
use crate::{
    modules::ModulePane,
    parameter::schema::{ParameterContext, ParameterSpec},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

const COMMAND: &str = "midi-parameter-command";
const MAPPED: &str = "midi-parameter-bindings";
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Mapping {
    pub target: ParameterContext,
    pub parameter: ParameterSpec,
    pub input: Binding,
}
#[derive(Clone)]
enum Command {
    Learn(ParameterContext, ParameterSpec),
    Remove(ParameterContext, String),
}
#[derive(Default)]
pub struct Controller {
    learning: Option<(ParameterContext, ParameterSpec, u64)>,
    serial: u64,
    initialized: bool,
    published: Arc<Vec<Mapping>>,
}
pub fn mapped(ctx: &egui::Context, target: &ParameterContext, field: &str) -> bool {
    ctx.data(|d| d.get_temp::<Arc<Vec<Mapping>>>(egui::Id::new(MAPPED)))
        .is_some_and(|mappings| {
            mappings
                .iter()
                .any(|m| m.target == *target && m.parameter.field == field)
        })
}
pub fn remove(ctx: &egui::Context, mapping: &Mapping) {
    ctx.data_mut(|d| {
        d.insert_temp(
            egui::Id::new(COMMAND),
            Command::Remove(mapping.target.clone(), mapping.parameter.field.clone()),
        )
    });
}
pub fn menu(ui: &mut egui::Ui, target: &ParameterContext, parameter: &ParameterSpec) {
    let existing = ui
        .data(|d| d.get_temp::<Arc<Vec<Mapping>>>(egui::Id::new(MAPPED)))
        .unwrap_or_default();
    let mapped = existing
        .iter()
        .find(|m| m.target == *target && m.parameter.field == parameter.field);
    if ui
        .button(if mapped.is_some() {
            "Relearn MIDI controller…"
        } else {
            "Learn MIDI controller…"
        })
        .clicked()
    {
        ui.data_mut(|d| {
            d.insert_temp(
                egui::Id::new(COMMAND),
                Command::Learn(target.clone(), parameter.clone()),
            )
        });
        ui.close();
    }
    if let Some(mapping) = mapped {
        ui.weak(format!(
            "{} · Ch {} · {}",
            mapping.input.message.label(),
            mapping.input.channel.unwrap_or(1),
            mapping
                .input
                .number
                .map_or(String::new(), |n| n.to_string())
        ));
        if ui.button("Remove MIDI mapping").clicked() {
            remove(ui.ctx(), mapping);
            ui.close();
        }
    }
}
impl Controller {
    pub fn pending_change(&self, ctx: &egui::Context, snapshot: &Snapshot) -> bool {
        ctx.data(|d| d.get_temp::<Command>(egui::Id::new(COMMAND)))
            .is_some()
            || self.learning.as_ref().is_some_and(|(_, _, after)| {
                snapshot
                    .events
                    .iter()
                    .any(|e| e.serial > *after && snapshot.enabled.contains(&e.port))
            })
    }

    pub fn reset(&mut self, snapshot: &Snapshot) {
        self.serial = snapshot.serial;
        self.initialized = true;
        self.learning = None;
    }
    /// Returns true only for a learned/removed binding, not performance motion.
    pub fn update(
        &mut self,
        ctx: &egui::Context,
        snapshot: &Snapshot,
        mappings: &mut Vec<Mapping>,
        panes: &mut [ModulePane],
    ) -> bool {
        if !self.initialized || snapshot.serial < self.serial {
            self.reset(snapshot);
        }
        let mut changed = false;
        if let Some(command) = ctx.data_mut(|d| {
            let id = egui::Id::new(COMMAND);
            let command = d.get_temp::<Command>(id);
            d.remove::<Command>(id);
            command
        }) {
            match command {
                Command::Learn(target, spec) => {
                    self.learning = Some((target, spec, snapshot.serial))
                }
                Command::Remove(target, field) => {
                    let before = mappings.len();
                    mappings.retain(|m| m.target != target || m.parameter.field != field);
                    changed = before != mappings.len();
                }
            }
        }
        if let Some((target, spec, after)) = &self.learning
            && let Some(event) = snapshot
                .events
                .iter()
                .find(|e| e.serial > *after && snapshot.enabled.contains(&e.port))
        {
            let target = target.clone();
            let parameter = spec.clone();
            mappings.retain(|m| m.target != target || m.parameter.field != parameter.field);
            if mappings.len() < 256 {
                mappings.push(Mapping {
                    target,
                    parameter,
                    input: Binding {
                        port: Some(event.port),
                        channel: Some(event.channel),
                        message: event.message,
                        number: event.number,
                    },
                });
                changed = true;
            }
            self.learning = None;
        }
        for pane in panes {
            let changes: Vec<_> = mappings
                .iter()
                .filter(|m| {
                    m.target.pane == pane.instance_id()
                        && m.target.kind == pane.kind
                        && snapshot.available(&m.input)
                })
                .filter(|m| snapshot.changed_since(&m.input, self.serial))
                .map(|m| {
                    let raw = snapshot.value(&m.input);
                    let normalized = if m.input.message == Message::PitchBend {
                        (raw + 1.0) * 0.5
                    } else {
                        raw
                    };
                    (
                        m.parameter.field.clone(),
                        m.parameter.value(normalized.clamp(0.0, 1.0)) * m.parameter.scale,
                    )
                })
                .collect();
            if changes.is_empty() {
                continue;
            }
            let mut settings = pane.copy_settings();
            if crate::macros::set_many(&mut settings, &changes) {
                pane.paste_settings(&settings);
            }
        }
        self.serial = snapshot.serial;
        if self.published.as_ref() != mappings {
            self.published = Arc::new(mappings.clone());
        }
        ctx.data_mut(|d| d.insert_temp(egui::Id::new(MAPPED), self.published.clone()));
        if let Some((_, spec, _)) = &self.learning {
            let mut cancel = false;
            egui::Window::new("Learn MIDI controller").collapsible(false).resizable(false).show(ctx,|ui|{
                ui.strong(&spec.label);ui.label("Move a knob, fader, or pad on an enabled MIDI input.");
                ui.weak("Absolute controllers only. The next message creates a scene mapping; no signal channel is created.");
                cancel=ui.button("Cancel").clicked() || ui.input(|i|i.key_pressed(egui::Key::Escape));
            });
            if cancel {
                self.learning = None;
            }
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::ModuleKind;
    fn setup() -> (super::super::Engine, Vec<ModulePane>, Mapping) {
        let mut engine = super::super::Engine::default();
        engine.snapshot.enabled.insert(7);
        engine.snapshot.ports.push(super::super::Port {
            id: 7,
            name: "Test controller".into(),
        });
        let panes = vec![ModulePane::new(ModuleKind::Waterfall)];
        let mapping = Mapping {
            target: ParameterContext {
                pane: panes[0].instance_id().into(),
                kind: ModuleKind::Waterfall,
            },
            parameter: ParameterSpec {
                field: "height".into(),
                label: "Height".into(),
                min: 0.0,
                max: 2.0,
                safe_min: 0.0,
                safe_max: 2.0,
                default: 0.85,
                integer: false,
                logarithmic: false,
                scale: 1.0,
            },
            input: Binding {
                port: Some(7),
                channel: Some(1),
                message: Message::ControlChange,
                number: Some(1),
            },
        };
        (engine, panes, mapping)
    }
    #[test]
    fn learns_new_message_and_ignores_old_or_unrelated_controls() {
        let (mut engine, mut panes, mapping) = setup();
        let ctx = egui::Context::default();
        let mut controller = Controller::default();
        let mut mappings = vec![];
        engine.bytes(7, &[0xb0, 1, 127]);
        ctx.data_mut(|d| {
            d.insert_temp(
                egui::Id::new(COMMAND),
                Command::Learn(mapping.target.clone(), mapping.parameter.clone()),
            )
        });
        let mut output = ctx.run_ui(Default::default(), |_| {
            controller.update(&ctx, &engine.snapshot, &mut mappings, &mut panes);
        });
        output.textures_delta.clear();
        assert!(mappings.is_empty());
        engine.bytes(7, &[0xb0, 1, 64]);
        let mut output = ctx.run_ui(Default::default(), |_| {
            assert!(controller.update(&ctx, &engine.snapshot, &mut mappings, &mut panes));
        });
        output.textures_delta.clear();
        assert_eq!(mappings.len(), 1);
        let value = crate::macros::get(&panes[0].copy_settings(), "height").unwrap();
        assert!((value - 128.0 / 127.0).abs() < 1e-6);
        engine.bytes(7, &[0xb0, 2, 127]);
        controller.update(&ctx, &engine.snapshot, &mut mappings, &mut panes);
        assert_eq!(
            crate::macros::get(&panes[0].copy_settings(), "height").unwrap(),
            value
        );
    }
    #[test]
    fn note_release_returns_parameter_to_minimum_and_bend_maps_center() {
        let (mut engine, mut panes, mut mapping) = setup();
        let ctx = egui::Context::default();
        let mut controller = Controller::default();
        controller.reset(&engine.snapshot);
        mapping.input.message = Message::NoteVelocity;
        mapping.input.number = Some(60);
        let mut mappings = vec![mapping];
        for (bytes, expected) in [([0x90, 60, 127], 2.0), ([0x80, 60, 0], 0.0)] {
            engine.bytes(7, &bytes);
            controller.update(&ctx, &engine.snapshot, &mut mappings, &mut panes);
            assert_eq!(
                crate::macros::get(&panes[0].copy_settings(), "height"),
                Some(expected)
            );
        }
        mappings[0].input.message = Message::PitchBend;
        mappings[0].input.number = None;
        engine.bytes(7, &[0xe0, 0, 64]);
        controller.update(&ctx, &engine.snapshot, &mut mappings, &mut panes);
        assert_eq!(
            crate::macros::get(&panes[0].copy_settings(), "height"),
            Some(1.0)
        );
        let encoded = serde_json::to_string(&mappings).unwrap();
        assert_eq!(
            serde_json::from_str::<Vec<Mapping>>(&encoded).unwrap(),
            mappings
        );
    }
}
