//! Portable creative documents. Device bindings, GUI preferences and runtime data stay local.
use crate::{
    analysis::{AnalysisSettings, ChannelMode},
    modules::{ModuleSettings, PaneState},
    signals::{Bank, Definition},
    workspace::{PresetContent, Session, Tile},
};
use serde::{Deserialize, Serialize};
use std::path::Path;
#[derive(Clone, Serialize, Deserialize)]
pub struct Project {
    pub format: String,
    pub version: u32,
    pub signals: Bank,
    pub panes: Vec<PaneState>,
    pub layout: Option<Tile>,
    pub selected_pane: usize,
    pub analysis: AnalysisSettings,
}
impl Project {
    pub fn capture(session: Session) -> Self {
        Self {
            format: "symphos-project".into(),
            version: 1,
            signals: session.signals,
            panes: session.panes,
            layout: session.layout,
            selected_pane: session.selected_pane,
            analysis: session.analysis,
        }
    }
    pub fn validate(&self) -> bool {
        let session = Session {
            version: 2,
            signals: self.signals.clone(),
            panes: self.panes.clone(),
            layout: self.layout.clone(),
            selected_pane: self.selected_pane,
            appearance: Default::default(),
            demo: false,
            focused_pane: None,
            sidebar_open: true,
            sidebar_width: 300.0,
            help_open: false,
            analysis: self.analysis.clone(),
            meter_stereo: true,
            meter_channel: ChannelMode::StereoMix,
            window_size: None,
            window_position: None,
            active_preset: "custom".into(),
        };
        self.format == "symphos-project" && self.version == 1 && session.validate()
    }
    pub fn save(&self, path: &Path) -> Result<(), String> {
        if !self.validate() {
            return Err("Project contains invalid settings. No file was changed.".into());
        }
        let data = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        crate::workspace::atomic_write(path, &data).map_err(|e| e.to_string())
    }
    pub fn open(path: &Path) -> Result<Self, String> {
        let size = std::fs::metadata(path).map_err(|e| e.to_string())?.len();
        if size > 8_000_000 {
            return Err("Project exceeds 8 MB limit.".into());
        }
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        let project: Self = serde_json::from_slice(&bytes)
            .map_err(|e| format!("Could not read project: {e}. Original file retained."))?;
        if !project.validate() {
            return Err("Invalid or unsupported project. Original file retained; current workspace unchanged.".into());
        }
        Ok(project)
    }
}
#[derive(Default)]
pub struct Dialog {
    pub current_path: Option<std::path::PathBuf>,
    pub open: bool,
    pub path: String,
    pub message: String,
    pub confirm_open: bool,
    pub confirm_save: bool,
}

pub fn visit(content: &mut PresetContent, mut f: impl FnMut(&mut ModuleSettings)) {
    match content {
        PresetContent::Component(s) => f(s),
        PresetContent::View { panes, .. } => {
            for p in panes {
                for s in &mut p.settings {
                    f(s);
                }
            }
        }
        _ => {}
    }
}
pub fn dependencies(content: &PresetContent, bank: &Bank) -> Vec<Definition> {
    let mut ids = std::collections::HashSet::new();
    visit(&mut content.clone(), |s| {
        if let Some(m) = s.modulation_mut() {
            for r in &m.routes {
                if let Some(id) = r.signal {
                    ids.insert(id);
                }
            }
        }
    });
    if let PresetContent::View { panes, .. } = content {
        for pane in panes {
            if let Some(macros) = &pane.macros {
                for slot in macros.banks.iter().flatten() {
                    if let Some(id) = slot.signal {
                        ids.insert(id);
                    }
                }
            }
        }
    }
    bank.channels
        .iter()
        .filter(|d| ids.contains(&d.id))
        .cloned()
        .collect()
}
/// Component/view presets can reuse existing sources but never create them.
/// Saved dependency descriptions prevent an unrelated channel with the same ID
/// in another project from being silently substituted.
pub fn bind_preset_sources(content: &mut PresetContent, definitions: &[Definition], bank: &Bank) {
    fn same_source(a: &Definition, b: &Definition) -> bool {
        let mut a = a.clone();
        let mut b = b.clone();
        a.color = Some(a.color_rgb());
        b.color = Some(b.color_rgb());
        a.id = 0;
        b.id = 0;
        a == b
    }
    let resolve = |id| {
        definitions.iter().find(|d| d.id == id).and_then(|saved| {
            bank.channels
                .iter()
                .find(|d| d.id == id && same_source(d, saved))
                .or_else(|| bank.channels.iter().find(|d| same_source(d, saved)))
                .map(|d| d.id)
        })
    };
    if let PresetContent::View { panes, .. } = content {
        for pane in panes {
            if let Some(macros) = &mut pane.macros {
                for slot in macros.banks.iter_mut().flatten() {
                    if let Some(id) = slot.signal {
                        slot.signal = resolve(id);
                    }
                }
            }
        }
    }
    visit(content, |s| {
        if let Some(m) = s.modulation_mut() {
            for r in &mut m.routes {
                if let Some(id) = r.signal {
                    r.signal = definitions.iter().find(|d| d.id == id).and_then(|saved| {
                        bank.channels
                            .iter()
                            .find(|d| d.id == id && same_source(d, saved))
                            .or_else(|| bank.channels.iter().find(|d| same_source(d, saved)))
                            .map(|d| d.id)
                    });
                    if r.signal.is_none() {
                        r.enabled = false;
                    }
                }
            }
        }
    });
}

// Compare creative preset content by dependency value, not freshly imported IDs.
pub fn matches(session: &Session, preset: &crate::workspace::Preset) -> bool {
    if preset.signals.is_empty() {
        return session.matches(&preset.content);
    }
    let current = match &preset.content {
        PresetContent::Layout(_) => return session.matches(&preset.content),
        PresetContent::Component(s) => {
            let Some(p) = session.panes.get(session.selected_pane) else {
                return false;
            };
            if p.kind != s.kind() {
                return false;
            }
            let Some(s) = p.settings.iter().find(|s| s.kind() == p.kind) else {
                return false;
            };
            PresetContent::Component(Box::new(s.clone()))
        }
        PresetContent::View { .. } => {
            let Some(layout) = &session.layout else {
                return false;
            };
            PresetContent::View {
                layout: layout.canonical(),
                panes: layout
                    .ids()
                    .iter()
                    .map(|id| session.panes[*id].clone())
                    .collect(),
            }
        }
    };
    fn resolve(value: &mut serde_json::Value, definitions: &[Definition]) {
        match value {
            serde_json::Value::Object(map) => {
                map.remove("instance_id");
                if let Some(id) = map.get("signal").and_then(|v| v.as_u64()) {
                    let resolved = definitions
                        .iter()
                        .find(|d| d.id == id)
                        .map(|d| {
                            let mut definition = d.clone();
                            definition.color = Some(d.color_rgb());
                            let mut value = serde_json::to_value(definition).unwrap_or_default();
                            value.as_object_mut().unwrap().remove("id");
                            value
                        })
                        .unwrap_or(serde_json::Value::String("missing".into()));
                    map.insert("signal".into(), resolved);
                }
                for (name, v) in map.iter_mut() {
                    if name != "signal" {
                        resolve(v, definitions);
                    }
                }
            }
            serde_json::Value::Array(a) => {
                for v in a {
                    resolve(v, definitions);
                }
            }
            _ => {}
        }
    }
    let mut a = serde_json::to_value(current).unwrap_or_default();
    let mut b = serde_json::to_value(&preset.content).unwrap_or_default();
    resolve(&mut a, &session.signals.channels);
    resolve(&mut b, &preset.signals);
    a == b
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        modulation::Target,
        modules::{ModuleKind, ModulePane},
    };
    fn example() -> Project {
        let mut pane = ModulePane::new(ModuleKind::Flux);
        let mut signals = Bank::default();
        let mut settings = pane.copy_settings();
        for route in &mut settings.modulation_mut().unwrap().routes {
            let id = signals.add(crate::signals::Method::Band).unwrap();
            signals.channels.last_mut().unwrap().name = format!("Manual source {id}");
            route.signal = Some(id);
        }
        pane.paste_settings(&settings);
        Project {
            format: "symphos-project".into(),
            version: 1,
            signals,
            panes: vec![pane.snapshot()],
            layout: Some(Tile::Pane(0)),
            selected_pane: 0,
            analysis: Default::default(),
        }
    }
    #[test]
    fn disk_save_open_preserves_creative_state_and_excludes_preferences_and_runtime() {
        let mut project = example();
        let macros = project.panes[0].macros.as_mut().unwrap();
        macros.open = false;
        macros.assign(
            ModuleKind::Flux,
            0,
            crate::macros::ParameterSpec {
                field: "brightness".into(),
                label: "Brightness".into(),
                min: 0.0,
                max: 2.0,
                safe_min: 0.0,
                safe_max: 2.0,
                default: 1.0,
                integer: false,
                logarithmic: false,
                scale: 1.0,
            },
        );
        macros.banks[ModuleKind::Flux as usize][0].signal = Some(1);
        macros.banks[ModuleKind::Flux as usize][0].depth = -0.4;
        let path = std::env::temp_dir().join(format!(
            "symphos-project-test-{}.symphos.json",
            std::process::id()
        ));
        project.save(&path).unwrap();
        let loaded = Project::open(&path).unwrap();
        assert_eq!(project.signals, loaded.signals);
        assert_eq!(project.panes, loaded.panes);
        assert_eq!(project.layout, loaded.layout);
        let text = std::fs::read_to_string(&path).unwrap();
        for excluded in [
            "appearance",
            "window_position",
            "selected_node",
            "waveform_left",
            "voices",
            "capture_epoch",
        ] {
            assert!(!text.contains(excluded));
        }
        std::fs::write(&path, "{\"version\":999}").unwrap();
        assert!(Project::open(&path).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{\"version\":999}");
        let _ = std::fs::remove_file(&path);
    }
    #[test]
    fn presets_reuse_existing_sources_without_creating_or_changing_channels() {
        let project = example();
        let settings = project.panes[0]
            .settings
            .iter()
            .find(|s| s.kind() == ModuleKind::Flux)
            .unwrap()
            .clone();
        let mut content = PresetContent::Component(Box::new(settings));
        let deps = dependencies(&content, &project.signals);
        assert_eq!(deps.len(), 7);
        let bank = project.signals.clone();
        let old = bank.clone();
        bind_preset_sources(&mut content, &deps, &bank);
        assert_eq!(bank, old);
        visit(&mut content, |s| {
            for r in &s.modulation_mut().unwrap().routes {
                assert!(old.channels.iter().any(|d| Some(d.id) == r.signal));
            }
        });
        assert_eq!(dependencies(&content, &bank).len(), 7);
    }
    #[test]
    fn missing_preset_sources_do_not_rebind_to_an_unrelated_same_id() {
        let project = example();
        let settings = project.panes[0]
            .settings
            .iter()
            .find(|s| s.kind() == ModuleKind::Flux)
            .unwrap()
            .clone();
        let mut content = PresetContent::Component(Box::new(settings));
        let bank = project.signals;
        bind_preset_sources(&mut content, &[], &bank);
        visit(&mut content, |s| {
            for r in &s.modulation_mut().unwrap().routes {
                assert!(r.signal.is_none());
                assert!(!r.enabled);
            }
        });
    }
    #[test]
    fn imported_preset_is_unmodified_until_a_shared_definition_changes() {
        let project = example();
        let settings = project.panes[0]
            .settings
            .iter()
            .find(|s| s.kind() == ModuleKind::Flux)
            .unwrap()
            .clone();
        let mut content = PresetContent::Component(Box::new(settings));
        let deps = dependencies(&content, &project.signals);
        let preset = crate::workspace::Preset {
            id: "user:test".into(),
            name: "Test".into(),
            content: content.clone(),
            signals: deps.clone(),
        };
        let bank = project.signals;
        bind_preset_sources(&mut content, &deps, &bank);
        let mut pane = ModulePane::new(ModuleKind::Flux);
        if let PresetContent::Component(s) = content {
            pane.paste_settings(&s);
        }
        let mut session = Session {
            version: 2,
            signals: bank,
            panes: vec![pane.snapshot()],
            layout: Some(Tile::Pane(0)),
            selected_pane: 0,
            appearance: Default::default(),
            demo: false,
            focused_pane: None,
            sidebar_open: true,
            sidebar_width: 300.0,
            help_open: false,
            analysis: Default::default(),
            meter_stereo: true,
            meter_channel: ChannelMode::StereoMix,
            window_size: None,
            window_position: None,
            active_preset: "user:test".into(),
        };
        assert!(matches(&session, &preset));
        session
            .signals
            .channels
            .last_mut()
            .unwrap()
            .response
            .gain_db = 6.0;
        assert!(!matches(&session, &preset));
    }

    #[test]
    fn invalid_definitions_and_duplicate_component_ids_are_rejected() {
        let mut project = example();
        project.signals.channels[0].response.envelope.attack = 0.0;
        assert!(!project.validate());
        let mut project = example();
        project.panes.push(project.panes[0].clone());
        assert!(!project.validate());
    }
    #[test]
    fn switching_visualizers_and_deleting_sources_never_creates_or_rebinds_channels() {
        let mut bank = Bank::default();
        let mut pane = ModulePane::new(ModuleKind::Flux);
        for kind in [ModuleKind::Flux, ModuleKind::Veroni, ModuleKind::Strata] {
            pane.kind = kind;
            pane.reconcile_signals(&bank);
            assert!(bank.channels.is_empty());
            let mut settings = pane.copy_settings();
            for r in &settings.modulation_mut().unwrap().routes {
                assert!(r.signal.is_none());
                assert!(!r.enabled || (r.target.layered() && r.layer == 0));
            }
        }
        let id = bank.add(crate::signals::Method::Band).unwrap();
        for kind in [ModuleKind::Flux, ModuleKind::Veroni, ModuleKind::Strata] {
            pane.kind = kind;
            let mut settings = pane.copy_settings();
            let m = settings.modulation_mut().unwrap();
            m.routes.push(crate::modulation::Route {
                id: 99,
                target: Target::Brightness,
                signal: Some(id),
                ..Default::default()
            });
            pane.paste_settings(&settings);
        }
        let deleted = bank.remove_channel(id).unwrap().0;
        pane.reconcile_signals(&bank); // all retained module types, not just the selected one
        for kind in [ModuleKind::Flux, ModuleKind::Veroni, ModuleKind::Strata] {
            pane.kind = kind;
            assert!(
                pane.copy_settings()
                    .modulation_mut()
                    .unwrap()
                    .routes
                    .iter()
                    .all(|r| r.signal != Some(id))
            );
        }
        bank.channels.push(deleted); // an undo restores the source, not the user's removed links
        pane.reconcile_signals(&bank);
        assert_eq!(bank.channels.len(), 1);
        for settings in &mut pane.snapshot().settings {
            if let Some(m) = settings.modulation_mut() {
                assert!(m.routes.iter().all(|r| r.signal != Some(id)));
            }
        }
    }
}
