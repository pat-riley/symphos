//! Tiled geometry and settings-only presets. Runtime audio never enters this schema.
use crate::analysis::{AnalysisSettings, ChannelMode, FFT_SIZES};
use crate::modules::{ModuleKind, ModulePane, ModuleSettings, PaneState};
use eframe::egui::{Pos2, Rect};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

mod recovery;

pub const MAX_PANES: usize = 16;
const GAP: f32 = 8.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Axis {
    Horizontal,
    Vertical,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    Left,
    Right,
    Top,
    Bottom,
    Center,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Tile {
    Pane(usize),
    Split {
        axis: Axis,
        fraction: f32,
        first: Box<Tile>,
        second: Box<Tile>,
    },
}

pub struct Divider {
    pub path: Vec<bool>,
    pub axis: Axis,
    pub bounds: Rect,
    pub handle: Rect,
}

impl Tile {
    pub fn split(axis: Axis, fraction: f32, first: Self, second: Self) -> Self {
        Self::Split {
            axis,
            fraction,
            first: Box::new(first),
            second: Box::new(second),
        }
    }
    pub fn default_view() -> Self {
        Self::legacy(0.62, [1.0 / 3.0, 2.0 / 3.0])
    }
    fn legacy(top: f32, columns: [f32; 2]) -> Self {
        Self::split(
            Axis::Vertical,
            top,
            Self::Pane(0),
            Self::split(
                Axis::Horizontal,
                columns[0],
                Self::Pane(1),
                Self::split(
                    Axis::Horizontal,
                    (columns[1] - columns[0]) / (1.0 - columns[0]),
                    Self::Pane(2),
                    Self::Pane(3),
                ),
            ),
        )
    }
    pub fn ids(&self) -> Vec<usize> {
        let mut ids = Vec::new();
        self.visit(&mut |id| ids.push(id));
        ids
    }
    fn visit(&self, f: &mut impl FnMut(usize)) {
        match self {
            Self::Pane(id) => f(*id),
            Self::Split { first, second, .. } => {
                first.visit(f);
                second.visit(f);
            }
        }
    }
    pub fn valid(&self, slots: usize) -> bool {
        fn node(tree: &Tile, depth: usize, slots: usize, seen: &mut Vec<usize>) -> bool {
            if depth > MAX_PANES {
                return false;
            }
            match tree {
                Tile::Pane(id) => {
                    if *id >= slots || seen.contains(id) {
                        return false;
                    }
                    seen.push(*id);
                    seen.len() <= MAX_PANES
                }
                Tile::Split {
                    fraction,
                    first,
                    second,
                    ..
                } => {
                    fraction.is_finite()
                        && (0.1..=0.9).contains(fraction)
                        && node(first, depth + 1, slots, seen)
                        && node(second, depth + 1, slots, seen)
                }
            }
        }
        node(self, 0, slots, &mut Vec::new())
    }
    pub fn insert(&mut self, target: usize, added: usize, edge: Edge) -> bool {
        match self {
            Self::Pane(id) if *id == target && edge != Edge::Center => {
                let existing = Self::Pane(*id);
                let (axis, before) = match edge {
                    Edge::Left => (Axis::Horizontal, true),
                    Edge::Right => (Axis::Horizontal, false),
                    Edge::Top => (Axis::Vertical, true),
                    Edge::Bottom => (Axis::Vertical, false),
                    Edge::Center => unreachable!(),
                };
                *self = if before {
                    Self::split(axis, 0.5, Self::Pane(added), existing)
                } else {
                    Self::split(axis, 0.5, existing, Self::Pane(added))
                };
                true
            }
            Self::Split { first, second, .. } => {
                first.insert(target, added, edge) || second.insert(target, added, edge)
            }
            _ => false,
        }
    }
    pub fn remove(self, target: usize) -> Option<Self> {
        match self {
            Self::Pane(id) => (id != target).then_some(Self::Pane(id)),
            Self::Split {
                axis,
                fraction,
                first,
                second,
            } => match (first.remove(target), second.remove(target)) {
                (Some(a), Some(b)) => Some(Self::split(axis, fraction, a, b)),
                (Some(remaining), None) | (None, Some(remaining)) => Some(remaining),
                (None, None) => None,
            },
        }
    }
    pub fn move_pane(&mut self, source: usize, target: usize, edge: Edge) -> bool {
        let ids = self.ids();
        if source == target || !ids.contains(&source) || !ids.contains(&target) {
            return false;
        }
        if edge == Edge::Center {
            self.remap(&|id| {
                if id == source {
                    target
                } else if id == target {
                    source
                } else {
                    id
                }
            });
        } else {
            let Some(mut next) = self.clone().remove(source) else {
                return false;
            };
            if !next.insert(target, source, edge) {
                return false;
            }
            *self = next;
        }
        true
    }
    pub fn remap(&mut self, map: &impl Fn(usize) -> usize) {
        match self {
            Self::Pane(id) => *id = map(*id),
            Self::Split { first, second, .. } => {
                first.remap(map);
                second.remap(map);
            }
        }
    }
    pub fn canonical(&self) -> Self {
        let ids = self.ids();
        let mut tree = self.clone();
        tree.remap(&|id| ids.iter().position(|slot| *slot == id).unwrap());
        tree
    }
    pub fn set_fraction(&mut self, path: &[bool], value: f32) {
        if let Self::Split {
            fraction,
            first,
            second,
            ..
        } = self
        {
            if let Some((branch, remaining)) = path.split_first() {
                if *branch {
                    second.set_fraction(remaining, value);
                } else {
                    first.set_fraction(remaining, value);
                }
            } else {
                *fraction = value.clamp(0.1, 0.9);
            }
        }
    }
    fn minimum_size(&self) -> eframe::egui::Vec2 {
        match self {
            Self::Pane(_) => eframe::egui::vec2(140.0, 96.0),
            Self::Split {
                axis,
                first,
                second,
                ..
            } => {
                let a = first.minimum_size();
                let b = second.minimum_size();
                if *axis == Axis::Horizontal {
                    eframe::egui::vec2(a.x + b.x + GAP, a.y.max(b.y))
                } else {
                    eframe::egui::vec2(a.x.max(b.x), a.y + b.y + GAP)
                }
            }
        }
    }
    pub fn geometry(&self, rect: Rect) -> (Vec<(usize, Rect)>, Vec<Divider>) {
        fn walk(
            tile: &Tile,
            rect: Rect,
            path: &mut Vec<bool>,
            leaves: &mut Vec<(usize, Rect)>,
            handles: &mut Vec<Divider>,
        ) {
            match tile {
                Tile::Pane(id) => leaves.push((*id, rect)),
                Tile::Split {
                    axis,
                    fraction,
                    first,
                    second,
                } => {
                    let extent = if *axis == Axis::Horizontal {
                        rect.width()
                    } else {
                        rect.height()
                    };
                    let gap = GAP.min(extent * 0.1);
                    let usable = (extent - gap).max(0.0);
                    let a = first.minimum_size();
                    let b = second.minimum_size();
                    let (a, b) = if *axis == Axis::Horizontal {
                        (a.x, b.x)
                    } else {
                        (a.y, b.y)
                    };
                    let actual = if a + b <= usable {
                        fraction.clamp(a / usable, 1.0 - b / usable)
                    } else {
                        a / (a + b)
                    };
                    let coordinate = usable * actual;
                    let (a, b, handle) = if *axis == Axis::Horizontal {
                        let x = rect.left() + coordinate;
                        (
                            Rect::from_min_max(rect.min, Pos2::new(x, rect.bottom())),
                            Rect::from_min_max(Pos2::new(x + gap, rect.top()), rect.max),
                            Rect::from_min_max(
                                Pos2::new(x, rect.top()),
                                Pos2::new(x + gap, rect.bottom()),
                            ),
                        )
                    } else {
                        let y = rect.top() + coordinate;
                        (
                            Rect::from_min_max(rect.min, Pos2::new(rect.right(), y)),
                            Rect::from_min_max(Pos2::new(rect.left(), y + gap), rect.max),
                            Rect::from_min_max(
                                Pos2::new(rect.left(), y),
                                Pos2::new(rect.right(), y + gap),
                            ),
                        )
                    };
                    handles.push(Divider {
                        path: path.clone(),
                        axis: *axis,
                        bounds: rect,
                        handle,
                    });
                    path.push(false);
                    walk(first, a, path, leaves, handles);
                    path.pop();
                    path.push(true);
                    walk(second, b, path, leaves, handles);
                    path.pop();
                }
            }
        }
        let mut leaves = Vec::new();
        let mut handles = Vec::new();
        walk(self, rect, &mut Vec::new(), &mut leaves, &mut handles);
        (leaves, handles)
    }
}

pub fn valid_settings(value: &impl Serialize) -> bool {
    fn bounded(value: &serde_json::Value, name: &str) -> bool {
        match value {
            serde_json::Value::Number(value) => value.as_f64().is_some_and(|n| {
                n.is_finite()
                    && n.abs() <= 384_000.0
                    && match name {
                        "detail" => (1.0..=256.0).contains(&n),
                        "time_pixels" | "frequency_pixels" => (1.0..=1024.0).contains(&n),
                        "seconds" => (0.1..=30.0).contains(&n),
                        "bar_bands" => (1.0..=96.0).contains(&n),
                        "cell_size" => (1.0..=32.0).contains(&n),
                        "cell_gap" => (0.0..=0.85).contains(&n),
                        "unlit" => (0.0..=0.3).contains(&n),
                        "bar_time_step" | "line_spacing" | "y_line_spacing" | "wire_spacing"
                        | "dot_spacing" | "stem_spacing" => (1.0..=8.0).contains(&n),
                        "min_hz" | "max_hz" => n >= 1.0,
                        "afterglow_traces" => (0.0..=16.0).contains(&n),
                        "cycles" => (1.0..=8.0).contains(&n),
                        _ => true,
                    }
            }),
            serde_json::Value::Object(map) => map.iter().all(|(name, value)| bounded(value, name)),
            serde_json::Value::Array(values) => values.iter().all(|value| bounded(value, name)),
            _ => true,
        }
    }
    serde_json::to_value(value).is_ok_and(|value| bounded(&value, ""))
}

fn valid_pane(pane: &PaneState) -> bool {
    if pane.instance_id.is_empty() || pane.instance_id.len() > 120 {
        return false;
    }
    pane.audio_input.valid()
        && pane.milkdrop_presets.len() <= crate::milkdrop::preset::MAX_PRESETS
        && pane.milkdrop_presets.iter().all(|p| p.validate().is_ok())
        && pane.settings.len() == ModuleKind::ALL.len()
        && ModuleKind::ALL.into_iter().all(|kind| {
            pane.settings
                .iter()
                .filter(|settings| settings.kind() == kind)
                .count()
                == 1
        })
        && pane.macros.as_ref().is_none_or(|m| m.valid(&pane.settings))
        && valid_settings(pane)
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Session {
    #[serde(
        default,
        skip_serializing_if = "crate::background::Background::is_default"
    )]
    pub background: crate::background::Background,
    #[serde(default)]
    pub macros: Option<crate::macros::scene::SceneMacros>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scenes: Option<crate::scenes::Book>,
    #[serde(default)]
    pub signals: crate::signals::Bank,
    #[serde(default)]
    pub appearance: crate::theme::AppearancePreferences,
    #[serde(default = "default_render_fps")]
    pub render_fps: u32,
    pub version: u32,
    #[serde(default = "default_demo")]
    pub demo: bool,
    pub panes: Vec<PaneState>,
    pub layout: Option<Tile>,
    pub selected_pane: usize,
    // Legacy expanded-pane state is accepted on load; the UI now uses inspection.
    pub focused_pane: Option<usize>,
    pub sidebar_open: bool,
    pub sidebar_width: f32,
    #[serde(default = "default_library_width")]
    pub library_width: f32,
    #[serde(default = "default_signals_height")]
    pub signals_height: f32,
    pub help_open: bool,
    pub analysis: AnalysisSettings,
    pub meter_stereo: bool,
    pub meter_channel: ChannelMode,
    pub window_size: Option<[f32; 2]>,
    pub window_position: Option<[f32; 2]>,
    #[serde(default = "default_preset")]
    pub active_preset: String,
}
pub fn default_render_fps() -> u32 {
    60
}
pub const fn default_library_width() -> f32 {
    236.0
}
pub fn default_signals_height() -> f32 {
    390.0
}
fn default_demo() -> bool {
    true
}
fn default_preset() -> String {
    "factory:default".into()
}
impl Session {
    pub fn validate(&self) -> bool {
        let ids = self.layout.as_ref().map_or_else(Vec::new, Tile::ids);
        self.scenes.as_ref().is_none_or(crate::scenes::Book::valid)
            && self.macros.as_ref().is_none_or(|m| m.valid(&self.panes))
            && self.background.valid()
            && self.signals.valid()
            && self.appearance.valid()
            && [30, 60, 90, 120].contains(&self.render_fps)
            && self.version == 2
            && !self.panes.is_empty()
            && self.panes.len() <= MAX_PANES
            && self.panes.iter().all(valid_pane)
            && {
                let mut ids = std::collections::HashSet::new();
                self.panes.iter().all(|p| ids.insert(&p.instance_id))
            }
            && self
                .layout
                .as_ref()
                .is_none_or(|tree| tree.valid(self.panes.len()))
            && self.selected_pane < self.panes.len()
            && (ids.is_empty() || ids.contains(&self.selected_pane))
            && self.focused_pane.is_none_or(|id| ids.contains(&id))
            && (160.0..=640.0).contains(&self.sidebar_width)
            && (default_library_width()..=440.0).contains(&self.library_width)
            && (180.0..=1000.0).contains(&self.signals_height)
            && FFT_SIZES.contains(&self.analysis.fft_size)
            && (1..=240).contains(&self.analysis.analysis_fps)
            && self.window_size.is_none_or(|size| {
                (1024.0..=8192.0).contains(&size[0]) && (700.0..=8192.0).contains(&size[1])
            })
            && self.active_preset.len() <= 120
            && valid_settings(self)
    }
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        let mut value: serde_json::Value = serde_json::from_slice(bytes).ok()?;
        // Preserve development workspaces created by the old four-pane dashboard.
        if value["version"] == 1 {
            let top = value["top_fraction"].as_f64()? as f32;
            let splits = [
                value["bottom_splits"][0].as_f64()? as f32,
                value["bottom_splits"][1].as_f64()? as f32,
            ];
            if !(0.35..=0.75).contains(&top)
                || splits[0] < 0.15
                || splits[1] > 0.85
                || splits[1] - splits[0] < 0.15
            {
                return None;
            }
            value["layout"] = serde_json::to_value(Tile::legacy(top, splits)).ok()?;
            value["version"] = 2.into();
        }
        let state: Self = serde_json::from_value(value).ok()?;
        state.validate().then_some(state)
    }
    pub fn load(dir: &Path) -> Option<Self> {
        let path = dir.join("workspace.json");
        if !path.exists() {
            return None;
        }
        let state = std::fs::read(&path)
            .ok()
            .filter(|b| b.len() <= crate::scenes::MAX_DOCUMENT_BYTES)
            .and_then(|bytes| Self::decode(&bytes));
        if state.is_none() {
            let _ = std::fs::copy(&path, dir.join("workspace.rejected.json"));
            crate::issues::record(
                "Workspace",
                "Invalid or incompatible workspace; starting with defaults. Original retained as workspace.rejected.json.",
            );
        }
        state
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum PresetContent {
    Layout(Tile),
    View {
        #[serde(
            default,
            skip_serializing_if = "crate::background::Background::is_default"
        )]
        background: crate::background::Background,
        layout: Tile,
        panes: Vec<PaneState>,
        #[serde(default)]
        macros: Option<Box<crate::macros::scene::SceneMacros>>,
    },
    Component(Box<ModuleSettings>),
}
impl PresetContent {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Layout(_) => "Layout",
            Self::View { .. } => "Full view",
            Self::Component(_) => "Component",
        }
    }
    pub fn tree(&self) -> Option<&Tile> {
        match self {
            Self::Layout(tree) | Self::View { layout: tree, .. } => Some(tree),
            Self::Component(_) => None,
        }
    }
    fn valid(&self) -> bool {
        match self {
            Self::Component(settings) => valid_settings(settings),
            Self::Layout(tree) | Self::View { layout: tree, .. } => {
                let ids = tree.ids();
                tree.valid(ids.len())
                    && ids.iter().enumerate().all(|(i, id)| i == *id)
                    && match self {
                        Self::View {
                            background,
                            panes,
                            macros,
                            ..
                        } => {
                            background.valid()
                                && panes.len() == ids.len()
                                && panes.iter().all(valid_pane)
                                && macros.as_ref().is_none_or(|m| m.valid(panes))
                        }
                        _ => true,
                    }
            }
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Preset {
    #[serde(default)]
    pub audio_inputs: Vec<crate::signals::audio::InputBus>,
    #[serde(default)]
    pub signals: Vec<crate::signals::Definition>,
    pub id: String,
    pub name: String,
    pub content: PresetContent,
}
impl Preset {
    pub fn factory(&self) -> bool {
        self.id.starts_with("factory:")
    }
}

pub fn factory_presets() -> Vec<Preset> {
    use Axis::{Horizontal as H, Vertical as V};
    let view = |id: &str, name: &str, layout: Tile, kinds: &[ModuleKind]| Preset {
        audio_inputs: Vec::new(),
        signals: Vec::new(),
        id: format!("factory:{id}"),
        name: name.into(),
        content: PresetContent::View {
            background: Default::default(),
            macros: None,
            layout,
            panes: kinds
                .iter()
                .map(|kind| ModulePane::new(*kind).snapshot())
                .collect(),
        },
    };
    let mut presets = vec![
        view(
            "default",
            "Default view",
            Tile::default_view(),
            &[
                ModuleKind::Waterfall,
                ModuleKind::Spectrum,
                ModuleKind::Waveform,
                ModuleKind::Spectrogram,
            ],
        ),
        view(
            "waterfall",
            "Waterfall focus",
            Tile::Pane(0),
            &[ModuleKind::Waterfall],
        ),
        view(
            "veroni",
            "Veroni flow",
            Tile::Pane(0),
            &[ModuleKind::Veroni],
        ),
        view(
            "strata",
            "Strata chamber",
            Tile::Pane(0),
            &[ModuleKind::Strata],
        ),
        view(
            "sphere",
            "Spherical spectrum",
            Tile::Pane(0),
            &[ModuleKind::Sphere],
        ),
        view(
            "cylinder",
            "Cylindrical spectrum",
            Tile::Pane(0),
            &[ModuleKind::Cylinder],
        ),
        view(
            "rings",
            "Orbital waveforms",
            Tile::Pane(0),
            &[ModuleKind::Rings],
        ),
        view("flux", "Flux currents", Tile::Pane(0), &[ModuleKind::Flux]),
        view(
            "orbital",
            "Orbital flocking",
            Tile::Pane(0),
            &[ModuleKind::Orbital],
        ),
        view(
            "resonance",
            "Resonance petals",
            Tile::Pane(0),
            &[ModuleKind::Resonance],
        ),
        view(
            "stereo",
            "Stereo lab",
            Tile::split(
                H,
                0.6,
                Tile::Pane(0),
                Tile::split(V, 0.5, Tile::Pane(1), Tile::Pane(2)),
            ),
            &[
                ModuleKind::Stereometer,
                ModuleKind::Spectrum,
                ModuleKind::Oscilloscope,
            ],
        ),
    ];
    for (id, name, layout) in [
        ("single", "Single pane", Tile::Pane(0)),
        (
            "columns",
            "Two columns",
            Tile::split(H, 0.5, Tile::Pane(0), Tile::Pane(1)),
        ),
        (
            "rows",
            "Two rows",
            Tile::split(V, 0.5, Tile::Pane(0), Tile::Pane(1)),
        ),
        (
            "grid",
            "Four-pane grid",
            Tile::split(
                V,
                0.5,
                Tile::split(H, 0.5, Tile::Pane(0), Tile::Pane(1)),
                Tile::split(H, 0.5, Tile::Pane(2), Tile::Pane(3)),
            ),
        ),
        ("default-layout", "Waterfall layout", Tile::default_view()),
    ] {
        presets.push(Preset {
            audio_inputs: Vec::new(),
            signals: Vec::new(),
            id: format!("factory:{id}"),
            name: name.into(),
            content: PresetContent::Layout(layout),
        });
    }
    presets
}

#[derive(Serialize, Deserialize)]
struct UserLibrary {
    version: u32,
    presets: Vec<Preset>,
}
pub struct Storage {
    pub dir: PathBuf,
    pub presets: Vec<Preset>,
    pub notice: Option<String>,
    pub active: String,
    pub modified: bool,
    last_save: Instant,
    recovery: Option<recovery::Writer>,
}
pub fn normal_directory() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join("Library/Application Support/Symphos")
    }
    #[cfg(not(target_os = "macos"))]
    {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(std::env::temp_dir)
                    .join(".config")
            })
            .join("symphos")
    }
}
pub fn directory() -> PathBuf {
    #[cfg(feature = "development")]
    if let Some(dev) = crate::development::options() {
        return dev.dir.clone();
    }
    normal_directory()
}
impl Storage {
    pub fn new(dir: PathBuf) -> Self {
        let mut presets = factory_presets();
        let path = dir.join("presets.json");
        let notice = if path.exists() {
            let library = std::fs::read(&path)
                .ok()
                .filter(|bytes| bytes.len() <= 8_000_000)
                .and_then(|bytes| serde_json::from_slice::<UserLibrary>(&bytes).ok())
                .filter(|lib| {
                    lib.version == 1
                        && lib.presets.len() <= 256
                        && lib.presets.iter().all(|p| {
                            !p.factory()
                                && !p.id.is_empty()
                                && p.id.len() <= 120
                                && valid_name(&p.name)
                                && p.content.valid()
                                && crate::signals::Bank {
                                    audio_inputs: p.audio_inputs.clone(),
                                    slots: Vec::new(),
                                    channels: p.signals.clone(),
                                    next_id: p
                                        .signals
                                        .iter()
                                        .map(|d| d.id)
                                        .chain(p.audio_inputs.iter().map(|b| b.id))
                                        .max()
                                        .unwrap_or(0)
                                        .saturating_add(1),
                                }
                                .valid()
                        })
                        && {
                            let mut ids = std::collections::HashSet::new();
                            lib.presets.iter().all(|p| ids.insert(&p.id))
                        }
                });
            if let Some(lib) = library {
                presets.extend(lib.presets);
                None
            } else {
                let _ = std::fs::copy(&path, dir.join("presets.rejected.json"));
                Some("Could not load presets. Original retained as presets.rejected.json.".into())
            }
        } else {
            None
        };
        Self {
            dir,
            presets,
            notice,
            active: default_preset(),
            modified: false,
            last_save: Instant::now(),
            recovery: None,
        }
    }
    pub fn save_due(&self) -> bool {
        self.last_save.elapsed() >= Duration::from_millis(750)
            && self.recovery.as_ref().is_none_or(|writer| !writer.busy())
    }
    fn checkpoint_result(&mut self, outcome: recovery::Outcome) {
        if outcome.active == self.active
            && let Some(modified) = outcome.modified
        {
            self.modified = modified;
        }
        if let Some(error) = outcome.error {
            self.notice = Some(error);
        }
    }
    pub fn poll_checkpoint(&mut self) {
        if let Some(outcome) = self.recovery.as_mut().and_then(recovery::Writer::poll) {
            self.checkpoint_result(outcome);
        }
    }
    pub fn checkpoint_async(&mut self, session: Session) {
        self.last_save = Instant::now();
        let preset = self
            .presets
            .iter()
            .find(|p| p.id == session.active_preset)
            .cloned();
        let writer = self
            .recovery
            .get_or_insert_with(|| recovery::Writer::new(self.dir.join("workspace.json")));
        if let Err(error) = writer.submit(recovery::Job { session, preset }) {
            self.notice = Some(error);
        }
    }
    fn finish_checkpoint(&mut self) {
        if let Some(outcome) = self.recovery.as_mut().and_then(recovery::Writer::finish) {
            self.checkpoint_result(outcome);
        }
    }
    /// Closing and explicit recovery checkpoints flush in order. An older
    /// background save must never overwrite the final workspace on exit.
    pub fn checkpoint(&mut self, session: &Session) {
        self.finish_checkpoint();
        self.checkpoint_async(session.clone());
        self.finish_checkpoint();
    }
    pub fn save_library(&mut self) {
        let library = UserLibrary {
            version: 1,
            presets: self
                .presets
                .iter()
                .filter(|p| !p.factory())
                .cloned()
                .collect(),
        };
        match serde_json::to_vec_pretty(&library)
            .map_err(std::io::Error::other)
            .and_then(|bytes| atomic_write(&self.dir.join("presets.json"), &bytes))
        {
            Ok(()) => self.notice = None,
            Err(error) => self.notice = Some(format!("Preset save failed: {error}")),
        }
    }
}
pub fn valid_name(name: &str) -> bool {
    !name.trim().is_empty() && name.chars().count() <= 80 && !name.chars().any(char::is_control)
}
pub fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temp = path.with_extension("tmp");
    std::fs::write(&temp, bytes)?;
    std::fs::rename(temp, path)
}
impl Session {
    pub fn matches(&self, preset: &PresetContent) -> bool {
        match preset {
            PresetContent::Layout(tree) => self
                .layout
                .as_ref()
                .is_some_and(|current| current.canonical() == *tree),
            PresetContent::View {
                background,
                layout,
                panes,
                macros,
            } => self.layout.as_ref().is_some_and(|current| {
                self.background == *background
                    && current.canonical() == *layout
                    && (macros.is_none() || self.macros.as_ref() == macros.as_deref())
                    && current.ids().iter().zip(panes).all(|(id, saved)| {
                        let current = &self.panes[*id];
                        current.kind == saved.kind
                            && current.sections == saved.sections
                            && current.settings == saved.settings
                    })
            }),
            PresetContent::Component(settings) => {
                self.panes.get(self.selected_pane).is_some_and(|pane| {
                    pane.kind == settings.kind()
                        && pane
                            .settings
                            .iter()
                            .any(|current| current == settings.as_ref())
                })
            }
        }
    }
}

pub fn drop_zone(rect: Rect, pos: Pos2) -> (Edge, Rect) {
    let x = (pos.x - rect.left()) / rect.width().max(1.0);
    let y = (pos.y - rect.top()) / rect.height().max(1.0);
    let nearest = x.min(1.0 - x).min(y.min(1.0 - y));
    if nearest >= 0.25 {
        return (Edge::Center, rect.shrink(12.0));
    }
    if nearest == x {
        (
            Edge::Left,
            Rect::from_min_max(rect.min, Pos2::new(rect.center().x, rect.bottom())),
        )
    } else if nearest == 1.0 - x {
        (
            Edge::Right,
            Rect::from_min_max(Pos2::new(rect.center().x, rect.top()), rect.max),
        )
    } else if nearest == y {
        (
            Edge::Top,
            Rect::from_min_max(rect.min, Pos2::new(rect.right(), rect.center().y)),
        )
    } else {
        (
            Edge::Bottom,
            Rect::from_min_max(Pos2::new(rect.left(), rect.center().y), rect.max),
        )
    }
}

/// Geometry presets retain component instances in visual order. Hidden slots
/// remain available for a subsequent larger layout without losing settings.
pub fn apply_layout(panes: &mut Vec<ModulePane>, previous: Option<&Tile>, template: &Tile) -> Tile {
    let mut slots = previous.map_or_else(Vec::new, Tile::ids);
    let count = template.ids().len();
    for id in 0..panes.len() {
        if !slots.contains(&id) {
            slots.push(id);
        }
    }
    while slots.len() < count {
        panes.push(ModulePane::new(ModuleKind::Spectrum));
        slots.push(panes.len() - 1);
    }
    let mut tree = template.clone();
    tree.remap(&|id| slots[id]);
    for id in slots.into_iter().skip(count) {
        panes[id].reset();
    }
    tree
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::Vec2;
    #[test]
    fn closing_promotes_sibling_without_touching_unrelated_proportions() {
        let tree = Tile::default_view().remove(2).unwrap();
        assert_eq!(tree.ids(), [0, 1, 3]);
        let (rects, _) = tree.geometry(Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 800.0)));
        assert!(rects[2].1.width() > 600.0);
        assert!(matches!(tree, Tile::Split { fraction: 0.62, .. }));
        assert_eq!(Tile::Pane(0).remove(0), None);
    }
    #[test]
    fn rearranging_is_atomic_and_keeps_every_pane_once() {
        let mut tree = Tile::default_view();
        assert!(tree.move_pane(0, 3, Edge::Right));
        assert!(tree.valid(4));
        assert_eq!(tree.ids().len(), 4);
        let before = tree.clone();
        assert!(!tree.move_pane(0, 99, Edge::Left));
        assert_eq!(before, tree);
        assert!(tree.move_pane(1, 2, Edge::Center));
        assert!(tree.valid(4));
    }
    #[test]
    fn malformed_geometry_and_duplicate_panes_are_rejected() {
        assert!(!Tile::split(Axis::Horizontal, 0.5, Tile::Pane(0), Tile::Pane(0)).valid(2));
        assert!(!Tile::split(Axis::Vertical, f32::NAN, Tile::Pane(0), Tile::Pane(1)).valid(2));
        assert!(!Tile::Pane(16).valid(16));
        for preset in factory_presets() {
            assert!(preset.content.valid(), "{}", preset.name);
        }
    }
    #[test]
    fn small_window_geometry_stays_positive_and_nonoverlapping() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(48.0, 32.0));
        let (leaves, _) = Tile::default_view().geometry(rect);
        for (i, (_, cell)) in leaves.iter().enumerate() {
            assert!(cell.width() > 0.0 && cell.height() > 0.0 && rect.contains_rect(*cell));
            for (_, other) in &leaves[i + 1..] {
                assert!(!cell.intersects(*other));
            }
        }
    }
    #[test]
    fn layout_presets_contain_no_component_configuration() {
        let encoded = serde_json::to_string(&PresetContent::Layout(Tile::default_view())).unwrap();
        assert!(!encoded.contains("Waterfall"));
        assert!(!encoded.contains("settings"));
        let canonical =
            Tile::split(Axis::Horizontal, 0.3, Tile::Pane(7), Tile::Pane(2)).canonical();
        assert_eq!(canonical.ids(), [0, 1]);
    }
    #[test]
    fn six_component_workspaces_and_view_presets_upgrade_without_losing_settings() {
        let mut original = session();
        original.panes[0].sections[0] = 2;
        let mut legacy = serde_json::to_value(&original).unwrap();
        for pane in legacy["panes"].as_array_mut().unwrap() {
            pane["sections"].as_array_mut().unwrap().truncate(6);
            pane["settings"].as_array_mut().unwrap().retain(|entry| {
                entry.get("Veroni").is_none()
                    && entry.get("Strata").is_none()
                    && entry.get("Flux").is_none()
                    && entry.get("Sphere").is_none()
                    && entry.get("Cylinder").is_none()
                    && entry.get("Rings").is_none()
                    && entry.get("Resonance").is_none()
                    && entry.get("Orbital").is_none()
                    && entry.get("Milkdrop").is_none()
                    && entry.get("Cymatics").is_none()
                    && entry.get("ParticleWaves").is_none()
                    && entry.get("SpectralBloom").is_none()
            });
        }
        let decoded = Session::decode(&serde_json::to_vec(&legacy).unwrap()).unwrap();
        assert_eq!(decoded.panes, original.panes);
        assert_eq!(decoded.layout, original.layout);
        let preset =
            serde_json::json!({"View": {"layout": legacy["layout"], "panes": legacy["panes"]}});
        let restored: PresetContent = serde_json::from_value(preset).unwrap();
        assert!(restored.valid());
        assert!(decoded.matches(&restored));
        // Adding the new default must not mask a missing or duplicated old component.
        legacy["panes"][0]["settings"][1] = legacy["panes"][0]["settings"][0].clone();
        assert!(Session::decode(&serde_json::to_vec(&legacy).unwrap()).is_none());
    }

    #[test]
    fn seven_component_workspace_upgrade_preserves_custom_veroni() {
        let mut original = session();
        let mut pane = ModulePane::new(ModuleKind::Veroni);
        let mut value = serde_json::to_value(pane.copy_settings()).unwrap();
        value["Veroni"]["cells"] = 213.into();
        value["Veroni"]["edge_opacity"] = 0.2.into();
        pane.paste_settings(&serde_json::from_value(value).unwrap());
        original.panes[0] = pane.snapshot();
        let mut legacy = serde_json::to_value(&original).unwrap();
        for pane in legacy["panes"].as_array_mut().unwrap() {
            pane["sections"].as_array_mut().unwrap().truncate(7);
            pane["settings"].as_array_mut().unwrap().retain(|entry| {
                entry.get("Strata").is_none()
                    && entry.get("Flux").is_none()
                    && entry.get("Sphere").is_none()
                    && entry.get("Cylinder").is_none()
                    && entry.get("Rings").is_none()
                    && entry.get("Resonance").is_none()
                    && entry.get("Orbital").is_none()
                    && entry.get("Milkdrop").is_none()
                    && entry.get("Cymatics").is_none()
                    && entry.get("ParticleWaves").is_none()
                    && entry.get("SpectralBloom").is_none()
            });
        }
        let decoded = Session::decode(&serde_json::to_vec(&legacy).unwrap()).unwrap();
        assert_eq!(decoded.panes, original.panes);
        let preset =
            serde_json::json!({"View": {"layout": legacy["layout"], "panes": legacy["panes"]}});
        let restored: PresetContent = serde_json::from_value(preset).unwrap();
        assert!(restored.valid() && decoded.matches(&restored));
    }

    #[test]
    fn eight_component_workspace_upgrade_preserves_custom_strata() {
        let mut original = session();
        let mut pane = ModulePane::new(ModuleKind::Strata);
        let mut value = serde_json::to_value(pane.copy_settings()).unwrap();
        value["Strata"]["layers"] = 11.into();
        value["Strata"]["yaw"] = 65.0.into();
        pane.paste_settings(&serde_json::from_value(value).unwrap());
        original.panes[0] = pane.snapshot();
        let mut legacy = serde_json::to_value(&original).unwrap();
        for pane in legacy["panes"].as_array_mut().unwrap() {
            pane["sections"].as_array_mut().unwrap().truncate(8);
            pane["settings"].as_array_mut().unwrap().retain(|entry| {
                entry.get("Flux").is_none()
                    && entry.get("Sphere").is_none()
                    && entry.get("Cylinder").is_none()
                    && entry.get("Rings").is_none()
                    && entry.get("Resonance").is_none()
                    && entry.get("Orbital").is_none()
                    && entry.get("Milkdrop").is_none()
                    && entry.get("Cymatics").is_none()
                    && entry.get("ParticleWaves").is_none()
                    && entry.get("SpectralBloom").is_none()
            });
        }
        let decoded = Session::decode(&serde_json::to_vec(&legacy).unwrap()).unwrap();
        assert_eq!(decoded.panes, original.panes);
        let preset =
            serde_json::json!({"View": {"layout": legacy["layout"], "panes": legacy["panes"]}});
        let restored: PresetContent = serde_json::from_value(preset).unwrap();
        assert!(restored.valid() && decoded.matches(&restored));
    }

    #[test]
    fn nine_component_projects_keep_macro_banks_when_adding_sphere() {
        let original = session();
        let mut legacy = serde_json::to_value(&original).unwrap();
        for pane in legacy["panes"].as_array_mut().unwrap() {
            pane["sections"].as_array_mut().unwrap().truncate(9);
            pane["settings"].as_array_mut().unwrap().retain(|s| {
                s.get("Sphere").is_none()
                    && s.get("Cylinder").is_none()
                    && s.get("Rings").is_none()
                    && s.get("Resonance").is_none()
                    && s.get("Orbital").is_none()
                    && s.get("Milkdrop").is_none()
                    && s.get("Cymatics").is_none()
                    && s.get("ParticleWaves").is_none()
                    && s.get("SpectralBloom").is_none()
            });
            pane["macros"]["banks"].as_array_mut().unwrap().truncate(9);
            pane["macros"]["banks"][8][0]["depth"] = serde_json::json!(-0.75);
        }
        let decoded = Session::decode(&serde_json::to_vec(&legacy).unwrap()).unwrap();
        for pane in &decoded.panes {
            assert_eq!(pane.settings.len(), ModuleKind::ALL.len());
            let macros = pane.macros.as_ref().unwrap();
            assert_eq!(macros.banks[8][0].depth, -0.75);
            assert!(
                macros.banks[9]
                    .iter()
                    .all(|slot| slot.parameter.is_none() && slot.signal.is_none())
            );
        }
        legacy["panes"][0]["settings"][1] = legacy["panes"][0]["settings"][0].clone();
        assert!(Session::decode(&serde_json::to_vec(&legacy).unwrap()).is_none());
    }

    #[test]
    fn ten_component_projects_preserve_sphere_settings_and_macros_when_adding_cylinder() {
        let mut original = session();
        let mut pane = ModulePane::new(ModuleKind::Sphere);
        let mut value = serde_json::to_value(pane.copy_settings()).unwrap();
        value["Sphere"]["radius"] = 1.6.into();
        pane.paste_settings(&serde_json::from_value(value).unwrap());
        original.panes[0] = pane.snapshot();
        let mut legacy = serde_json::to_value(&original).unwrap();
        for pane in legacy["panes"].as_array_mut().unwrap() {
            pane["sections"].as_array_mut().unwrap().truncate(10);
            pane["settings"].as_array_mut().unwrap().retain(|s| {
                s.get("Cylinder").is_none()
                    && s.get("Rings").is_none()
                    && s.get("Resonance").is_none()
                    && s.get("Orbital").is_none()
                    && s.get("Milkdrop").is_none()
                    && s.get("Cymatics").is_none()
                    && s.get("ParticleWaves").is_none()
                    && s.get("SpectralBloom").is_none()
            });
            for settings in pane["settings"].as_array_mut().unwrap() {
                if let Some(sphere) = settings.get_mut("Sphere") {
                    sphere.as_object_mut().unwrap().remove("cylinder");
                }
            }
            pane["macros"]["banks"].as_array_mut().unwrap().truncate(10);
            pane["macros"]["banks"][9][0]["depth"] = (-0.6).into();
        }
        let decoded = Session::decode(&serde_json::to_vec(&legacy).unwrap()).unwrap();
        assert_eq!(decoded.panes[0].kind, ModuleKind::Sphere);
        assert_eq!(decoded.panes[0].settings, original.panes[0].settings);
        for pane in &decoded.panes {
            let macros = pane.macros.as_ref().unwrap();
            assert_eq!(macros.banks[9][0].depth, -0.6);
            assert!(
                macros.banks[10]
                    .iter()
                    .all(|slot| slot.parameter.is_none() && slot.signal.is_none())
            );
        }
        legacy["panes"][0]["settings"][1] = legacy["panes"][0]["settings"][0].clone();
        assert!(Session::decode(&serde_json::to_vec(&legacy).unwrap()).is_none());
    }

    #[test]
    fn twelve_component_projects_preserve_rings_and_add_resonance_defaults() {
        let mut original = session();
        original.panes[0] = ModulePane::new(ModuleKind::Rings).snapshot();
        let mut legacy = serde_json::to_value(&original).unwrap();
        for pane in legacy["panes"].as_array_mut().unwrap() {
            pane["sections"].as_array_mut().unwrap().truncate(12);
            pane["settings"].as_array_mut().unwrap().retain(|s| {
                s.get("Resonance").is_none()
                    && s.get("Orbital").is_none()
                    && s.get("Milkdrop").is_none()
                    && s.get("Cymatics").is_none()
                    && s.get("ParticleWaves").is_none()
                    && s.get("SpectralBloom").is_none()
            });
            pane["macros"]["banks"].as_array_mut().unwrap().truncate(12);
            pane["macros"]["banks"][11][0]["depth"] = 0.6.into();
        }
        let decoded = Session::decode(&serde_json::to_vec(&legacy).unwrap()).unwrap();
        assert_eq!(decoded.panes[0].kind, ModuleKind::Rings);
        assert_eq!(decoded.panes[0].settings, original.panes[0].settings);
        for pane in &decoded.panes {
            let banks = &pane.macros.as_ref().unwrap().banks;
            assert_eq!(banks[11][0].depth, 0.6);
            assert!(
                banks[12]
                    .iter()
                    .all(|s| s.parameter.is_none() && s.signal.is_none())
            );
        }
        assert_eq!(
            decoded.signals.channels.len(),
            original.signals.channels.len()
        );
    }

    #[test]
    fn eleven_component_projects_keep_cylinder_settings_and_add_empty_rings_macros() {
        let mut original = session();
        let mut pane = ModulePane::new(ModuleKind::Cylinder);
        let mut value = serde_json::to_value(pane.copy_settings()).unwrap();
        value["Cylinder"]["cylinder"]["height"] = 3.6.into();
        pane.paste_settings(&serde_json::from_value(value).unwrap());
        original.panes[0] = pane.snapshot();
        let mut legacy = serde_json::to_value(&original).unwrap();
        for pane in legacy["panes"].as_array_mut().unwrap() {
            pane["sections"].as_array_mut().unwrap().truncate(11);
            pane["settings"].as_array_mut().unwrap().retain(|s| {
                s.get("Rings").is_none()
                    && s.get("Resonance").is_none()
                    && s.get("Orbital").is_none()
                    && s.get("Milkdrop").is_none()
                    && s.get("Cymatics").is_none()
                    && s.get("ParticleWaves").is_none()
                    && s.get("SpectralBloom").is_none()
            });
            pane["macros"]["banks"].as_array_mut().unwrap().truncate(11);
            pane["macros"]["banks"][10][0]["depth"] = 0.7.into();
        }
        let decoded = Session::decode(&serde_json::to_vec(&legacy).unwrap()).unwrap();
        assert_eq!(decoded.panes[0].kind, ModuleKind::Cylinder);
        assert_eq!(decoded.panes[0].settings, original.panes[0].settings);
        for pane in &decoded.panes {
            let macros = pane.macros.as_ref().unwrap();
            assert_eq!(macros.banks[10][0].depth, 0.7);
            assert!(
                macros.banks[11]
                    .iter()
                    .all(|s| s.parameter.is_none() && s.signal.is_none())
            );
        }
        assert_eq!(
            decoded.signals.channels.len(),
            original.signals.channels.len()
        );
    }

    #[test]
    fn all_component_snapshots_roundtrip_without_captured_audio() {
        for kind in ModuleKind::ALL {
            let state = ModulePane::new(kind).snapshot();
            let encoded = serde_json::to_string(&state).unwrap();
            assert!(!encoded.contains("waveform_left"));
            let restored: PaneState = serde_json::from_str(&encoded).unwrap();
            let mut pane = ModulePane::new(ModuleKind::Spectrum);
            pane.restore(&restored);
            assert_eq!(pane.snapshot(), state);
            assert!(valid_pane(&state));
        }
    }

    fn session() -> Session {
        Session {
            background: Default::default(),
            macros: None,
            project_path: None,
            scenes: None,
            signals: crate::signals::Bank::default(),
            appearance: crate::theme::AppearancePreferences::default(),
            version: 2,
            demo: true,
            panes: [
                ModuleKind::Waterfall,
                ModuleKind::Spectrum,
                ModuleKind::Waveform,
                ModuleKind::Spectrogram,
            ]
            .into_iter()
            .map(|kind| ModulePane::new(kind).snapshot())
            .collect(),
            layout: Some(Tile::default_view()),
            selected_pane: 0,
            focused_pane: None,
            sidebar_open: true,
            sidebar_width: 236.0,
            library_width: default_library_width(),
            signals_height: default_signals_height(),
            help_open: false,
            render_fps: 60,
            analysis: AnalysisSettings::default(),
            meter_stereo: true,
            meter_channel: ChannelMode::StereoMix,
            window_size: None,
            window_position: None,
            active_preset: default_preset(),
        }
    }

    #[test]
    fn applying_layouts_preserves_custom_settings_and_reopens_hidden_slots() {
        let mut panes: Vec<_> = session()
            .panes
            .iter()
            .map(|state| {
                let mut pane = ModulePane::new(state.kind);
                pane.restore(state);
                pane
            })
            .collect();
        let mut value = serde_json::to_value(panes[1].copy_settings()).unwrap();
        value["Spectrum"]["labels"] = false.into();
        let custom: ModuleSettings = serde_json::from_value(value).unwrap();
        assert!(panes[1].paste_settings(&custom));
        let original: Vec<_> = panes.iter().map(ModulePane::snapshot).collect();
        let one = apply_layout(&mut panes, Some(&Tile::default_view()), &Tile::Pane(0));
        assert_eq!(one.ids(), [0]);
        let four = apply_layout(&mut panes, Some(&one), &Tile::default_view());
        assert_eq!(four.ids(), [0, 1, 2, 3]);
        assert_eq!(
            panes.iter().map(ModulePane::snapshot).collect::<Vec<_>>(),
            original
        );
    }

    #[test]
    fn legacy_dev_workspace_migrates_and_invalid_sessions_are_rejected() {
        let original = session();
        let mut legacy = serde_json::to_value(&original).unwrap();
        legacy.as_object_mut().unwrap().remove("layout");
        legacy["version"] = 1.into();
        legacy["top_fraction"] = 0.62.into();
        legacy["bottom_splits"] = serde_json::json!([1.0 / 3.0, 2.0 / 3.0]);
        let restored = Session::decode(&serde_json::to_vec(&legacy).unwrap()).unwrap();
        assert_eq!(restored.version, 2);
        assert_eq!(restored.layout, original.layout);
        for (key, value) in [
            ("version", serde_json::json!(99)),
            ("selected_pane", serde_json::json!(99)),
            ("sidebar_width", serde_json::json!(-1)),
        ] {
            let mut bad = serde_json::to_value(&original).unwrap();
            bad[key] = value;
            assert!(Session::decode(&serde_json::to_vec(&bad).unwrap()).is_none());
        }
        let mut bad = original;
        bad.panes[0].settings.pop();
        assert!(!bad.validate());
    }

    #[test]
    fn component_browser_width_defaults_for_old_sessions_and_stays_out_of_projects() {
        let mut value = serde_json::to_value(session()).unwrap();
        value.as_object_mut().unwrap().remove("library_width");
        let mut state = Session::decode(&serde_json::to_vec(&value).unwrap()).unwrap();
        assert_eq!(state.library_width, default_library_width());
        state.library_width = 380.0;
        let restored = Session::decode(&serde_json::to_vec(&state).unwrap()).unwrap();
        assert_eq!(restored.library_width, 380.0);
        let project = crate::project::Project::capture(state.clone());
        assert!(
            !serde_json::to_string(&project)
                .unwrap()
                .contains("library_width")
        );
        for width in [0.0, 2000.0, f32::NAN] {
            state.library_width = width;
            assert!(!state.validate());
        }
    }

    #[test]
    fn render_rate_is_local_and_old_sessions_default_to_sixty() {
        let mut value = serde_json::to_value(session()).unwrap();
        value.as_object_mut().unwrap().remove("render_fps");
        let mut state = Session::decode(&serde_json::to_vec(&value).unwrap()).unwrap();
        assert_eq!(state.render_fps, 60);
        state.render_fps = 120;
        let restored = Session::decode(&serde_json::to_vec(&state).unwrap()).unwrap();
        assert_eq!(restored.render_fps, 120);
        let scene = crate::project::SceneState::capture(state);
        assert!(
            serde_json::to_value(scene)
                .unwrap()
                .get("render_fps")
                .is_none()
        );
        value["render_fps"] = 0.into();
        assert!(Session::decode(&serde_json::to_vec(&value).unwrap()).is_none());
    }
    #[test]
    fn old_sessions_default_appearance_and_new_preferences_roundtrip() {
        let mut value = serde_json::to_value(session()).unwrap();
        value.as_object_mut().unwrap().remove("appearance");
        let restored = Session::decode(&serde_json::to_vec(&value).unwrap()).unwrap();
        assert_eq!(
            restored.appearance,
            crate::theme::AppearancePreferences::default()
        );
        let mut state = restored;
        state.appearance.theme = crate::theme::ThemeChoice::Midnight;
        state.appearance.ui_scale = Some(1.3);
        let restored = Session::decode(&serde_json::to_vec(&state).unwrap()).unwrap();
        assert_eq!(restored.appearance, state.appearance);
        let mut value = serde_json::to_value(state).unwrap();
        value["appearance"]["theme"] = "future_skin".into();
        assert_eq!(
            Session::decode(&serde_json::to_vec(&value).unwrap())
                .unwrap()
                .appearance
                .theme,
            crate::theme::ThemeChoice::Automatic
        );
        value["appearance"]["ui_scale"] = 10.into();
        assert!(Session::decode(&serde_json::to_vec(&value).unwrap()).is_none());
    }

    #[test]
    #[ignore = "manual recovery checkpoint timing; SYMPHOS_PERF_WORKSPACE selects a read-only fixture"]
    fn recovery_checkpoint_timing() {
        let path = std::env::var("SYMPHOS_PERF_WORKSPACE").expect("workspace fixture path");
        let session: Session = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let start = Instant::now();
        for _ in 0..10 {
            std::hint::black_box(session.clone());
        }
        eprintln!(
            "session clone: {:.2} ms",
            start.elapsed().as_secs_f64() * 100.0
        );
        let start = Instant::now();
        for _ in 0..10 {
            assert!(session.validate());
        }
        eprintln!(
            "session validation: {:.2} ms",
            start.elapsed().as_secs_f64() * 100.0
        );
        let start = Instant::now();
        for _ in 0..10 {
            std::hint::black_box(serde_json::to_vec_pretty(&session).unwrap());
        }
        eprintln!(
            "session serialization: {:.2} ms",
            start.elapsed().as_secs_f64() * 100.0
        );
        let dir =
            std::env::temp_dir().join(format!("symphos-recovery-timing-{}", std::process::id()));
        let mut storage = Storage::new(dir.clone());
        let mut foreground = Duration::ZERO;
        for _ in 0..10 {
            let start = Instant::now();
            storage.checkpoint_async(session.clone());
            foreground += start.elapsed();
            storage.finish_checkpoint();
            assert!(storage.notice.is_none());
        }
        eprintln!(
            "foreground clone + background handoff: {:.2} ms",
            foreground.as_secs_f64() * 100.0
        );
        drop(storage);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn background_recovery_flushes_latest_state_and_retains_last_good_save() {
        let dir = std::env::temp_dir().join(format!(
            "symphos-recovery-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut storage = Storage::new(dir.clone());
        let mut state = session();
        state.sidebar_width = 280.0;
        storage.checkpoint_async(state.clone());
        storage.last_save = Instant::now() - Duration::from_secs(1);
        assert!(
            !storage.save_due(),
            "do not build a backlog of recovery snapshots"
        );
        state.sidebar_width = 410.0;
        storage.checkpoint(&state); // closing flushes older work before the final save
        assert!(storage.notice.is_none());
        assert_eq!(Session::load(&dir).unwrap().sidebar_width, 410.0);
        state.sidebar_width = f32::NAN;
        storage.checkpoint(&state);
        assert!(
            storage
                .notice
                .as_deref()
                .unwrap()
                .contains("Invalid workspace")
        );
        assert_eq!(Session::load(&dir).unwrap().sidebar_width, 410.0);
        storage.notice = None;
        state.sidebar_width = 320.0;
        storage.checkpoint_async(state.clone());
        drop(storage); // outstanding recovery is joined even without explicit flush
        assert_eq!(Session::load(&dir).unwrap().sidebar_width, 320.0);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn linked_scene_fingerprints_survive_recovery_without_bypassing_parameter_bounds() {
        let dir =
            std::env::temp_dir().join(format!("symphos-linked-recovery-{}", std::process::id()));
        let mut state = session();
        let mut book = crate::scenes::Book::new(crate::project::SceneState::capture(state.clone()));
        book.current_mut().unwrap().preset = Some(crate::scenes::PresetLink {
            id: 1,
            baseline: u64::MAX,
            edited: false,
        });
        state.scenes = Some(book);
        assert!(
            state.validate(),
            "preset fingerprints are metadata, not numeric parameters"
        );
        let mut legacy = serde_json::to_value(&state).unwrap();
        assert_eq!(
            legacy["scenes"]["scenes"][0]["preset"]["baseline"],
            "ffffffffffffffff"
        );
        legacy["scenes"]["scenes"][0]["preset"]["baseline"] = u64::MAX.into();
        let legacy = Session::decode(&serde_json::to_vec(&legacy).unwrap())
            .expect("old numeric fingerprints remain supported");
        assert_eq!(
            legacy
                .scenes
                .unwrap()
                .current()
                .unwrap()
                .preset
                .as_ref()
                .unwrap()
                .baseline,
            u64::MAX
        );
        let mut storage = Storage::new(dir.clone());
        storage.checkpoint(&state);
        assert!(storage.notice.is_none());
        let reloaded = Session::load(&dir).expect("linked preset must survive a recovery reload");
        assert_eq!(
            reloaded
                .scenes
                .unwrap()
                .current()
                .unwrap()
                .preset
                .as_ref()
                .unwrap()
                .baseline,
            u64::MAX
        );
        let saved = std::fs::read(dir.join("workspace.json")).unwrap();
        // Invalid controls must still be rejected, retaining the good checkpoint.
        let mut settings = serde_json::to_value(&state.panes[0].settings).unwrap();
        settings[0]["Waterfall"]["detail"] = 300.into();
        state.panes[0].settings = serde_json::from_value(settings).unwrap();
        assert!(!state.validate());
        storage.checkpoint(&state);
        assert!(
            storage
                .notice
                .as_ref()
                .is_some_and(|notice| notice.contains("Invalid workspace"))
        );
        assert_eq!(std::fs::read(dir.join("workspace.json")).unwrap(), saved);
        drop(storage);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn background_recovery_reports_write_errors_and_can_retry() {
        let dir =
            std::env::temp_dir().join(format!("symphos-recovery-failure-{}", std::process::id()));
        std::fs::write(&dir, b"a file cannot be a recovery directory").unwrap();
        let mut storage = Storage::new(dir.clone());
        let state = session();
        storage.checkpoint(&state);
        assert!(
            storage
                .notice
                .as_deref()
                .unwrap()
                .contains("Workspace save failed")
        );
        std::fs::remove_file(&dir).unwrap();
        storage.notice = None;
        storage.checkpoint(&state);
        assert!(storage.notice.is_none());
        assert!(Session::load(&dir).is_some());
        drop(storage);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn user_presets_and_last_view_survive_storage_reload() {
        let dir = std::env::temp_dir().join(format!(
            "symphos-workspace-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut storage = Storage::new(dir.clone());
        storage.presets.push(Preset {
            audio_inputs: Vec::new(),
            signals: Vec::new(),
            id: "user:test".into(),
            name: "My layout".into(),
            content: PresetContent::Layout(Tile::Pane(0)),
        });
        storage.save_library();
        assert!(storage.notice.is_none());
        let mut state = session();
        state.layout = Some(Tile::Pane(0));
        state.active_preset = "user:test".into();
        storage.checkpoint(&state);
        assert!(storage.notice.is_none());
        assert_eq!(
            Storage::new(dir.clone()).presets.last().unwrap().name,
            "My layout"
        );
        let restored = Session::load(&dir).unwrap();
        assert_eq!(restored.layout, state.layout);
        assert_eq!(restored.active_preset, state.active_preset);
        std::fs::write(dir.join("presets.json"), b"invalid").unwrap();
        let recovered = Storage::new(dir.clone());
        assert!(recovered.notice.is_some());
        assert!(dir.join("presets.rejected.json").exists());
        assert_eq!(recovered.presets.len(), factory_presets().len());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
