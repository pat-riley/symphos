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
pub struct SceneState {
    #[serde(default)]
    pub macros: Option<crate::macros::scene::SceneMacros>,
    pub signals: Bank,
    pub panes: Vec<PaneState>,
    pub layout: Option<Tile>,
    pub selected_pane: usize,
    pub analysis: AnalysisSettings,
}
impl SceneState {
    pub fn capture(session: Session) -> Self {
        Self {
            macros: Some(session.macros.unwrap_or_else(|| {
                crate::macros::scene::SceneMacros::from_legacy(
                    &session.panes,
                    session.selected_pane,
                )
            })),
            signals: session.signals,
            panes: session.panes,
            layout: session.layout,
            selected_pane: session.selected_pane,
            analysis: session.analysis,
        }
    }
    pub fn validate(&self) -> bool {
        let session = Session {
            render_fps: 60,
            macros: self.macros.clone(),
            project_path: None,
            scenes: None,
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
            library_width: crate::workspace::default_library_width(),
            signals_height: crate::workspace::default_signals_height(),
            help_open: false,
            analysis: self.analysis.clone(),
            meter_stereo: true,
            meter_channel: ChannelMode::StereoMix,
            window_size: None,
            window_position: None,
            active_preset: "custom".into(),
        };
        session.validate()
    }
}
/// The portable document owns all scenes. Source connections and GUI preferences stay local.
#[derive(Clone, Serialize, Deserialize)]
pub struct Project {
    pub format: String,
    pub version: u32,
    #[serde(flatten)]
    pub scenes: crate::scenes::Book,
}
impl Project {
    pub fn capture(session: Session) -> Self {
        let mut scenes = session
            .scenes
            .clone()
            .unwrap_or_else(|| crate::scenes::Book::new(SceneState::capture(session.clone())));
        scenes.store(SceneState::capture(session));
        Self {
            format: "symphos-project".into(),
            version: 2,
            scenes,
        }
    }
    pub fn validate(&self) -> bool {
        self.format == "symphos-project" && self.version == 2 && self.scenes.valid()
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        let value: serde_json::Value =
            serde_json::from_slice(bytes).map_err(|e| format!("Could not read project: {e}"))?;
        if value["format"] != "symphos-project" {
            return Err("Not a Symphos project.".into());
        }
        let project = match value["version"].as_u64() {
            Some(1) => {
                let state: SceneState = serde_json::from_value(value).map_err(|e| e.to_string())?;
                Self {
                    format: "symphos-project".into(),
                    version: 2,
                    scenes: crate::scenes::Book::new(state),
                }
            }
            Some(2) => serde_json::from_value(value).map_err(|e| e.to_string())?,
            _ => return Err("Unsupported project version.".into()),
        };
        if !project.validate() {
            return Err("Invalid project; current workspace unchanged.".into());
        }
        Ok(project)
    }
    pub fn save(&self, path: &Path) -> Result<(), String> {
        if !self.validate() {
            return Err("Project contains invalid settings. No file was changed.".into());
        }
        let data = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        if data.len() > crate::scenes::MAX_DOCUMENT_BYTES {
            return Err("Project exceeds the 64 MB document limit.".into());
        }
        crate::workspace::atomic_write(path, &data).map_err(|e| e.to_string())
    }
    pub fn open(path: &Path) -> Result<Self, String> {
        let size = std::fs::metadata(path).map_err(|e| e.to_string())?.len();
        if size > crate::scenes::MAX_DOCUMENT_BYTES as u64 {
            return Err("Project exceeds the 64 MB document limit.".into());
        }
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        Self::decode(&bytes)
    }
}
#[derive(Default)]
pub struct Dialog {
    pub pending_open: Option<std::path::PathBuf>,
    pub pending_new: bool,
    pub waiting_for_save: bool,
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
    if let PresetContent::View { panes, macros, .. } = content {
        if let Some(macros) = macros {
            for slot in &macros.slots {
                if let Some(id) = slot.signal {
                    ids.insert(id);
                }
            }
        }
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
    if let PresetContent::View { panes, macros, .. } = content {
        if let Some(macros) = macros {
            for slot in &mut macros.slots {
                if let Some(id) = slot.signal {
                    slot.signal = resolve(id);
                }
            }
        }
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
            let panes: Vec<_> = layout
                .ids()
                .iter()
                .map(|id| session.panes[*id].clone())
                .collect();
            PresetContent::View {
                macros: session
                    .macros
                    .as_ref()
                    .map(|m| Box::new(m.for_panes(&panes))),
                layout: layout.canonical(),
                panes,
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
    fn example() -> SceneState {
        let mut pane = ModulePane::new(ModuleKind::Flux);
        let mut signals = Bank::default();
        let mut settings = pane.copy_settings();
        for route in &mut settings.modulation_mut().unwrap().routes {
            let id = signals.add(crate::signals::Method::Band).unwrap();
            signals.channels.last_mut().unwrap().name = format!("Manual source {id}");
            route.signal = Some(id);
        }
        pane.paste_settings(&settings);
        SceneState {
            macros: None,
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
        let document = Project {
            format: "symphos-project".into(),
            version: 2,
            scenes: crate::scenes::Book::new(project.clone()),
        };
        document.save(&path).unwrap();
        let restored = Project::open(&path).unwrap();
        let loaded = &restored.scenes.current().unwrap().state;
        assert_eq!(project.signals, loaded.signals);
        assert_eq!(project.panes, loaded.panes);
        assert_eq!(project.layout, loaded.layout);
        let text = std::fs::read_to_string(&path).unwrap();
        // Component appearance is creative state. GUI appearance preferences
        // must stay absent at the project/scene level.
        let json: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert!(json.get("appearance").is_none());
        for scene in json["scenes"].as_array().unwrap() {
            assert!(scene.get("appearance").is_none());
        }
        for excluded in [
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
            render_fps: 60,
            macros: None,
            project_path: None,
            scenes: None,
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
            library_width: crate::workspace::default_library_width(),
            signals_height: crate::workspace::default_signals_height(),
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
