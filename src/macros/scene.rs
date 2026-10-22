//! One performance bank for the entire scene. Targets use stable component IDs.
use super::*;
use crate::modules::{ModulePane, PaneState};

pub enum InspectAction {
    None,
    Close,
    Signal(u64),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SceneMacros {
    pub open: bool,
    pub slots: [Slot; 12],
    pub midi: Vec<crate::midi::mapping::Mapping>,
}
impl Default for SceneMacros {
    fn default() -> Self {
        Self {
            open: true,
            slots: std::array::from_fn(|_| Slot::default()),
            midi: Vec::new(),
        }
    }
}
impl SceneMacros {
    pub fn from_legacy(panes: &[PaneState], selected: usize) -> Self {
        // Normalize older named artistic routes before importing their macro bank.
        let normalized: Vec<_> = panes
            .iter()
            .map(|saved| {
                let mut pane = ModulePane::new(saved.kind);
                pane.restore(saved);
                pane.import_legacy_macros();
                pane.snapshot()
            })
            .collect();
        let panes = normalized.as_slice();
        let mut result = Self::default();
        if let Some(pane) = panes.get(selected)
            && let Some(macros) = &pane.macros
        {
            result.open = macros.open;
            result.slots = macros.banks[pane.kind as usize].clone();
            for slot in &mut result.slots {
                if slot.parameter.is_some() {
                    slot.target = Some(ParameterContext {
                        pane: pane.instance_id.clone(),
                        kind: pane.kind,
                    });
                }
            }
        }
        let mut append = |pane: &PaneState, kind: ModuleKind| {
            if let Some(macros) = &pane.macros {
                for slot in &macros.banks[kind as usize] {
                    if slot.parameter.is_none() {
                        continue;
                    }
                    let Some(index) = result.slots.iter().position(|s| s.parameter.is_none())
                    else {
                        return;
                    };
                    let mut slot = slot.clone();
                    slot.target = Some(ParameterContext {
                        pane: pane.instance_id.clone(),
                        kind,
                    });
                    result.slots[index] = slot;
                }
            }
        };
        for (index, pane) in panes.iter().enumerate() {
            if index != selected {
                append(pane, pane.kind);
            }
        }
        for pane in panes {
            for kind in ModuleKind::ALL {
                if kind != pane.kind {
                    append(pane, kind);
                }
            }
        }
        result
    }
    pub fn valid(&self, panes: &[PaneState]) -> bool {
        let mut targets = std::collections::HashSet::new();
        self.midi.len() <= 256
            && self.midi.iter().all(|m| {
                targets.insert((&m.target.pane, m.target.kind as usize, &m.parameter.field))
                    && m.input.valid()
                    && m.parameter.valid()
                    && panes.iter().any(|p| {
                        p.instance_id == m.target.pane
                            && p.settings.iter().any(|s| {
                                s.kind() == m.target.kind
                                    && crate::macros::get(s, &m.parameter.field).is_some()
                            })
                    })
            })
            && self.slots.iter().all(|slot| {
                if slot.parameter.is_some() != slot.target.is_some() {
                    return false;
                }
                let Some(target) = &slot.target else {
                    let mut state = State::default();
                    state.banks[0][0] = slot.clone();
                    return state.valid(&[]);
                };
                let Some(pane) = panes.iter().find(|p| p.instance_id == target.pane) else {
                    return false;
                };
                let mut state = State::default();
                state.banks[target.kind as usize][0] = slot.clone();
                state.valid(&pane.settings)
            })
    }
    pub fn reconcile(&mut self, panes: &[ModulePane], bank: &signals::Bank) {
        self.midi
            .retain(|m| panes.iter().any(|p| p.instance_id() == m.target.pane));
        for slot in &mut self.slots {
            if slot
                .signal
                .is_some_and(|id| bank.channels.iter().all(|s| s.id != id))
            {
                slot.signal = None;
            }
            if slot
                .target
                .as_ref()
                .is_some_and(|target| panes.iter().all(|p| p.instance_id() != target.pane))
            {
                slot.parameter = None;
                slot.target = None;
            }
        }
    }
    pub fn for_panes(&self, panes: &[PaneState]) -> Self {
        let mut macros = self.clone();
        macros
            .midi
            .retain(|m| panes.iter().any(|p| p.instance_id == m.target.pane));
        for slot in &mut macros.slots {
            if slot
                .target
                .as_ref()
                .is_some_and(|target| panes.iter().all(|p| p.instance_id != target.pane))
            {
                slot.target = None;
                slot.parameter = None;
            }
        }
        macros
    }
    pub fn remove_component(&mut self, id: &str) {
        self.midi.retain(|m| m.target.pane != id);
        for slot in &mut self.slots {
            if slot.target.as_ref().is_some_and(|target| target.pane == id) {
                slot.parameter = None;
                slot.target = None;
            }
        }
    }
    pub fn height(&self) -> Option<f32> {
        self.open.then_some(236.0)
    }
    pub fn bar(
        &mut self,
        ui: &mut egui::Ui,
        rect: Rect,
        theme: &AppTheme,
        scope: &str,
        panes: &mut [ModulePane],
    ) {
        let mut components: Vec<Component> = panes
            .iter()
            .enumerate()
            .map(|(i, p)| {
                Component::new(
                    i,
                    ParameterContext {
                        pane: p.instance_id().to_owned(),
                        kind: p.kind,
                    },
                    p.copy_settings(),
                    format!("{} · {}", i + 1, p.kind.label()),
                )
            })
            .collect();
        for slot in &self.slots {
            if let Some(target) = &slot.target
                && !components.iter().any(|c| &c.target == target)
                && let Some((index, pane)) = panes
                    .iter()
                    .enumerate()
                    .find(|(_, p)| p.instance_id() == target.pane)
                && let Some(settings) = pane
                    .snapshot()
                    .settings
                    .into_iter()
                    .find(|s| s.kind() == target.kind)
            {
                components.push(Component::new(
                    index,
                    target.clone(),
                    settings,
                    format!("{} · {} · inactive", target.kind.label(), index + 1),
                ));
            }
        }
        if components.is_empty() {
            return;
        }
        let mut adapter = State::default();
        adapter.banks[0] = std::mem::take(&mut self.slots);
        let mut placeholder = components[0].settings.clone();
        super::bar_impl(
            ui,
            rect,
            theme,
            scope,
            ModuleKind::Waterfall,
            &mut adapter,
            &mut placeholder,
            Some(&mut components),
        );
        self.slots = std::mem::take(&mut adapter.banks[0]);
        for component in components {
            if component.settings != component.original {
                panes[component.index].apply_macro_settings(&component.settings);
            }
        }
    }
}
pub(super) struct Component {
    index: usize,
    pub target: ParameterContext,
    pub settings: ModuleSettings,
    original: ModuleSettings,
    pub document: serde_json::Value,
    pub effective: Vec<(String, f64)>,
    pub label: String,
}
impl Component {
    fn new(
        index: usize,
        target: ParameterContext,
        settings: ModuleSettings,
        label: String,
    ) -> Self {
        Self {
            document: serde_json::to_value(&settings).unwrap_or_default(),
            original: settings.clone(),
            settings,
            target,
            index,
            effective: Vec::new(),
            label,
        }
    }
}
pub(super) fn state_for(slots: &[Slot; 12], target: &ParameterContext) -> State {
    let mut state = State::default();
    for (index, slot) in slots.iter().enumerate() {
        if slot.target.as_ref() == Some(target) {
            state.banks[target.kind as usize][index] = slot.clone();
        }
    }
    state
}

#[cfg(test)]
mod tests {
    use super::*;
    fn parameter() -> ParameterSpec {
        ParameterSpec {
            field: "height".into(),
            label: "Height".into(),
            min: 0.0,
            max: 10.0,
            safe_min: 0.0,
            safe_max: 10.0,
            default: 1.0,
            integer: false,
            logarithmic: false,
            scale: 1.0,
        }
    }
    #[test]
    fn legacy_banks_merge_and_targets_isolate_identical_component_parameters() {
        let mut a = ModulePane::new(ModuleKind::Waterfall);
        let mut b = ModulePane::new(ModuleKind::Waterfall);
        a.macros.assign(ModuleKind::Waterfall, 0, parameter());
        b.macros.assign(ModuleKind::Waterfall, 0, parameter());
        a.macros.banks[0][0].signal = Some(1);
        b.macros.banks[0][0].signal = Some(1);
        a.macros.banks[0][0].depth = 0.1;
        b.macros.banks[0][0].depth = -0.2;
        let panes = vec![a.snapshot(), b.snapshot()];
        let mut macros = SceneMacros::from_legacy(&panes, 0);
        assert_eq!(
            macros.slots[0].target.as_ref().unwrap().pane,
            a.instance_id()
        );
        assert_eq!(
            macros.slots[1].target.as_ref().unwrap().pane,
            b.instance_id()
        );
        assert!(macros.valid(&panes));
        let mut snapshot = signals::Snapshot::default();
        snapshot.values.insert(
            1,
            signals::Sample {
                value: 0.5,
                availability: 1.0,
                ..Default::default()
            },
        );
        for (index, expected) in [(0, 0.5), (1, -1.0)] {
            let target = macros.slots[index].target.as_ref().unwrap();
            let settings = if index == 0 {
                a.copy_settings()
            } else {
                b.copy_settings()
            };
            let base = get(&settings, "height").unwrap();
            let values =
                state_for(&macros.slots, target).effective(target.kind, &settings, Some(&snapshot));
            assert_eq!(values.len(), 1);
            assert!((values[0].1 - (base + expected).clamp(0.0, 10.0)).abs() < 1e-5);
        }
        let loaded: SceneMacros =
            serde_json::from_slice(&serde_json::to_vec(&macros).unwrap()).unwrap();
        assert_eq!(loaded, macros);
        macros.remove_component(a.instance_id());
        assert!(macros.slots[0].parameter.is_none());
        assert!(macros.slots[1].parameter.is_some());
        let mut bank = signals::Bank::default();
        bank.add(signals::Method::Band);
        macros.reconcile(&[a, b], &bank);
        assert_eq!(macros.slots[1].signal, Some(1));
        bank.remove_channel(1);
        let mut a = ModulePane::new(ModuleKind::Waterfall);
        a.restore(&panes[1]);
        macros.reconcile(&[a], &bank);
        assert!(macros.slots[1].signal.is_none());
        assert!(macros.slots[1].parameter.is_some());
        macros.slots[1].target.as_mut().unwrap().pane = "missing".into();
        assert!(!macros.valid(&panes));
    }
}
