// Palette access stays local to each renderer; app preferences are applied only
// when a component is added, never while restoring settings or rendering.
macro_rules! module_palette {
    ($module:ident, $($field:ident).+) => {
        impl $module {
            pub(super) fn set_palette(&mut self, palette: crate::colors::Palette) {
                self.$($field).+ = palette;
            }
        }
    };
}

// Explicit settings-only snapshots: no samples, histories, textures, or clocks.
// Clone is deliberate so this schema can grow owned preset fields later.
macro_rules! module_settings {
    ($module:ident, $settings:ident, { $($(#[$meta:meta])* $field:ident : $ty:ty => $($path:ident).+),* $(,)? }) => {
        #[derive(serde::Serialize, serde::Deserialize)]
        #[derive(Clone, Debug, PartialEq)]
        pub(crate) struct $settings { $( $(#[$meta])* $field: $ty, )* }
        impl $module {
            #[allow(clippy::clone_on_copy)]
            pub(crate) fn settings_snapshot(&self) -> $settings {
                $settings { $( $field: self.$($path).+.clone(), )* }
            }
            #[allow(clippy::clone_on_copy)]
            pub(crate) fn apply_settings(&mut self, settings: &$settings) {
                $( self.$($path).+ = settings.$field.clone(); )*
            }
        }
    };
}

mod camera;
mod camera_gizmo;
mod camera_motion;
mod cymatics;
mod flux;
mod frequency;
#[cfg(test)]
mod inspector_tests;
mod milkdrop;
mod orbital;
mod oscilloscope;
mod particle_waves;
pub(crate) mod performance;
mod plot_grid;
mod resonance;
mod retro;
mod rings;
mod trackball;

fn new_instance_id() -> String {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    format!(
        "pane-{}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    )
}

#[cfg(test)]
mod reactive;
mod spectral_surface;
mod spectrogram;
mod spectrum;
mod stereometer;
mod strata;
mod veroni;
mod waterfall;
mod waveform;

use std::time::{Duration, Instant};

use eframe::egui::{self, Color32, FontId, Pos2, Rect};

pub(crate) use crate::colors::Palette;
use crate::help::HoverHelp;
use crate::icons::Icon;
use crate::{analysis::AnalysisFrame, theme::AppTheme};
use cymatics::Cymatics;
use flux::Flux;
use milkdrop::Milkdrop;
use orbital::Orbital;
use oscilloscope::Oscilloscope;
use particle_waves::ParticleWaves;
use resonance::Resonance;
use rings::Rings;
use spectral_surface::SpectralSurface;
use spectrogram::Spectrogram;
use spectrum::Spectrum;
use stereometer::Stereometer;
use strata::Strata;
use veroni::Veroni;
use waterfall::Waterfall;
use waveform::Waveform;

#[derive(Clone, Copy)]
struct ModuleRenderState {
    live: bool,
    frozen: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ModuleKind {
    Waterfall,
    Spectrum,
    Waveform,
    Spectrogram,
    Stereometer,
    Oscilloscope,
    Veroni,
    Strata,
    Flux,
    Sphere,
    Cylinder,
    Rings,
    Resonance,
    Orbital,
    Milkdrop,
    Cymatics,
    ParticleWaves,
    SpectralBloom,
}

impl ModuleKind {
    pub const ALL: [Self; 18] = [
        Self::Waterfall,
        Self::Spectrum,
        Self::Waveform,
        Self::Spectrogram,
        Self::Stereometer,
        Self::Oscilloscope,
        Self::Veroni,
        Self::Strata,
        Self::Flux,
        Self::Sphere,
        Self::Cylinder,
        Self::Rings,
        Self::Resonance,
        Self::Orbital,
        Self::Milkdrop,
        Self::Cymatics,
        Self::ParticleWaves,
        Self::SpectralBloom,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::SpectralBloom => "Spectral Bloom",
            Self::ParticleWaves => "Particle Waves",
            Self::Cymatics => "Cymatics",
            Self::Milkdrop => "MilkDrop",
            Self::Orbital => "Orbital",
            Self::Resonance => "Lotus Flower",
            Self::Rings => "3D Rings",
            Self::Cylinder => "3D Cylinder",
            Self::Sphere => "3D Sphere",
            Self::Waterfall => "3D Waterfall",
            Self::Spectrum => "Frequency spectrum",
            Self::Waveform => "Waveform",
            Self::Spectrogram => "Spectrogram",
            Self::Stereometer => "Stereometer",
            Self::Oscilloscope => "Oscilloscope",
            Self::Veroni => "Veroni",
            Self::Strata => "Strata",
            Self::Flux => "Flux",
        }
    }

    pub fn icon(self) -> Icon {
        match self {
            Self::SpectralBloom => Icon::Bars,
            Self::ParticleWaves => Icon::Strata,
            Self::Cymatics => Icon::Resonance,
            Self::Milkdrop => Icon::Flux,
            Self::Orbital => Icon::Flux,
            Self::Resonance => Icon::Resonance,
            Self::Rings => Icon::Rings,
            Self::Cylinder => Icon::Cylinder,
            Self::Sphere => Icon::Sphere,
            Self::Waterfall => Icon::Waterfall,
            Self::Spectrum => Icon::Bars,
            Self::Waveform => Icon::Waveform,
            Self::Spectrogram => Icon::Spectrogram,
            Self::Stereometer => Icon::Stereo,
            Self::Oscilloscope => Icon::Oscilloscope,
            Self::Veroni => Icon::Veroni,
            Self::Strata => Icon::Strata,
            Self::Flux => Icon::Flux,
        }
    }

    pub fn subtitle(self) -> &'static str {
        match self {
            Self::SpectralBloom => "Sculptural spectrum, strands and feedback",
            Self::ParticleWaves => "Layered spectral particle ribbons",
            Self::Cymatics => "Audio-driven vibrating plates",
            Self::Milkdrop => "MilkDrop-compatible presets",
            Self::Orbital => "GPU orbital flocking",
            Self::Resonance => "Spectrum-driven petal layers",
            Self::Rings => "Orbital waveform rings",
            Self::Cylinder => "Cylindrical frequency history",
            Self::Sphere => "Spherical frequency history",
            Self::Waterfall => "3D frequency history",
            Self::Spectrum => "Frequency levels",
            Self::Waveform => "Audio over time",
            Self::Spectrogram => "Frequency over time",
            Self::Stereometer => "Stereo image & phase",
            Self::Oscilloscope => "Signal shape & cycle",
            Self::Strata => "Audio-reactive layered sculpture",
            Self::Flux => "3D particle currents",
            Self::Veroni => "Audio-reactive cellular flow",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::SpectralBloom => {
                "Measured spectrum columns bend into luminous strands and dissolve into particles. Transform lingering light with image feedback; tune shape, color, motion and the 3D camera in the inspector."
            }
            Self::ParticleWaves => {
                "Measured frequency spectra form layered ribbons, rings, spirals and tunnels of light. Audio scatters particles away from the surface and springs draw them back. Shape individual layers, bokeh, colors and glow; drag to rotate freely in 3D."
            }
            Self::Cymatics => {
                "Audio-driven Chladni plates: naturally settling GPU sand, shaded wave surfaces, nodal patterns and artistic interference. Explore plate dimensions, materials and boundaries; drag to rotate freely. Thin-plate modes with approximate grain transport."
            }
            Self::Milkdrop => {
                "Import, edit and perform MilkDrop presets with native audio response, feedback, waves, shapes and shaders. Drag inspector adjustments to scene macros. Powered by projectM."
            }
            Self::Orbital => {
                "GPU particles flock around moving attractor orbs. Shape orbital forces, collective motion, lighting, bloom and trails. Drag to rotate freely; Shift/right-drag pans. Parameters can be assigned to scene macros and driven by Signals."
            }
            Self::Resonance => {
                "Translucent lotus petals driven by the selected audio spectrum. Map bass to treble across layers, from pole to pole, or around petals; add spectral movement, attack accents and frequency-driven light. The resting flower remains an artistic form, not a calibrated spectrum plot. Drag to rotate; Shift/right-drag to pan."
            }
            Self::Rings => {
                "Live waveforms flow around configurable orbital, concentric, or stacked rings. Choose stereo or Mid/Side input, shape the audio displacement, and rotate freely in 3D."
            }
            Self::Cylinder => {
                "Frequency wraps around the cylinder; history flows down its length; level displaces the radius. Shape the rims, launch axial ripples, and drag to rotate freely."
            }
            Self::Sphere => {
                "Frequency wraps around the sphere; history flows from pole to pole; level displaces the surface. Drag to orbit, Shift-drag to pan, scroll to zoom."
            }
            Self::Waterfall => {
                "A 3D frequency history: X is frequency, Y is time, and Z is signal level. Drag to orbit; right-drag or Shift-drag to pan."
            }
            Self::Spectrum => {
                "Current signal level by frequency. Hover over a band to inspect its frequency and level."
            }
            Self::Waveform => {
                "Audio amplitude over time. Choose one or two independent Left, Right, Mid, or Side lanes, then scroll the signal or overwrite a static loop."
            }
            Self::Spectrogram => {
                "Frequency over time, with color showing signal level. Newest audio appears on the right or bottom, depending on orientation."
            }
            Self::Stereometer => {
                "Stereo width, balance, and phase correlation. The vectorscope plots the relationship between the left and right channels."
            }
            Self::Flux => {
                "A dancing 3D particle cloud pulled into colorful currents by moving attraction points. Bass expands the cloud, mids drive swirl, and highs add scatter. Drag to orbit; Shift-drag to roll; scroll to zoom."
            }
            Self::Strata => {
                "Suspended wire membranes, drifting dust and tumbling grains. Bass bends the layers, mids drive ripples, and highs illuminate the particles. Drag to orbit; scroll to zoom; double-click to reset the view."
            }
            Self::Veroni => {
                "A flowing Voronoi mesh with luminous sites. Bass expands the cells, mids bend the flow, and highs animate fine detail. Open the inspector to shape its motion and appearance."
            }
            Self::Oscilloscope => {
                "A short, stabilized amplitude view for inspecting the repeating cycle, symmetry, and shape of a sound."
            }
        }
    }
}

pub struct ModulePane {
    /// Runtime copy of the owning scene background, also used by recording.
    pub background: crate::background::Background,
    pub audio_input: crate::signals::audio::Connection,
    input_revision: Option<(crate::signals::audio::Connection, u64, u64)>,
    silent_input: Option<AnalysisFrame>,
    instance_id: String,
    pub(crate) macros: crate::macros::State,
    scene_macros: Option<crate::macros::State>,
    pub kind: ModuleKind,
    waterfall: Waterfall,
    sphere: SpectralSurface,
    cylinder: SpectralSurface,
    rings: Rings,
    resonance: Resonance,
    orbital: Orbital,
    milkdrop: Milkdrop,
    cymatics: Cymatics,
    particle_waves: ParticleWaves,
    spectral_bloom: ParticleWaves,
    spectrum: Spectrum,
    waveform: Waveform,
    spectrogram: Spectrogram,
    stereometer: Stereometer,
    oscilloscope: Oscilloscope,
    veroni: Veroni,
    strata: Strata,
    flux: Flux,
    active_sections: [usize; 18],
    camera_runtime: [camera::Runtime; 18],
    inspector_focus: Option<SettingsSection>,
    frozen: Option<FrozenFrame>,
    paused_duration: Duration,
    pub last_capture: u64,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ModuleSettings {
    Waterfall(waterfall::WaterfallSettings),
    Sphere(spectral_surface::SurfaceSettings),
    Cylinder(spectral_surface::SurfaceSettings),
    Rings(rings::RingsSettings),
    Resonance(resonance::ResonanceSettings),
    Orbital(Box<orbital::OrbitalSettings>),
    Milkdrop(Box<milkdrop::MilkdropSettings>),
    Cymatics(Box<cymatics::Settings>),
    ParticleWaves(Box<particle_waves::Settings>),
    SpectralBloom(Box<particle_waves::Settings>),
    Spectrum(spectrum::SpectrumSettings),
    Waveform(waveform::WaveformSettings),
    Spectrogram(spectrogram::SpectrogramSettings),
    Stereometer(stereometer::StereometerSettings),
    Oscilloscope(oscilloscope::OscilloscopeSettings),
    Veroni(veroni::VeroniSettings),
    Strata(strata::StrataSettings),
    Flux(flux::FluxSettings),
}

impl ModuleSettings {
    /// Modulation edits numeric parameters, never portable binary assets. Keep
    /// large image payloads out of the per-frame JSON parameter round-trip.
    pub(crate) fn parameter_document(&self) -> Result<serde_json::Value, serde_json::Error> {
        if let Self::Waterfall(settings) = self {
            serde_json::to_value(Self::Waterfall(settings.without_image()))
        } else {
            serde_json::to_value(self)
        }
    }

    pub(crate) fn updated_parameters(
        &self,
        document: serde_json::Value,
    ) -> Result<Self, serde_json::Error> {
        let mut updated: Self = serde_json::from_value(document)?;
        if let (Self::Waterfall(original), Self::Waterfall(settings)) = (self, &mut updated) {
            settings.restore_image(original);
        }
        Ok(updated)
    }

    pub fn modulation_mut(&mut self) -> Option<&mut crate::modulation::ModulationSettings> {
        match self {
            Self::Flux(s) => s.modulation.as_mut(),
            Self::Veroni(s) => s.modulation.as_mut(),
            Self::Strata(s) => s.modulation.as_mut(),
            _ => None,
        }
    }

    pub fn kind(&self) -> ModuleKind {
        match self {
            Self::Orbital(_) => ModuleKind::Orbital,
            Self::Milkdrop(_) => ModuleKind::Milkdrop,
            Self::Cymatics(_) => ModuleKind::Cymatics,
            Self::ParticleWaves(_) => ModuleKind::ParticleWaves,
            Self::SpectralBloom(_) => ModuleKind::SpectralBloom,
            Self::Resonance(_) => ModuleKind::Resonance,
            Self::Rings(_) => ModuleKind::Rings,
            Self::Cylinder(_) => ModuleKind::Cylinder,
            Self::Sphere(_) => ModuleKind::Sphere,
            Self::Waterfall(_) => ModuleKind::Waterfall,
            Self::Spectrum(_) => ModuleKind::Spectrum,
            Self::Waveform(_) => ModuleKind::Waveform,
            Self::Spectrogram(_) => ModuleKind::Spectrogram,
            Self::Stereometer(_) => ModuleKind::Stereometer,
            Self::Oscilloscope(_) => ModuleKind::Oscilloscope,
            Self::Veroni(_) => ModuleKind::Veroni,
            Self::Strata(_) => ModuleKind::Strata,
            Self::Flux(_) => ModuleKind::Flux,
        }
    }
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PaneState {
    #[serde(default)]
    pub audio_input: crate::signals::audio::Connection,
    #[serde(default)]
    pub macros: Option<crate::macros::State>,
    #[serde(default = "new_instance_id")]
    pub instance_id: String,
    pub kind: ModuleKind,
    #[serde(deserialize_with = "decode_sections")]
    pub sections: [usize; 18],
    #[serde(default)]
    pub orbital_inspector_version: u8,
    #[serde(default)]
    pub milkdrop_presets: Vec<std::sync::Arc<crate::milkdrop::preset::Preset>>,
    #[serde(deserialize_with = "decode_module_settings")]
    pub settings: Vec<ModuleSettings>,
}

// Upgrade workspaces and view presets from earlier six- through thirteen-component schemas.
// Only complete known legacy sets are upgraded; malformed sets still fail validation.
fn decode_sections<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<[usize; 18], D::Error> {
    let values = <Vec<usize> as serde::Deserialize>::deserialize(deserializer)?;
    if !(6..=18).contains(&values.len()) {
        return Err(serde::de::Error::custom(
            "expected six to eighteen module sections",
        ));
    }
    let mut sections = [0; 18];
    sections[..values.len()].copy_from_slice(&values);
    Ok(sections)
}
fn decode_module_settings<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<ModuleSettings>, D::Error> {
    let mut settings = <Vec<ModuleSettings> as serde::Deserialize>::deserialize(deserializer)?;
    let legacy_len = settings.len();
    if (6..=17).contains(&legacy_len)
        && ModuleKind::ALL[..legacy_len].iter().all(|kind| {
            settings
                .iter()
                .filter(|entry| entry.kind() == *kind)
                .count()
                == 1
        })
    {
        if legacy_len == 6 {
            settings.push(ModuleSettings::Veroni(
                Veroni::default().settings_snapshot(),
            ));
        }
        if legacy_len <= 7 {
            settings.push(ModuleSettings::Strata(
                Strata::default().settings_snapshot(),
            ));
        }
        if legacy_len <= 8 {
            settings.push(ModuleSettings::Flux(Flux::default().settings_snapshot()));
        }
        if legacy_len <= 9 {
            settings.push(ModuleSettings::Sphere(
                SpectralSurface::default().settings_snapshot(),
            ));
        }
        if legacy_len <= 10 {
            settings.push(ModuleSettings::Cylinder(
                SpectralSurface::cylinder().settings_snapshot(),
            ));
        }
        if legacy_len <= 11 {
            settings.push(ModuleSettings::Rings(Rings::default().settings_snapshot()));
        }
        if legacy_len <= 12 {
            settings.push(ModuleSettings::Resonance(
                Resonance::default().settings_snapshot(),
            ));
        }
        if legacy_len <= 13 {
            settings.push(ModuleSettings::Orbital(Box::new(
                Orbital::default().settings_snapshot(),
            )));
        }
        if legacy_len <= 14 {
            settings.push(ModuleSettings::Milkdrop(Box::default()));
        }
        if legacy_len <= 15 {
            settings.push(ModuleSettings::Cymatics(Box::default()));
        }
        if legacy_len <= 16 {
            settings.push(ModuleSettings::ParticleWaves(Box::default()));
        }
        settings.push(ModuleSettings::SpectralBloom(Box::new(
            particle_waves::Settings::spectral_bloom(),
        )));
    }
    Ok(settings)
}

struct FrozenFrame {
    frame: AnalysisFrame,
    started: Instant,
    time: Instant,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsSection {
    Camera,
    History,
    Geometry,
    Frequency,
    Response,
    Modulation,
    Appearance,
    Channels,
    TimeWindow,
    Amplitude,
    Display,
    Correlation,
    Sync,
    Flow,
    Particles,
    Orbs,
    Rendering,
    Presets,
    Feedback,
}

impl SettingsSection {
    pub fn title(self) -> &'static str {
        match self {
            Self::Camera => "Camera",
            Self::History => "Time & History",
            Self::Geometry => "Geometry",
            Self::Frequency => "Frequency Range",
            Self::Response => "Signal Response",
            Self::Modulation => "Audio mappings",
            Self::Appearance => "Appearance",
            Self::Channels => "Channels",
            Self::TimeWindow => "Time Window",
            Self::Amplitude => "Amplitude",
            Self::Display => "Display",
            Self::Correlation => "Correlation",
            Self::Sync => "Synchronization",
            Self::Flow => "Flow & Motion",
            Self::Particles => "Particles",
            Self::Orbs => "Orbs",
            Self::Rendering => "Rendering",
            Self::Presets => "Presets",
            Self::Feedback => "Feedback",
        }
    }
}

impl ModulePane {
    /// Source lifecycle is owned by Signals. Reconcile every retained visualizer,
    /// including inactive types, without creating or rebinding any channels.
    pub fn reconcile_signals(&mut self, bank: &crate::signals::Bank) {
        if let crate::signals::audio::Connection::Input(id) = self.audio_input
            && !bank.audio_inputs.iter().any(|b| b.id == id)
        {
            self.audio_input = crate::signals::audio::Connection::Off;
        }
        self.import_legacy_macros();
        self.macros.reconcile(bank);
        for settings in [
            &mut self.flux.settings.modulation,
            &mut self.veroni.settings.modulation,
            &mut self.strata.settings.modulation,
        ]
        .into_iter()
        .flatten()
        {
            settings.routes.retain(|r| {
                r.signal
                    .is_none_or(|id| bank.channels.iter().any(|d| d.id == id))
            });
            for r in &mut settings.routes {
                if r.signal.is_none() && !(r.target.layered() && r.layer == 0) {
                    r.enabled = false;
                }
            }
        }
    }

    pub fn export_settings(&self) -> ModuleSettings {
        if self.kind == ModuleKind::Milkdrop {
            ModuleSettings::Milkdrop(Box::new(self.milkdrop.export_settings()))
        } else {
            self.copy_settings()
        }
    }
    pub fn copy_settings(&self) -> ModuleSettings {
        match self.kind {
            ModuleKind::Veroni => ModuleSettings::Veroni(self.veroni.settings_snapshot()),
            ModuleKind::Strata => ModuleSettings::Strata(self.strata.settings_snapshot()),
            ModuleKind::Flux => ModuleSettings::Flux(self.flux.settings_snapshot()),
            ModuleKind::Milkdrop => {
                ModuleSettings::Milkdrop(Box::new(self.milkdrop.settings_snapshot()))
            }
            ModuleKind::SpectralBloom => {
                ModuleSettings::SpectralBloom(Box::new(self.spectral_bloom.settings_snapshot()))
            }
            ModuleKind::ParticleWaves => {
                ModuleSettings::ParticleWaves(Box::new(self.particle_waves.settings_snapshot()))
            }
            ModuleKind::Cymatics => {
                ModuleSettings::Cymatics(Box::new(self.cymatics.settings_snapshot()))
            }
            ModuleKind::Orbital => {
                ModuleSettings::Orbital(Box::new(self.orbital.settings_snapshot()))
            }
            ModuleKind::Resonance => ModuleSettings::Resonance(self.resonance.settings_snapshot()),
            ModuleKind::Rings => ModuleSettings::Rings(self.rings.settings_snapshot()),
            ModuleKind::Cylinder => ModuleSettings::Cylinder(self.cylinder.settings_snapshot()),
            ModuleKind::Sphere => ModuleSettings::Sphere(self.sphere.settings_snapshot()),
            ModuleKind::Waterfall => ModuleSettings::Waterfall(self.waterfall.settings_snapshot()),
            ModuleKind::Spectrum => ModuleSettings::Spectrum(self.spectrum.settings_snapshot()),
            ModuleKind::Waveform => ModuleSettings::Waveform(self.waveform.settings_snapshot()),
            ModuleKind::Spectrogram => {
                ModuleSettings::Spectrogram(self.spectrogram.settings_snapshot())
            }
            ModuleKind::Stereometer => {
                ModuleSettings::Stereometer(self.stereometer.settings_snapshot())
            }
            ModuleKind::Oscilloscope => {
                ModuleSettings::Oscilloscope(self.oscilloscope.settings_snapshot())
            }
        }
    }

    pub fn paste_settings(&mut self, settings: &ModuleSettings) -> bool {
        if settings.kind() != self.kind {
            return false;
        }
        match settings {
            ModuleSettings::Veroni(settings) => self.veroni.apply_settings(settings),
            ModuleSettings::Strata(settings) => self.strata.apply_settings(settings),
            ModuleSettings::Flux(settings) => self.flux.apply_settings(settings),
            ModuleSettings::Cymatics(settings) => self.cymatics.apply_settings(settings),
            ModuleSettings::SpectralBloom(settings) => self.spectral_bloom.apply_settings(settings),
            ModuleSettings::ParticleWaves(settings) => self.particle_waves.apply_settings(settings),
            ModuleSettings::Orbital(settings) => self.orbital.apply_settings(settings),
            ModuleSettings::Milkdrop(settings) => self.milkdrop.apply_settings(settings),
            ModuleSettings::Resonance(settings) => self.resonance.apply_settings(settings),
            ModuleSettings::Rings(settings) => self.rings.apply_settings(settings),
            ModuleSettings::Cylinder(settings) => self.cylinder.apply_settings(settings),
            ModuleSettings::Sphere(settings) => self.sphere.apply_settings(settings),
            ModuleSettings::Waterfall(settings) => self.waterfall.apply_settings(settings),
            ModuleSettings::Spectrum(settings) => self.spectrum.apply_settings(settings),
            ModuleSettings::Waveform(settings) => self.waveform.apply_settings(settings),
            ModuleSettings::Spectrogram(settings) => self.spectrogram.apply_settings(settings),
            ModuleSettings::Stereometer(settings) => self.stereometer.apply_settings(settings),
            ModuleSettings::Oscilloscope(settings) => self.oscilloscope.apply_settings(settings),
        }
        true
    }

    pub fn snapshot(&self) -> PaneState {
        PaneState {
            audio_input: self.audio_input,
            instance_id: self.instance_id.clone(),
            macros: Some(self.macros.clone()),
            kind: self.kind,
            sections: self.active_sections,
            orbital_inspector_version: 1,
            milkdrop_presets: self.milkdrop.presets.clone(),
            settings: vec![
                ModuleSettings::Waterfall(self.waterfall.settings_snapshot()),
                ModuleSettings::Spectrum(self.spectrum.settings_snapshot()),
                ModuleSettings::Waveform(self.waveform.settings_snapshot()),
                ModuleSettings::Spectrogram(self.spectrogram.settings_snapshot()),
                ModuleSettings::Stereometer(self.stereometer.settings_snapshot()),
                ModuleSettings::Oscilloscope(self.oscilloscope.settings_snapshot()),
                ModuleSettings::Veroni(self.veroni.settings_snapshot()),
                ModuleSettings::Strata(self.strata.settings_snapshot()),
                ModuleSettings::Flux(self.flux.settings_snapshot()),
                ModuleSettings::Sphere(self.sphere.settings_snapshot()),
                ModuleSettings::Cylinder(self.cylinder.settings_snapshot()),
                ModuleSettings::Rings(self.rings.settings_snapshot()),
                ModuleSettings::Resonance(self.resonance.settings_snapshot()),
                ModuleSettings::Orbital(Box::new(self.orbital.settings_snapshot())),
                ModuleSettings::Milkdrop(Box::new(self.milkdrop.settings_snapshot())),
                ModuleSettings::Cymatics(Box::new(self.cymatics.settings_snapshot())),
                ModuleSettings::ParticleWaves(Box::new(self.particle_waves.settings_snapshot())),
                ModuleSettings::SpectralBloom(Box::new(self.spectral_bloom.settings_snapshot())),
            ],
        }
    }

    /// Restore an edit while preserving simulation/capture state and inspector navigation.
    pub fn restore_edit(&mut self, state: &PaneState) {
        if self.audio_input != state.audio_input {
            self.audio_input = state.audio_input;
            self.input_revision = None;
            self.clear_input_history();
        }
        let before = self.snapshot();
        for saved in &state.settings {
            let settings = saved.clone();
            if before.settings.iter().any(|s| s == &settings) {
                continue;
            }
            self.kind = settings.kind();
            self.paste_settings(&settings);
        }
        self.kind = state.kind;
        self.macros = state.macros.clone().unwrap_or_default();
        self.milkdrop.presets = state.milkdrop_presets.clone();
    }

    pub fn restore(&mut self, state: &PaneState) {
        self.camera_runtime = std::array::from_fn(|_| Default::default());
        self.audio_input = state.audio_input;
        self.input_revision = None;
        self.instance_id = state.instance_id.clone();
        self.milkdrop.presets = if state.milkdrop_presets.is_empty() {
            crate::milkdrop::preset::builtins()
                .into_iter()
                .map(std::sync::Arc::new)
                .collect()
        } else {
            state.milkdrop_presets.clone()
        };
        self.macros = state.macros.clone().unwrap_or_default();
        for settings in &state.settings {
            self.kind = settings.kind();
            self.paste_settings(settings);
        }
        for (index, kind) in ModuleKind::ALL.into_iter().enumerate() {
            self.kind = kind;
            self.active_sections[index] =
                if kind == ModuleKind::Orbital && state.orbital_inspector_version == 0 {
                    orbital::inspector::legacy_section(state.sections[index])
                } else {
                    state.sections[index]
                }
                .min(self.sections().len() - 1);
        }
        self.kind = state.kind;
        self.reset();
    }

    pub(crate) fn new_with_palette(kind: ModuleKind, palette: Palette) -> Self {
        let mut pane = Self::new(kind);
        pane.set_initial_palette(palette);
        pane
    }

    pub(crate) fn set_initial_palette(&mut self, palette: Palette) {
        match self.kind {
            ModuleKind::Waterfall => self.waterfall.set_palette(palette),
            ModuleKind::Sphere => self.sphere.set_palette(palette),
            ModuleKind::Cylinder => self.cylinder.set_palette(palette),
            ModuleKind::Rings => self.rings.set_palette(palette),
            ModuleKind::Resonance => self.resonance.set_palette(palette),
            ModuleKind::Spectrum => self.spectrum.set_palette(palette),
            ModuleKind::Waveform => self.waveform.set_palette(palette),
            ModuleKind::Spectrogram => self.spectrogram.set_palette(palette),
            ModuleKind::Stereometer => self.stereometer.set_palette(palette),
            ModuleKind::Oscilloscope => self.oscilloscope.set_palette(palette),
            ModuleKind::Veroni => self.veroni.set_palette(palette),
            ModuleKind::Strata => self.strata.set_palette(palette),
            ModuleKind::Flux => self.flux.set_palette(palette),
            // Preset shaders and sketches with material/particle color systems
            // keep their own color controls and authored defaults.
            _ => {}
        }
    }

    pub fn new(kind: ModuleKind) -> Self {
        Self {
            background: Default::default(),
            audio_input: Default::default(),
            input_revision: None,
            silent_input: None,
            instance_id: new_instance_id(),
            macros: crate::macros::State::default(),
            scene_macros: None,
            kind,
            waterfall: Waterfall::default(),
            sphere: SpectralSurface::default(),
            cylinder: SpectralSurface::cylinder(),
            rings: Rings::default(),
            resonance: Resonance::default(),
            orbital: Orbital::default(),
            milkdrop: Milkdrop::default(),
            cymatics: Cymatics::default(),
            particle_waves: ParticleWaves::default(),
            spectral_bloom: ParticleWaves::spectral_bloom(),
            spectrum: Spectrum::default(),
            waveform: Waveform::default(),
            spectrogram: Spectrogram::default(),
            stereometer: Stereometer::default(),
            oscilloscope: Oscilloscope::default(),
            veroni: Veroni::default(),
            strata: Strata::default(),
            flux: Flux::default(),
            active_sections: [0; 18],
            camera_runtime: std::array::from_fn(|_| Default::default()),
            inspector_focus: None,
            frozen: None,
            paused_duration: Duration::ZERO,
            last_capture: 0,
        }
    }

    pub fn reset_capture(&mut self) {
        self.last_capture = 0;
        self.waterfall.clear();
        self.sphere.clear();
        self.cylinder.clear();
        self.rings.clear();
        self.resonance.clear();
        self.cymatics.clear();
        self.particle_waves.clear();
        self.spectral_bloom.clear();
        self.spectrum.clear();
        self.waveform.clear();
        self.spectrogram.clear();
        self.stereometer.clear();
        self.oscilloscope.clear();
    }
    pub fn reset(&mut self) {
        self.last_capture = 0;
        self.frozen = None;
        self.paused_duration = Duration::ZERO;
        self.waterfall.clear();
        self.sphere.clear();
        self.cylinder.clear();
        self.rings.clear();
        self.resonance.clear();
        self.cymatics.clear();
        self.particle_waves.clear();
        self.spectral_bloom.clear();
        self.spectrum.clear();
        self.waveform.clear();
        self.spectrogram.clear();
        self.stereometer.clear();
        self.oscilloscope.clear();
        self.veroni.clear();
        self.strata.clear();
        self.flux.clear();
        self.orbital.clear();
        self.milkdrop.clear();
    }

    pub fn is_frozen(&self) -> bool {
        self.frozen.is_some()
    }

    pub fn toggle_freeze(&mut self, frame: &AnalysisFrame, now: Instant) {
        if let Some(frozen) = self.frozen.take() {
            self.paused_duration += now.saturating_duration_since(frozen.started);
            self.last_capture = frame.sequence;
            self.flux.resume_signals(frame);
            self.spectrum.clear_cursor();
            self.spectrogram.clear_cursor();
        } else {
            self.frozen = Some(FrozenFrame {
                frame: frame.clone(),
                started: now,
                time: now - self.paused_duration,
            });
        }
    }

    pub(crate) fn reset_settings(&mut self) {
        match self.kind {
            ModuleKind::Cymatics => self.cymatics.reset_settings(),
            ModuleKind::ParticleWaves => self.particle_waves.reset_settings(),
            ModuleKind::SpectralBloom => self.spectral_bloom.reset_settings(),
            ModuleKind::Orbital => self.orbital.reset_settings(),
            ModuleKind::Milkdrop => self.milkdrop.reset_settings(),
            ModuleKind::Resonance => self.resonance.reset_settings(),
            ModuleKind::Rings => self.rings.reset_settings(),
            ModuleKind::Cylinder => self.cylinder.reset_settings(),
            ModuleKind::Sphere => self.sphere.reset_settings(),
            ModuleKind::Waterfall => self.waterfall.reset_settings(),
            ModuleKind::Spectrum => self.spectrum.reset_settings(),
            ModuleKind::Waveform => self.waveform.reset_settings(),
            ModuleKind::Spectrogram => self.spectrogram.reset_settings(),
            ModuleKind::Stereometer => self.stereometer.reset_settings(),
            ModuleKind::Oscilloscope => self.oscilloscope.reset_settings(),
            ModuleKind::Veroni => self.veroni.reset_settings(),
            ModuleKind::Strata => self.strata.reset_settings(),
            ModuleKind::Flux => self.flux.reset_settings(),
        }
    }

    #[cfg(test)]
    pub fn controls(&mut self, ui: &mut egui::Ui, frame: &AnalysisFrame) {
        ui.spacing_mut().slider_width = ui.spacing().slider_width.min(78.0);
        let section = self.active_section();
        let reset = ui.push_id(self.kind.label(), |ui| {
            ui.data_mut(|data| data.insert_temp(ui.id().with("active-module-section"), section));
            self.section_controls(ui, frame);
            ui.add_space(12.0);
            ui.separator();
            ui.small_button("Reset module settings")
                .help_text("Restore all settings for this module in this pane only. Retains captured history and leaves other panes, audio, and global settings unchanged.")
                .clicked()
        }).inner;
        if reset {
            self.reset_settings();
        }
    }

    pub(crate) fn import_legacy_macros(&mut self) {
        for kind in [ModuleKind::Flux, ModuleKind::Veroni, ModuleKind::Strata] {
            if self.macros.banks[kind as usize]
                .iter()
                .any(|s| s.parameter.is_some())
            {
                continue;
            }
            let legacy = match kind {
                ModuleKind::Flux => &self.flux.settings.modulation,
                ModuleKind::Veroni => &self.veroni.settings.modulation,
                _ => &self.strata.settings.modulation,
            };
            if !legacy
                .as_ref()
                .is_some_and(|m| m.routes.iter().any(|r| r.signal.is_some()))
            {
                continue;
            }
            let (controls, fields) = match kind {
                ModuleKind::Flux => (
                    Flux::legacy_macro_controls(),
                    [
                        "brightness",
                        "speed",
                        "expansion",
                        "impact",
                        "swirl",
                        "turbulence",
                        "orbit_magnetism",
                        "orbit_shape",
                        "depth",
                        "trails",
                        "glow",
                        "color_drift",
                    ],
                ),
                ModuleKind::Veroni => (
                    Veroni::macro_controls(),
                    [
                        "brightness",
                        "speed",
                        "breathing",
                        "swirl",
                        "turbulence",
                        "drift",
                        "clustering",
                        "rounding",
                        "point_size",
                        "edge_width",
                        "link_opacity",
                        "glow",
                    ],
                ),
                _ => (
                    Strata::macro_controls(),
                    [
                        "brightness",
                        "speed",
                        "sag",
                        "ripple_amount",
                        "shimmer_amount",
                        "turbulence",
                        "twist",
                        "height",
                        "lift",
                        "spread",
                        "trails",
                        "glow",
                    ],
                ),
            };
            let mut settings = match kind {
                ModuleKind::Flux => ModuleSettings::Flux(self.flux.settings_snapshot()),
                ModuleKind::Veroni => ModuleSettings::Veroni(self.veroni.settings_snapshot()),
                _ => ModuleSettings::Strata(self.strata.settings_snapshot()),
            };
            let Some(m) = settings.modulation_mut() else {
                continue;
            };
            let mut imported = Vec::new();
            for (i, c) in controls.iter().enumerate() {
                if let Some(r) = m
                    .routes
                    .iter()
                    .find(|r| r.target == c.target && r.enabled)
                    .or_else(|| m.routes.iter().find(|r| r.target == c.target))
                    .filter(|r| r.signal.is_some() && (!r.target.layered() || r.layer == 0))
                {
                    self.macros.banks[kind as usize][i] = crate::macros::Slot {
                        parameter: Some(crate::macros::ParameterSpec {
                            field: fields[i].into(),
                            label: c.label.into(),
                            min: c.min.into(),
                            max: c.max.into(),
                            safe_min: c.min.into(),
                            safe_max: c.max.into(),
                            default: c.default.into(),
                            integer: false,
                            logarithmic: false,
                            scale: 1.0,
                        }),
                        signal: r.signal,
                        depth: r.amount * m.amount,
                        enabled: r.enabled,
                        invert: r.invert,
                        output_min: r.output_min,
                        output_max: r.output_max,
                        ..Default::default()
                    };
                    imported.push(c.target);
                }
            }
            if !imported.is_empty() {
                m.routes.retain(|r| !imported.contains(&r.target));
                self.apply_live_settings(&settings);
            }
        }
    }
    pub(crate) fn instance_id(&self) -> &str {
        &self.instance_id
    }
    pub(crate) fn set_scene_macros(&mut self, macros: &crate::macros::scene::SceneMacros) {
        let mut state = crate::macros::State::default();
        for kind in ModuleKind::ALL {
            for (index, slot) in macros.slots.iter().enumerate() {
                if slot
                    .target
                    .as_ref()
                    .is_some_and(|target| target.pane == self.instance_id && target.kind == kind)
                {
                    state.banks[kind as usize][index] = slot.clone();
                }
            }
        }
        self.scene_macros = Some(state);
    }
    fn performance_state(&self) -> &crate::macros::State {
        self.scene_macros.as_ref().unwrap_or(&self.macros)
    }
    pub(crate) fn apply_macro_settings(&mut self, settings: &ModuleSettings) {
        let kind = self.kind;
        self.kind = settings.kind();
        self.apply_live_settings(settings);
        self.kind = kind;
    }
    #[cfg(all(test, feature = "development"))]
    pub(crate) fn advance_waterfall_camera(&mut self, now: Instant) -> (f32, f32) {
        self.waterfall.advance_camera(now);
        self.waterfall.camera_angles()
    }
    #[cfg(test)]
    pub fn toggle_performance(&mut self) {
        self.macros.open = !self.macros.open;
    }
    #[cfg(test)]
    pub fn performance_height(&self, _width: f32) -> Option<f32> {
        self.macros.open.then_some(220.0)
    }
    #[cfg(test)]
    pub fn performance_bar(
        &mut self,
        ui: &mut egui::Ui,
        rect: Rect,
        theme: &AppTheme,
        _index: usize,
    ) -> bool {
        let mut settings = self.copy_settings();
        let original = settings.clone();
        let (inspect, burst) = crate::macros::bar(
            ui,
            rect,
            theme,
            &self.instance_id,
            self.kind,
            &mut self.macros,
            &mut settings,
        );
        if settings != original {
            self.apply_live_settings(&settings);
        }
        if burst {
            self.flux.trigger_burst();
        }
        inspect
    }
    fn apply_live_settings(&mut self, settings: &ModuleSettings) {
        match settings {
            ModuleSettings::Flux(s) => self.flux.settings = s.clone(),
            ModuleSettings::Veroni(s) => self.veroni.settings = s.clone(),
            ModuleSettings::Strata(s) => self.strata.settings = s.clone(),
            _ => {
                self.paste_settings(settings);
            }
        }
    }
    fn section_controls(&mut self, ui: &mut egui::Ui, frame: &AnalysisFrame) {
        let indicators = crate::macros::inspector_indicators(
            ui,
            &self.instance_id,
            self.kind,
            self.performance_state(),
            &self.copy_settings(),
        );
        ui.data_mut(|d| {
            d.insert_temp(egui::Id::new(crate::macros::INDICATORS), indicators);
            d.insert_temp(
                egui::Id::new(crate::macros::CONTEXT),
                crate::macros::ParameterContext {
                    pane: self.instance_id.clone(),
                    kind: self.kind,
                },
            )
        });
        let frame = self.frozen.as_ref().map_or(frame, |frozen| &frozen.frame);
        match self.kind {
            ModuleKind::Cymatics => self.cymatics.controls(ui),
            ModuleKind::ParticleWaves => self.particle_waves.controls(ui),
            ModuleKind::SpectralBloom => self.spectral_bloom.controls(ui),
            ModuleKind::Orbital => self.orbital.controls(ui),
            ModuleKind::Milkdrop => self.milkdrop.controls(ui),
            ModuleKind::Resonance => self.resonance.controls(ui),
            ModuleKind::Rings => self.rings.controls(ui),
            ModuleKind::Cylinder => self.cylinder.controls(ui),
            ModuleKind::Sphere => self.sphere.controls(ui),
            ModuleKind::Waterfall => self.waterfall.controls(ui),
            ModuleKind::Spectrum => self.spectrum.controls(ui, frame),
            ModuleKind::Waveform => self.waveform.controls(ui),
            ModuleKind::Spectrogram => self.spectrogram.controls(ui),
            ModuleKind::Stereometer => self.stereometer.controls(ui),
            ModuleKind::Oscilloscope => self.oscilloscope.controls(ui),
            ModuleKind::Veroni => self.veroni.controls(ui, frame),
            ModuleKind::Strata => self.strata.controls(ui, frame),
            ModuleKind::Flux => self.flux.controls(ui, frame),
        }
    }

    pub(crate) fn resolve_inspector_focus(&mut self) {
        if let Some(section) = self.inspector_focus.take() {
            self.select_section(section);
        }
    }

    pub fn inspector_controls(&mut self, ui: &mut egui::Ui, frame: &AnalysisFrame) {
        let section = self.active_inspector_section();
        ui.data_mut(|d| d.insert_temp(egui::Id::new("property-row-index"), 0_usize));
        ui.data_mut(|d| d.insert_temp(egui::Id::new(VISUAL_INSPECTOR), (self.kind, section)));
        ui.push_id(self.kind.label(), |ui| {
            ui.data_mut(|d| d.insert_temp(egui::Id::new(FLAT_INSPECTOR), true));
            egui::Frame::new()
                .inner_margin(egui::Margin::symmetric(10, 5))
                .show(ui, |ui| {
                    if section == SettingsSection::Appearance {
                        // No visualizer-specific group filter applies to scene properties.
                        ui.data_mut(|d| {
                            d.remove::<(ModuleKind, SettingsSection)>(egui::Id::new(
                                VISUAL_INSPECTOR,
                            ));
                        });
                        self.background.controls(ui);
                        if self.kind == ModuleKind::Milkdrop {
                            ui.small("Milkdrop presets render their own full-frame background.");
                        }
                        ui.data_mut(|d| {
                            d.insert_temp(egui::Id::new(VISUAL_INSPECTOR), (self.kind, section))
                        });
                    }
                    if section == SettingsSection::Camera
                        && let Some(camera) = self.controlled_camera()
                    {
                        camera::views(camera, ui);
                    }
                    self.section_controls(ui, frame);
                    if section == SettingsSection::Camera
                        && self
                            .controlled_camera()
                            .is_some_and(|m| camera::controls(m, ui))
                    {
                        self.camera_runtime[self.kind as usize] = Default::default();
                    }
                });
            ui.data_mut(|d| d.remove::<bool>(egui::Id::new(FLAT_INSPECTOR)));
        });
        ui.data_mut(|d| {
            d.remove::<(ModuleKind, SettingsSection)>(egui::Id::new(VISUAL_INSPECTOR));
        });
    }

    fn controlled_camera(&mut self) -> Option<&mut dyn camera::Controlled> {
        Some(match self.kind {
            ModuleKind::Sphere => &mut self.sphere,
            ModuleKind::Cylinder => &mut self.cylinder,
            ModuleKind::Rings => &mut self.rings,
            ModuleKind::Resonance => &mut self.resonance,
            ModuleKind::Orbital => &mut self.orbital,
            ModuleKind::Cymatics => &mut self.cymatics,
            ModuleKind::ParticleWaves => &mut self.particle_waves,
            ModuleKind::SpectralBloom => &mut self.spectral_bloom,
            ModuleKind::Flux => &mut self.flux,
            ModuleKind::Strata => &mut self.strata,
            _ => return None,
        })
    }

    pub fn inspector_sections(&self) -> &'static [SettingsSection] {
        use SettingsSection::*;
        if self.sections().contains(&Camera) {
            &[Geometry, Appearance, Camera]
        } else {
            &[Geometry, Appearance]
        }
    }

    pub fn active_inspector_section(&self) -> SettingsSection {
        visual_category_for(self.kind, self.active_section().title())
    }

    pub fn select_inspector_section(&mut self, section: SettingsSection) {
        // Keep the existing saved section indices readable; the new presentation
        // groups them without changing any parameter addresses or values.
        if let Some(index) = self
            .sections()
            .iter()
            .position(|s| visual_category_for(self.kind, s.title()) == section)
        {
            self.active_sections[self.kind as usize] = index;
        }
    }

    pub fn sections(&self) -> &'static [SettingsSection] {
        use SettingsSection::*;
        match self.kind {
            ModuleKind::Waterfall | ModuleKind::Sphere | ModuleKind::Cylinder => {
                &[Geometry, Frequency, Response, Appearance, Camera]
            }
            ModuleKind::Cymatics => &[Geometry, Particles, Response, Appearance, Flow, Camera],
            ModuleKind::ParticleWaves => &[Geometry, Particles, Response, Flow, Appearance, Camera],
            ModuleKind::SpectralBloom => &[
                Geometry, Particles, Response, Flow, Feedback, Appearance, Camera,
            ],
            ModuleKind::Orbital => &[Particles, Orbs, Response, Camera, Rendering],
            ModuleKind::Milkdrop => &[Presets, Flow, Appearance, Response, Rendering],
            ModuleKind::Resonance => &[Geometry, Flow, Frequency, Response, Appearance, Camera],
            ModuleKind::Rings => &[Geometry, Channels, TimeWindow, Response, Appearance, Camera],
            ModuleKind::Spectrum => &[Frequency, Response, Appearance],
            ModuleKind::Spectrogram => &[History, Frequency, Response, Appearance],
            ModuleKind::Waveform => &[Channels, TimeWindow, Amplitude, Appearance],
            ModuleKind::Stereometer => &[Display, Appearance, Correlation],
            ModuleKind::Oscilloscope => &[Channels, Sync, TimeWindow, Amplitude, Appearance],
            ModuleKind::Veroni => &[Modulation, Geometry, Flow, Appearance],
            ModuleKind::Strata | ModuleKind::Flux => {
                &[Modulation, Geometry, Flow, Particles, Appearance, Camera]
            }
        }
    }

    pub fn active_section(&self) -> SettingsSection {
        self.sections()[self.active_sections[self.kind as usize]]
    }

    pub fn select_section(&mut self, section: SettingsSection) {
        if let Some(index) = self
            .sections()
            .iter()
            .position(|candidate| *candidate == section)
        {
            self.active_sections[self.kind as usize] = index;
        }
    }

    pub fn ingest(&mut self, frames: &[std::sync::Arc<crate::capture_history::SpectralFrame>]) {
        for frame in frames {
            if frame.sequence <= self.last_capture {
                continue;
            }
            self.last_capture = frame.sequence;
            if self.is_frozen() {
                continue;
            }
            let time = frame.time - self.paused_duration;
            match self.kind {
                ModuleKind::Cylinder => self.cylinder.ingest(frame, time),
                ModuleKind::Sphere => self.sphere.ingest(frame, time),
                ModuleKind::Waterfall => self.waterfall.ingest(frame, time),
                ModuleKind::Spectrogram => self.spectrogram.ingest(frame, time),
                ModuleKind::ParticleWaves => self.particle_waves.ingest(frame, time),
                ModuleKind::SpectralBloom => self.spectral_bloom.ingest(frame, time),
                _ => {}
            }
        }
    }

    pub fn ingest_input(
        &mut self,
        main: &[std::sync::Arc<crate::capture_history::SpectralFrame>],
        frame: &AnalysisFrame,
    ) {
        use crate::signals::audio::Connection;
        let feed = match self.audio_input {
            Connection::Input(id) => frame
                .signals
                .as_ref()
                .and_then(|s| s.audio.get(&id))
                .cloned(),
            _ => None,
        };
        let revision = (
            self.audio_input,
            frame.capture_epoch,
            feed.as_ref().map_or(0, |f| f.revision),
        );
        if self.input_revision != Some(revision) {
            self.clear_input_history();
            self.input_revision = Some(revision);
        }
        match self.audio_input {
            Connection::Main => self.ingest(main),
            Connection::Input(_) => {
                if let Some(feed) = feed {
                    let frames = feed
                        .history
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .since(self.last_capture);
                    self.ingest(&frames);
                }
            }
            Connection::Off => {}
        }
    }

    fn clear_input_history(&mut self) {
        self.last_capture = 0;
        self.waterfall.clear();
        self.sphere.clear();
        self.cylinder.clear();
        self.spectrogram.clear();
        self.spectrum.clear();
        self.particle_waves.clear_input_history();
        self.spectral_bloom.clear_input_history();
    }

    pub fn routed_frame(&mut self, main: &AnalysisFrame) -> AnalysisFrame {
        use crate::signals::audio::Connection;
        let feed = match self.audio_input {
            Connection::Main => return main.clone(),
            Connection::Input(id) => main.signals.as_ref().and_then(|s| s.audio.get(&id)),
            Connection::Off => None,
        };
        let mut frame = if let Some(feed) = feed {
            (*feed.frame).clone()
        } else {
            if self.silent_input.as_ref().is_none_or(|s| {
                s.capture_epoch != main.capture_epoch
                    || s.fft_size != main.fft_size
                    || s.waveform_left.len() != main.waveform_left.len()
            }) {
                self.silent_input = Some(crate::signals::audio::silence(main));
            }
            let mut silent = self.silent_input.as_ref().unwrap().clone();
            silent.sequence = main.sequence;
            silent
        };
        frame.signals = main.signals.clone();
        frame
    }

    pub fn input_controls(&mut self, ui: &mut egui::Ui, frame: &AnalysisFrame) {
        for section in [
            SettingsSection::Frequency,
            SettingsSection::Channels,
            SettingsSection::Response,
            SettingsSection::Modulation,
        ] {
            if !self.sections().contains(&section) {
                continue;
            }
            inspector_section(ui, section.title(), |ui| {
                ui.data_mut(|d| d.insert_temp(ui.id().with("active-module-section"), section));
                self.section_controls(ui, frame);
                ui.data_mut(|d| d.remove::<SettingsSection>(ui.id().with("active-module-section")));
            });
        }
        ui.data_mut(|d| {
            d.remove::<crate::macros::InspectorIndicators>(egui::Id::new(
                crate::macros::INDICATORS,
            ));
            d.remove::<crate::macros::ParameterContext>(egui::Id::new(crate::macros::CONTEXT));
        });
    }

    pub fn draw(
        &mut self,
        ui: &mut egui::Ui,
        rect: Rect,
        frame: &AnalysisFrame,
        theme: &AppTheme,
        live: bool,
    ) {
        let routed = self.routed_frame(frame);
        let frame = &routed;
        let now = self.frozen.as_ref().map_or_else(
            || Instant::now() - self.paused_duration,
            |frozen| frozen.time,
        );
        let frozen_frame = self.frozen.as_ref().map(|frozen| frozen.frame.clone());
        let frame = frozen_frame.as_ref().unwrap_or(frame);
        let frozen = self.frozen.is_some();
        let live = live || self.frozen.is_some();
        let restore = if self
            .performance_state()
            .has_modulation(self.kind, frame.signals.as_deref())
        {
            let mut settings = self.copy_settings();
            let restore =
                self.performance_state()
                    .apply(self.kind, &mut settings, frame.signals.as_deref());
            if !restore.is_empty() {
                self.apply_live_settings(&settings);
            }
            restore
        } else {
            Vec::new()
        };
        if self.kind != ModuleKind::Milkdrop {
            self.milkdrop.release_if_inactive();
        }
        if self.kind != ModuleKind::Cymatics {
            self.cymatics.release_if_inactive();
        }
        if self.kind != ModuleKind::SpectralBloom {
            self.spectral_bloom.release_if_inactive();
        }
        if self.kind != ModuleKind::ParticleWaves {
            self.particle_waves.release_if_inactive();
        }
        if self.kind != ModuleKind::Orbital {
            self.orbital.release_if_inactive();
        }
        let render_state = ModuleRenderState { live, frozen };
        let mut canvas_theme = theme.clone();
        canvas_theme.background = self.background.sample([0.5; 2]);
        let [r, g, b, _] = canvas_theme.background.to_array();
        if (r as f32 * 0.2126 + g as f32 * 0.7152 + b as f32 * 0.0722) > 145.0 {
            canvas_theme.foreground = Color32::from_gray(25);
            canvas_theme.muted = Color32::from_gray(75);
        }
        let theme = &canvas_theme;
        if !crate::background::pane_painted(ui) {
            self.background.paint(ui, rect);
        }
        let previous_background = ui.data_mut(|d| {
            let previous = d.get_temp::<crate::background::Background>(egui::Id::new(
                crate::background::CONTEXT,
            ));
            d.insert_temp(egui::Id::new(crate::background::CONTEXT), self.background);
            previous
        });
        ui.interact(rect, ui.id().with("module-help"), egui::Sense::hover())
            .help_text(self.kind.description());
        match self.kind {
            ModuleKind::Milkdrop => self.milkdrop.draw(ui, rect, frame, now, live, frozen),
            ModuleKind::Cymatics => camera::draw(
                &mut self.cymatics,
                &mut self.camera_runtime[self.kind as usize],
                ui,
                rect,
                now,
                true,
                |m, ui| m.draw(ui, rect, frame, theme, now, live),
            ),
            ModuleKind::SpectralBloom => camera::draw(
                &mut self.spectral_bloom,
                &mut self.camera_runtime[self.kind as usize],
                ui,
                rect,
                now,
                true,
                |m, ui| m.draw(ui, rect, frame, theme, now, live),
            ),
            ModuleKind::ParticleWaves => camera::draw(
                &mut self.particle_waves,
                &mut self.camera_runtime[self.kind as usize],
                ui,
                rect,
                now,
                true,
                |m, ui| m.draw(ui, rect, frame, theme, now, live),
            ),
            ModuleKind::Orbital => camera::draw(
                &mut self.orbital,
                &mut self.camera_runtime[self.kind as usize],
                ui,
                rect,
                now,
                true,
                |m, ui| m.draw(ui, rect, frame, theme, now, live),
            ),
            ModuleKind::Resonance => camera::draw(
                &mut self.resonance,
                &mut self.camera_runtime[self.kind as usize],
                ui,
                rect,
                now,
                false,
                |m, ui| m.draw(ui, rect, frame, theme, now, live),
            ),
            ModuleKind::Rings => camera::draw(
                &mut self.rings,
                &mut self.camera_runtime[self.kind as usize],
                ui,
                rect,
                now,
                false,
                |m, ui| m.draw(ui, rect, frame, theme, now, live),
            ),
            ModuleKind::Cylinder => camera::draw(
                &mut self.cylinder,
                &mut self.camera_runtime[self.kind as usize],
                ui,
                rect,
                now,
                false,
                |m, ui| m.draw(ui, rect, frame, theme, now),
            ),
            ModuleKind::Sphere => camera::draw(
                &mut self.sphere,
                &mut self.camera_runtime[self.kind as usize],
                ui,
                rect,
                now,
                false,
                |m, ui| m.draw(ui, rect, frame, theme, now),
            ),
            ModuleKind::Waterfall => self.waterfall.draw(ui, rect, frame, theme, now, false),
            ModuleKind::Spectrum => self
                .spectrum
                .draw(ui, rect, frame, theme, now, render_state),
            ModuleKind::Waveform => self.waveform.draw(ui, rect, frame, theme, now, live),
            ModuleKind::Spectrogram => self.spectrogram.draw(
                ui,
                rect,
                frame,
                theme,
                now,
                ModuleRenderState {
                    live: false,
                    frozen,
                },
            ),
            ModuleKind::Stereometer => self.stereometer.draw(ui, rect, frame, theme, live),
            ModuleKind::Oscilloscope => self.oscilloscope.draw(ui, rect, frame, theme, live),
            ModuleKind::Veroni => self.veroni.draw(ui, rect, frame, theme, now, live),
            ModuleKind::Strata => camera::draw(
                &mut self.strata,
                &mut self.camera_runtime[self.kind as usize],
                ui,
                rect,
                now,
                true,
                |m, ui| m.draw(ui, rect, frame, theme, now, live),
            ),
            ModuleKind::Flux => camera::draw(
                &mut self.flux,
                &mut self.camera_runtime[self.kind as usize],
                ui,
                rect,
                now,
                true,
                |m, ui| m.draw(ui, rect, frame, theme, now, live),
            ),
        }
        ui.data_mut(|d| {
            d.remove::<crate::background::Background>(egui::Id::new(crate::background::CONTEXT));
            if let Some(previous) = previous_background {
                d.insert_temp(egui::Id::new(crate::background::CONTEXT), previous);
            }
        });
        if !restore.is_empty() {
            let mut settings = self.copy_settings();
            crate::macros::set_many(&mut settings, &restore);
            self.apply_live_settings(&settings);
        }
        if self.frozen.is_some() {
            ui.painter().text(
                rect.right_top() + egui::vec2(-6.0, 4.0),
                egui::Align2::RIGHT_TOP,
                "PAUSED",
                FontId::monospace(10.0),
                theme.foreground,
            );
        }
        if !live
            && !matches!(
                self.kind,
                ModuleKind::Orbital
                    | ModuleKind::Milkdrop
                    | ModuleKind::Cymatics
                    | ModuleKind::ParticleWaves
                    | ModuleKind::SpectralBloom
            )
        {
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "Waiting for audio",
                FontId::proportional(14.0),
                theme.muted,
            );
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum TraceStyle {
    Bars,
    Line,
    Filled,
}

// Popups otherwise inherit the application's roomier menu defaults. Keep every
// properties picker consistent and reserve enough height for its complete list.
fn properties_combo(id: &str, width: f32, rows: usize) -> crate::dropdown::ComboBox {
    crate::dropdown::ComboBox::from_id_salt(id)
        .width(width)
        .height(rows as f32 * 26.0)
        .truncate()
}

impl TraceStyle {
    fn label(self) -> &'static str {
        match self {
            Self::Bars => "Bars",
            Self::Line => "Line",
            Self::Filled => "Filled area",
        }
    }

    fn controls(&mut self, ui: &mut egui::Ui, bars: bool) {
        property_row(ui, "Style", |ui| {
            properties_combo("trace-style", ui.available_width(), 3)
                .selected_text(self.label())
                .show_ui(ui, |ui| {
                    for style in [Self::Bars, Self::Line, Self::Filled] {
                        if style == Self::Bars && !bars {
                            continue;
                        }
                        let (rect, response) =
                            ui.allocate_exact_size(egui::vec2(176.0, 22.0), egui::Sense::click());
                        if response.hovered() || *self == style {
                            ui.painter()
                                .rect_filled(rect, 0.0, ui.visuals().selection.bg_fill);
                        }
                        let icon_rect = Rect::from_min_size(rect.min, egui::vec2(24.0, 24.0));
                        crate::icons::paint(
                            ui,
                            icon_rect,
                            match style {
                                Self::Bars => Icon::Bars,
                                Self::Line => Icon::Waveform,
                                Self::Filled => Icon::Surface,
                            },
                            ui.visuals().text_color(),
                        );
                        ui.painter().text(
                            rect.left_center() + egui::vec2(30.0, 0.0),
                            egui::Align2::LEFT_CENTER,
                            style.label(),
                            FontId::proportional(11.0),
                            ui.visuals().text_color(),
                        );
                        if response.clicked() {
                            *self = style;
                            ui.close();
                        }
                        response.help_text(
                        "Choose a trace style. Its relevant appearance controls are shown below.",
                    );
                    }
                })
                .response
                .help_text("Choose how the signal is drawn; does not change analysis.");
        });
    }
}

fn trace_mesh(points: &[Pos2], baseline: f32, color: Color32) -> egui::Mesh {
    let mut mesh = egui::Mesh::default();
    for pair in points.windows(2) {
        let base = mesh.vertices.len() as u32;
        // Split at zero crossings: an unsplit quad would self-intersect and
        // double-blend part of a translucent waveform fill.
        if (pair[0].y - baseline) * (pair[1].y - baseline) < 0.0 {
            let t = (baseline - pair[0].y) / (pair[1].y - pair[0].y);
            let crossing = Pos2::new(egui::lerp(pair[0].x..=pair[1].x, t), baseline);
            for p in [
                pair[0],
                crossing,
                Pos2::new(pair[0].x, baseline),
                pair[1],
                Pos2::new(pair[1].x, baseline),
            ] {
                mesh.colored_vertex(p, color);
            }
            mesh.add_triangle(base, base + 1, base + 2);
            mesh.add_triangle(base + 1, base + 3, base + 4);
            continue;
        }
        for p in [
            pair[0],
            pair[1],
            Pos2::new(pair[1].x, baseline),
            Pos2::new(pair[0].x, baseline),
        ] {
            mesh.colored_vertex(p, color);
        }
        mesh.add_triangle(base, base + 1, base + 2);
        mesh.add_triangle(base, base + 2, base + 3);
    }
    mesh
}

fn fill_trace(painter: &egui::Painter, points: &[Pos2], baseline: f32, color: Color32) {
    painter.add(egui::Shape::mesh(trace_mesh(points, baseline, color)));
}

pub(crate) fn settings_panel(ui: &mut egui::Ui, title: &str, body: impl FnOnce(&mut egui::Ui)) {
    if let Some((kind, tab)) = visual_inspector(ui) {
        if matches!(
            title,
            "Signal Response" | "Audio mappings" | "Frequency Range" | "Channels"
        ) {
            return;
        }
        let split = kind == ModuleKind::Orbital && matches!(title, "Particles" | "Orbs")
            || matches!(kind, ModuleKind::Spectrum | ModuleKind::Spectrogram)
                && title == "Appearance";
        let category = if title == "Particles"
            && matches!(
                kind,
                ModuleKind::ParticleWaves | ModuleKind::SpectralBloom | ModuleKind::Flux
            ) {
            SettingsSection::Appearance
        } else {
            visual_category(title)
        };
        if tab != category && !(split && tab != SettingsSection::Camera) {
            return;
        }
        ui.push_id(title, |ui| {
            let old = ui.data(|d| d.get_temp::<String>(egui::Id::new(VISUAL_PANEL)));
            ui.data_mut(|d| d.insert_temp(egui::Id::new(VISUAL_PANEL), title.to_owned()));
            if matches!(
                title,
                "Flow & Motion"
                    | "Particles"
                    | "Rendering"
                    | "Presets"
                    | "Feedback"
                    | "Time Window"
                    | "Time & History"
                    | "Amplitude"
                    | "Synchronization"
                    | "Correlation"
                    | "Display"
            ) && kind != ModuleKind::Orbital
            {
                settings_group(ui, title, body);
            } else {
                body(ui);
            }
            // The title is only used while rendering this block.
            ui.data_mut(|d| {
                if let Some(old) = old {
                    d.insert_temp(egui::Id::new(VISUAL_PANEL), old);
                } else {
                    d.remove::<String>(egui::Id::new(VISUAL_PANEL));
                }
            });
        });
        return;
    }
    let selected =
        ui.data(|data| data.get_temp::<SettingsSection>(ui.id().with("active-module-section")));
    if selected.is_some_and(|section| section.title() != title) {
        return;
    }
    // Render the requested section inside its inspector disclosure.
    ui.push_id(title, body);
}

/// The inspector's outer bounds allow rules to span padding and the scroll gutter.
pub(crate) const FLAT_INSPECTOR: &str = "visualizer-flat-inspector";
const VISUAL_INSPECTOR: &str = "visualizer-inspector-tab";
const VISUAL_PANEL: &str = "visualizer-inspector-source-section";

fn visual_inspector(ui: &egui::Ui) -> Option<(ModuleKind, SettingsSection)> {
    ui.data(|d| d.get_temp(egui::Id::new(VISUAL_INSPECTOR)))
}

fn visual_category(title: &str) -> SettingsSection {
    match title {
        "Camera" => SettingsSection::Camera,
        "Appearance" | "Rendering" | "Feedback" | "Correlation" => SettingsSection::Appearance,
        _ => SettingsSection::Geometry,
    }
}

fn visual_category_for(kind: ModuleKind, title: &str) -> SettingsSection {
    if title == "Particles"
        && matches!(
            kind,
            ModuleKind::ParticleWaves | ModuleKind::SpectralBloom | ModuleKind::Flux
        )
    {
        SettingsSection::Appearance
    } else {
        visual_category(title)
    }
}

fn flat_inspector(ui: &egui::Ui) -> bool {
    ui.data(|d| {
        d.get_temp::<bool>(egui::Id::new(FLAT_INSPECTOR))
            .unwrap_or(false)
    })
}

pub(crate) fn inspector_bounds(ui: &egui::Ui) -> Option<Rect> {
    ui.data(|data| data.get_temp::<Rect>(egui::Id::new("inspector-bounds")))
}

pub(crate) fn inspector_rule(ui: &mut egui::Ui) {
    crate::workspace_ui::sidebar_rule(ui, inspector_bounds(ui).unwrap_or(ui.max_rect()));
}

/// Shared active-section framing for either inspector: rules reach the pane edges.
pub(crate) fn inspector_section(ui: &mut egui::Ui, title: &str, body: impl FnOnce(&mut egui::Ui)) {
    if flat_inspector(ui) {
        settings_group(ui, title, body);
        return;
    }
    inspector_rule(ui);
    egui::Frame::new()
        .fill(ui.visuals().widgets.noninteractive.bg_fill)
        .inner_margin(egui::Margin::symmetric(8, 5))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.strong(title);
        });
    inspector_rule(ui);
    egui::Frame::new()
        .inner_margin(egui::Margin::symmetric(8, 5))
        .show(ui, body);
    inspector_rule(ui);
}

/// Compact disclosure rows, with state scoped to the pane, module, and section.
pub(crate) fn settings_group(ui: &mut egui::Ui, title: &str, body: impl FnOnce(&mut egui::Ui)) {
    settings_group_with_default(ui, title, true, body);
}

pub(crate) fn settings_group_with_default(
    ui: &mut egui::Ui,
    title: &str,
    open: bool,
    body: impl FnOnce(&mut egui::Ui),
) {
    if let Some((kind, tab)) = visual_inspector(ui) {
        if kind != ModuleKind::Waterfall && matches!(title, "View presets" | "Auto-orbit") {
            return;
        }
        let panel = ui
            .data(|d| d.get_temp::<String>(egui::Id::new(VISUAL_PANEL)))
            .unwrap_or_default();
        let group_title = title.rsplit(" · ").next().unwrap_or(title);
        let category = match (kind, panel.as_str(), group_title) {
            (
                ModuleKind::Orbital,
                "Particles",
                "Population & quality"
                | "Flow & motion"
                | "Flocking"
                | "Turbulence"
                | "Cloud boundaries",
            ) => SettingsSection::Geometry,
            (ModuleKind::Orbital, "Particles", _) => SettingsSection::Appearance,
            (
                ModuleKind::Orbital,
                "Orbs",
                "Arrangement & motion"
                | "Size & shape"
                | "Attraction & orbit"
                | "Surface field"
                | "Size pulsing",
            ) => SettingsSection::Geometry,
            (ModuleKind::Orbital, "Orbs", _) => SettingsSection::Appearance,
            (ModuleKind::Spectrum, "Appearance", "Trace") => SettingsSection::Geometry,
            (ModuleKind::Spectrogram, "Appearance", "Layout & detail") => SettingsSection::Geometry,
            (
                ModuleKind::ParticleWaves | ModuleKind::SpectralBloom | ModuleKind::Flux,
                "Particles",
                _,
            ) => SettingsSection::Appearance,
            _ => visual_category(&panel),
        };
        if tab != category {
            return;
        }
    }
    if inspector_bounds(ui).is_some() {
        ui.add_space(3.0);
        inspector_rule(ui);
        egui::CollapsingHeader::new(egui::RichText::new(title).size(12.0).strong())
            .default_open(open)
            .show_unindented(ui, |ui| {
                if flat_inspector(ui) {
                    ui.add_space(5.0);
                }
                ui.push_id(title, body);
                if flat_inspector(ui) {
                    ui.add_space(8.0);
                }
            });
        return;
    }
    ui.separator();
    ui.scope(|ui| {
        let widgets = ui.visuals().widgets.clone();
        let accent = ui.visuals().selection.bg_fill;
        let visuals = ui.visuals_mut();
        // egui uses this flag for full-row hit targets as well as painting.
        // Keep the hit target, but make every header state transparent.
        visuals.collapsing_header_frame = true;
        for widget in [
            &mut visuals.widgets.noninteractive,
            &mut visuals.widgets.inactive,
            &mut visuals.widgets.hovered,
            &mut visuals.widgets.active,
            &mut visuals.widgets.open,
        ] {
            widget.weak_bg_fill = Color32::TRANSPARENT;
            widget.bg_stroke = egui::Stroke::NONE;
        }
        visuals.widgets.hovered.fg_stroke.color = accent;
        visuals.widgets.active.fg_stroke.color = accent;
        egui::CollapsingHeader::new(title)
            .default_open(open)
            .show_background(false)
            .show_unindented(ui, |ui| {
                // Fields retain their normal recessed backgrounds.
                ui.visuals_mut().widgets = widgets;
                ui.visuals_mut().collapsing_header_frame = false;
                ui.add_space(2.0);
                body(ui);
                ui.add_space(4.0);
            });
    });
}

pub(crate) fn property_background(ui: &egui::Ui, rect: Rect) {
    if inspector_bounds(ui).is_none() || flat_inspector(ui) {
        return;
    }
    let index = ui.data_mut(|d| {
        let id = egui::Id::new("property-row-index");
        let n = d.get_temp::<usize>(id).unwrap_or(0);
        d.insert_temp(id, n + 1);
        n
    });
    if index % 2 == 1 {
        ui.painter()
            .rect_filled(rect, 0.0, ui.visuals().text_color().gamma_multiply(0.025));
    }
}

/// Shared two-column property rows, aligned with compact numeric controls.
pub(crate) fn property_row(ui: &mut egui::Ui, label: &str, control: impl FnOnce(&mut egui::Ui)) {
    let width = ui.available_width();
    property_background(
        ui,
        Rect::from_min_size(ui.cursor().min, egui::vec2(width, 22.0)),
    );
    ui.horizontal(|ui| {
        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(width * 0.43, 22.0), egui::Sense::hover());
        let mut label_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        label_ui
            .add(egui::Label::new(label).truncate())
            .help_fallback(|| format!("{label}\n{}", crate::help::control_description(label)));
        let (rect, _) = ui.allocate_exact_size(
            egui::vec2((width * 0.57 - ui.spacing().item_spacing.x).max(30.0), 22.0),
            egui::Sense::hover(),
        );
        let mut field = ui.new_child(
            egui::UiBuilder::new()
                .id_salt(label)
                .max_rect(rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        control(&mut field);
    })
    .response
    .help_fallback(|| format!("{label}\n{}", crate::help::control_description(label)));
}

fn label(painter: &egui::Painter, position: Pos2, text: impl ToString, color: Color32) {
    painter.text(
        position,
        egui::Align2::LEFT_TOP,
        text,
        FontId::monospace(10.0),
        color,
    );
}

pub(crate) fn mix(a: Color32, b: Color32, amount: f32) -> Color32 {
    let channel = |left: u8, right: u8| {
        (left as f32 + (right as f32 - left as f32) * amount.clamp(0.0, 1.0)) as u8
    };
    Color32::from_rgb(
        channel(a.r(), b.r()),
        channel(a.g(), b.g()),
        channel(a.b(), b.b()),
    )
}

fn frequency_label(hz: f32) -> String {
    if hz >= 1000.0 {
        format!("{:.1}k", hz / 1000.0)
    } else {
        format!("{hz:.0}")
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn live_input_and_insert_gain_preserve_waterfall_history_and_refresh_downstream_audio() {
        use crate::signals::audio;
        use crate::signals::{Bank, Input, Processor, inserts::Effect};
        use std::sync::Arc;
        for mode in 0..3 {
            let mut bank = Bank::default();
            let id = bank.add_source(Input::System).unwrap();
            if mode > 0 {
                let d = &mut bank.channels[0];
                let chain = d.inserts.as_mut().unwrap();
                chain
                    .add(d.input, 0, Effect::AudioGain { db: 0.0 })
                    .unwrap();
                chain
                    .add(d.input, 1, Effect::LowPass { hz: 1000.0 })
                    .unwrap();
            }
            let mut p = Processor::default();
            let settings = crate::analysis::AnalysisSettings {
                smoothing: 0.0,
                ..Default::default()
            };
            let samples: Vec<_> = (0..24000)
                .map(|i| [0.1 * (std::f32::consts::TAU * 100.0 * i as f32 / 48000.0).sin(); 2])
                .collect();
            let mut f = crate::analysis::Analyzer::new(4096, settings.window)
                .analyze(&samples, 0, 48000, 2, &settings, 1, 0);
            f.capture_epoch = 3;
            let first = p.step(&bank, &f, 0.01, true);
            let original = first.audio[&id].clone();
            let mut pane = ModulePane::new(ModuleKind::Waterfall);
            pane.audio_input = audio::Connection::Input(id);
            let mut routed = f.clone();
            routed.signals = Some(Arc::new(first));
            pane.ingest_input(&[], &routed);
            assert_eq!(pane.waterfall.history_len(), 1);

            if mode == 2 {
                bank.channels[0].inserts.as_mut().unwrap().stages[0].effect =
                    Effect::AudioGain { db: 6.0206 };
            } else {
                bank.audio_inputs[0].gain_db = 6.0206;
            }
            // Reuse the capture sequence: a live edit must not wait for another FFT.
            let changed = p.step(&bank, &f, 0.02, true);
            let feed = changed.audio[&id].clone();
            assert_eq!(feed.revision, original.revision);
            assert!(Arc::ptr_eq(&feed.history, &original.history));
            assert!((feed.frame.mix_rms / original.frame.mix_rms - 2.0).abs() < 0.002);
            routed.signals = Some(Arc::new(changed));
            pane.ingest_input(&[], &routed);
            assert_eq!(pane.waterfall.history_len(), 1);
            assert_eq!(pane.last_capture, 1);

            let mut next = f.clone();
            next.sequence = 2;
            let snapshot = p.step(&bank, &next, 0.06, true);
            let feed = &snapshot.audio[&id];
            // Supply deterministic archive time instead of sleeping in the test.
            feed.history.lock().unwrap().push(
                &feed.frame,
                Instant::now() + std::time::Duration::from_millis(50),
            );
            routed.signals = Some(Arc::new(snapshot));
            pane.ingest_input(&[], &routed);
            assert_eq!(pane.waterfall.history_len(), 2);
            assert_eq!(pane.last_capture, 2);
        }
    }

    use super::*;

    #[test]
    fn hidden_history_panes_catch_up_without_painting_and_manual_pause_still_skips_audio() {
        use std::sync::Arc;
        let mut panes = [
            ModulePane::new(ModuleKind::Spectrogram),
            ModulePane::new(ModuleKind::Waterfall),
        ];
        let start = Instant::now();
        let frames: Vec<_> = (0..240)
            .map(|i| {
                Arc::new(crate::capture_history::SpectralFrame {
                    sequence: i + 1,
                    time: start + Duration::from_millis(i * 34),
                    sample_rate: 48000,
                    fft_size: 512,
                    magnitudes: vec![0.1; 257].into(),
                })
            })
            .collect();
        // No UI frame or draw call occurs during these eight seconds of capture.
        for pane in &mut panes {
            pane.ingest(&frames);
        }
        assert_eq!(panes[0].spectrogram.history_len(), 240);
        assert_eq!(panes[1].waterfall.history_len(), 240);
        let paused_at = start + Duration::from_secs(9);
        panes[0].toggle_freeze(
            &AnalysisFrame {
                sequence: 240,
                ..Default::default()
            },
            paused_at,
        );
        let next = Arc::new(crate::capture_history::SpectralFrame {
            sequence: 241,
            time: paused_at + Duration::from_secs(1),
            sample_rate: 48000,
            fft_size: 512,
            magnitudes: vec![0.2; 257].into(),
        });
        for pane in &mut panes {
            pane.ingest(std::slice::from_ref(&next));
        }
        assert_eq!(panes[0].spectrogram.history_len(), 240);
        assert_eq!(panes[1].waterfall.history_len(), 241);
        panes[0].toggle_freeze(
            &AnalysisFrame {
                sequence: 241,
                ..Default::default()
            },
            paused_at + Duration::from_secs(1),
        );
        panes[0].ingest(&frames);
        assert_eq!(panes[0].spectrogram.history_len(), 240);
    }

    #[test]
    fn legacy_visualizer_presets_migrate_to_editable_mappings_without_losing_geometry() {
        for kind in [ModuleKind::Flux, ModuleKind::Strata, ModuleKind::Veroni] {
            let mut value = serde_json::to_value(ModulePane::new(kind).copy_settings()).unwrap();
            let object = value
                .as_object_mut()
                .unwrap()
                .values_mut()
                .next()
                .unwrap()
                .as_object_mut()
                .unwrap();
            object.remove("modulation");
            object.insert("sensitivity".into(), serde_json::json!(2.0));
            object.insert("attack_ms".into(), serde_json::json!(123.0));
            object.insert("gate_db".into(), serde_json::json!(-72.0));
            let legacy: ModuleSettings = serde_json::from_value(value).unwrap();
            let mut pane = ModulePane::new(kind);
            assert!(pane.paste_settings(&legacy));
            let migrated = serde_json::to_value(pane.copy_settings()).unwrap();
            let settings = migrated.as_object().unwrap().values().next().unwrap();
            assert_eq!(settings["modulation"]["amount"], 2.0);
            assert_eq!(settings["modulation"]["ambient"], 1.0);
            assert_eq!(
                settings["modulation"]["routes"][0]["envelope"]["attack"],
                123.0
            );
            assert_eq!(settings["modulation"]["routes"][0]["floor_db"], -72.0);
            let saved: ModuleSettings = serde_json::from_value(migrated).unwrap();
            assert!(pane.paste_settings(&saved));
            assert_eq!(pane.copy_settings(), saved);
        }
    }

    #[test]
    fn settings_clipboard_is_typed_and_does_not_copy_capture_or_pause_state() {
        for kind in ModuleKind::ALL {
            let mut source = ModulePane::new(kind);
            source.toggle_freeze(&AnalysisFrame::default(), Instant::now());
            let settings = source.copy_settings();
            assert_eq!(settings.kind(), kind);
            let mut target = ModulePane::new(kind);
            assert!(target.paste_settings(&settings));
            assert!(target.copy_settings() == settings);
            assert!(!target.is_frozen());
            assert_eq!(target.paused_duration, Duration::ZERO);
            target.kind = if kind == ModuleKind::Waveform {
                ModuleKind::Spectrum
            } else {
                ModuleKind::Waveform
            };
            let before = target.copy_settings();
            assert!(!target.paste_settings(&settings));
            assert!(target.copy_settings() == before);
        }
    }

    #[test]
    fn pause_captures_only_one_pane_and_resume_omits_paused_time() {
        let mut pane = ModulePane::new(ModuleKind::Waveform);
        let other = ModulePane::new(ModuleKind::Waveform);
        let now = Instant::now();
        let mut frame = AnalysisFrame {
            sequence: 42,
            ..AnalysisFrame::default()
        };
        pane.toggle_freeze(&frame, now);
        frame.sequence = 43;
        std::sync::Arc::make_mut(&mut frame.waveform_left).fill(1.0);
        assert_eq!(pane.frozen.as_ref().unwrap().frame.sequence, 42);
        assert_eq!(pane.frozen.as_ref().unwrap().frame.waveform_left[0], 0.0);
        assert!(!other.is_frozen());
        pane.reset_settings();
        assert!(pane.is_frozen());
        let later = now + Duration::from_secs(90);
        pane.toggle_freeze(&frame, later);
        assert!(!pane.is_frozen());
        assert_eq!(later - pane.paused_duration, now);
        pane.toggle_freeze(&frame, later + Duration::from_secs(1));
        assert_eq!(
            pane.frozen.as_ref().unwrap().time,
            now + Duration::from_secs(1)
        );
        pane.reset();
        assert!(!pane.is_frozen());
        assert_eq!(pane.paused_duration, Duration::ZERO);
    }

    #[test]
    fn filled_waveform_splits_zero_crossings_without_overlapping_triangles() {
        let mesh = trace_mesh(
            &[Pos2::new(0.0, -1.0), Pos2::new(2.0, 1.0)],
            0.0,
            Color32::WHITE,
        );
        let area: f32 = mesh
            .indices
            .as_chunks::<3>()
            .0
            .iter()
            .map(|triangle| {
                let a = mesh.vertices[triangle[0] as usize].pos;
                let b = mesh.vertices[triangle[1] as usize].pos;
                let c = mesh.vertices[triangle[2] as usize].pos;
                ((b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)).abs() * 0.5
            })
            .sum();
        assert_eq!(area, 1.0);
        assert!(mesh.vertices.iter().any(|v| v.pos == Pos2::new(1.0, 0.0)));
    }

    #[test]
    fn frozen_waveform_ignores_new_frames_and_live_status() {
        let context = egui::Context::default();
        let mut pane = ModulePane::new(ModuleKind::Waveform);
        let mut frame = AnalysisFrame::default();
        pane.toggle_freeze(&frame, Instant::now());
        let render = |pane: &mut ModulePane, frame: &AnalysisFrame, live| {
            let mut output = context.run_ui(egui::RawInput::default(), |ui| {
                pane.draw(
                    ui,
                    Rect::from_min_size(Pos2::ZERO, egui::vec2(500.0, 300.0)),
                    frame,
                    &AppTheme::default(),
                    live,
                );
            });
            output.textures_delta.clear();
            context
                .tessellate(output.shapes, output.pixels_per_point)
                .into_iter()
                .flat_map(|p| match p.primitive {
                    egui::epaint::Primitive::Mesh(mesh) => mesh
                        .vertices
                        .into_iter()
                        .map(|v| (v.pos, v.color))
                        .collect(),
                    _ => vec![],
                })
                .collect::<Vec<_>>()
        };
        let before = render(&mut pane, &frame, true);
        std::sync::Arc::make_mut(&mut frame.waveform_left).fill(1.0);
        std::sync::Arc::make_mut(&mut frame.waveform_right).fill(-1.0);
        let after = render(&mut pane, &frame, false);
        assert_eq!(before, after);
    }

    #[test]
    fn spectrum_keeps_measurement_guides_in_visual_inspector_and_processing_in_signals() {
        let mut pane = ModulePane::new(ModuleKind::Spectrum);
        pane.select_section(SettingsSection::Appearance);
        let ctx = egui::Context::default();
        let draw = |pane: &mut ModulePane, signal: bool| {
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(360.0, 3000.0))),
                    ..Default::default()
                },
                |ui| {
                    crate::workspace_ui::sidebar_style(ui, &AppTheme::default());
                    if signal {
                        pane.input_controls(ui, &AnalysisFrame::default());
                    } else {
                        pane.inspector_controls(ui, &AnalysisFrame::default());
                    }
                },
            );
            out.textures_delta.clear();
            out.shapes
                .into_iter()
                .filter_map(|s| match s.shape {
                    egui::Shape::Text(t) => Some(t.galley.job.text.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        draw(&mut pane, false);
        let visual = draw(&mut pane, false);
        for label in ["Peak markers", "Note labels", "Reference"] {
            assert!(visual.iter().any(|text| text == label), "missing {label}");
        }
        assert!(
            !visual
                .iter()
                .any(|text| text == "Gain dB" || text == "Time smooth")
        );
        let signal = draw(&mut pane, true);
        assert!(signal.iter().any(|text| text == "Gain dB"));
        assert!(signal.iter().any(|text| text == "Time smooth"));
        assert!(
            !signal
                .iter()
                .any(|text| text == "Peak markers" || text == "Note labels")
        );
    }

    #[test]
    fn legacy_scope_snapshots_default_new_palette_and_custom_colors_are_portable() {
        for kind in [ModuleKind::Oscilloscope, ModuleKind::Stereometer] {
            let pane = ModulePane::new(kind);
            let mut value = serde_json::to_value(pane.copy_settings()).unwrap();
            value
                .as_object_mut()
                .unwrap()
                .values_mut()
                .next()
                .unwrap()
                .as_object_mut()
                .unwrap()
                .remove("palette");
            let legacy: ModuleSettings = serde_json::from_value(value).unwrap();
            assert_eq!(pane.copy_settings(), legacy);
        }
        let mut source = ModulePane::new(ModuleKind::Flux);
        let mut value = serde_json::to_value(source.copy_settings()).unwrap();
        value["Flux"]["palette"] = serde_json::to_value(Palette::Custom(
            Palette::Aurora.gradient(&AppTheme::default()),
        ))
        .unwrap();
        let colors: ModuleSettings = serde_json::from_value(value).unwrap();
        source.paste_settings(&colors);
        let encoded = serde_json::to_vec(&source.snapshot()).unwrap();
        let saved: PaneState = serde_json::from_slice(&encoded).unwrap();
        let mut restored = ModulePane::new(ModuleKind::Flux);
        restored.restore(&saved);
        assert_eq!(restored.copy_settings(), source.copy_settings());
    }

    #[test]
    fn module_controls_fit_the_compact_sidebar_and_reset_is_always_available() {
        for kind in ModuleKind::ALL {
            let context = egui::Context::default();
            let mut pane = ModulePane::new(kind);
            for &section in pane.sections() {
                pane.select_section(section);
                let mut output = context.run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(
                            Pos2::ZERO,
                            egui::vec2(216.0, 12000.0),
                        )),
                        ..Default::default()
                    },
                    |ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
                        ui.style_mut()
                            .text_styles
                            .insert(egui::TextStyle::Body, FontId::proportional(13.0));
                        ui.style_mut()
                            .text_styles
                            .insert(egui::TextStyle::Button, FontId::proportional(13.0));
                        pane.controls(ui, &AnalysisFrame::default());
                        assert!(
                            ui.min_rect().right() <= 216.0,
                            "{kind:?}/{section:?} width {}",
                            ui.min_rect().right()
                        );
                    },
                );
                output.textures_delta.clear();
                assert!(output.shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.job.text == "Reset module settings")));
                let tab_title_count = output.shapes.iter().filter(|shape| matches!(
                    &shape.shape, egui::Shape::Text(text) if text.galley.job.text == section.title()
                )).count();
                // Amplitude is also a numeric field label. No tab gets an
                // additional heading that could collapse all of its groups.
                assert_eq!(
                    tab_title_count,
                    usize::from(section == SettingsSection::Amplitude),
                    "{kind:?}/{section:?} must show its groups directly",
                );
            }
        }
    }

    #[test]
    fn property_groups_remember_collapsed_state_without_affecting_other_panes() {
        let context = egui::Context::default();
        for theme in [egui::Theme::Dark, egui::Theme::Light] {
            context.style_mut_of(theme, |style| style.animation_time = 0.0);
        }
        let mut pane = ModulePane::new(ModuleKind::Waterfall);
        let render = |pane: &mut ModulePane, pane_index, events| {
            let mut output = context.run_ui(
                egui::RawInput {
                    events,
                    ..Default::default()
                },
                |ui| {
                    ui.push_id(pane_index, |ui| {
                        pane.controls(ui, &AnalysisFrame::default())
                    });
                },
            );
            output.textures_delta.clear();
            output
        };
        let text_rect = |output: &egui::FullOutput, label: &str| {
            output.shapes.iter().find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.job.text == label => {
                    Some(Rect::from_min_size(text.pos, text.galley.size()))
                }
                _ => None,
            })
        };
        let output = render(&mut pane, 0, vec![]);
        assert!(text_rect(&output, "Geometry").is_none());
        let header = text_rect(&output, "Dimensions").unwrap().center();
        assert!(text_rect(&output, "Length X").is_some());
        for pressed in [true, false] {
            render(
                &mut pane,
                0,
                vec![
                    egui::Event::PointerMoved(header),
                    egui::Event::PointerButton {
                        pos: header,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
        }
        let output = render(&mut pane, 0, vec![]);
        assert!(text_rect(&output, "Length X").is_none());
        assert!(text_rect(&output, "History s").is_some());
        assert!(text_rect(&output, "Reset module settings").is_some());
        pane.select_section(SettingsSection::Frequency);
        render(&mut pane, 0, vec![]);
        pane.select_section(SettingsSection::Geometry);
        assert!(text_rect(&render(&mut pane, 0, vec![]), "Length X").is_none());
        assert!(text_rect(&render(&mut pane, 1, vec![]), "Length X").is_some());
    }

    #[test]
    fn icon_tabs_show_one_section_and_preserve_per_module_and_pane_selection() {
        let context = egui::Context::default();
        let mut pane = ModulePane::new(ModuleKind::Waterfall);
        assert_eq!(pane.active_section(), SettingsSection::Geometry);
        assert_eq!(pane.sections().last(), Some(&SettingsSection::Camera));
        assert!(!pane.sections().contains(&SettingsSection::History));
        pane.select_section(SettingsSection::Camera);
        let render = |pane: &mut ModulePane| {
            let mut output = context.run_ui(egui::RawInput::default(), |ui| {
                pane.controls(ui, &AnalysisFrame::default());
            });
            output.textures_delta.clear();
            output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) => Some(text.galley.job.text.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        let labels = render(&mut pane);
        assert!(labels.iter().any(|label| label == "View presets"));
        assert!(!labels.iter().any(|label| label == "Camera"));
        assert!(!labels.iter().any(|label| label == "History s"));
        pane.select_section(SettingsSection::Geometry);
        let labels = render(&mut pane);
        assert!(labels.iter().any(|label| label == "Length X"));
        assert!(labels.iter().any(|label| label == "History s"));
        assert!(
            !labels
                .iter()
                .any(|label| label == "Camera" || label == "Auto-orbit")
        );
        pane.kind = ModuleKind::Spectrum;
        assert_eq!(pane.active_section(), SettingsSection::Frequency);
        pane.select_section(SettingsSection::Appearance);
        pane.kind = ModuleKind::Waterfall;
        assert_eq!(pane.active_section(), SettingsSection::Geometry);
        pane.kind = ModuleKind::Spectrum;
        assert_eq!(pane.active_section(), SettingsSection::Appearance);
        let other = ModulePane::new(ModuleKind::Spectrum);
        assert_eq!(other.active_section(), SettingsSection::Frequency);
        pane.kind = ModuleKind::Waveform;
        assert_eq!(
            pane.sections(),
            &[
                SettingsSection::Channels,
                SettingsSection::TimeWindow,
                SettingsSection::Amplitude,
                SettingsSection::Appearance
            ]
        );
    }

    #[test]
    fn heatmap_spans_cool_to_warm_without_theme_tinting() {
        let theme = AppTheme::default();
        let palette = Palette::Heatmap;
        let quiet = palette.color(0.0, &theme);
        let middle = palette.color(0.5, &theme);
        let loud = palette.color(1.0, &theme);
        assert!(quiet.b() > quiet.r() && quiet.b() > quiet.g());
        assert!(middle.g() > middle.r() && middle.g() > middle.b());
        assert!(loud.r() > loud.g() && loud.r() > loud.b());
        assert_eq!(palette.color(-1.0, &theme), quiet);
        assert_eq!(palette.color(2.0, &theme), loud);
        let other_theme = AppTheme {
            background: Color32::WHITE,
            foreground: Color32::BLACK,
            ..theme.clone()
        };
        let mut colors = std::collections::HashSet::new();
        for step in 0..=255 {
            let level = step as f32 / 255.0;
            let color = palette.color(level, &theme);
            colors.insert(color.to_array());
            assert_eq!(color, palette.color(level, &other_theme));
            if step > 0 {
                let previous = palette.color((step - 1) as f32 / 255.0, &theme);
                for (a, b) in color.to_array().iter().zip(previous.to_array()) {
                    assert!(a.abs_diff(b) <= 6, "smooth gradient between color stops");
                }
            }
        }
        assert!(colors.len() > 250);
    }

    #[test]
    fn heatmap_contrast_preserves_endpoints_midpoint_and_other_palettes() {
        let theme = AppTheme::default();
        for contrast in [0.25, 1.0, 3.0] {
            for t in [0.0, 0.5, 1.0] {
                assert_eq!(
                    Palette::Heatmap.color_with_contrast(t, contrast, &theme),
                    Palette::Heatmap.color(t, &theme)
                );
            }
            for palette in [Palette::Theme, Palette::Ember, Palette::Ocean] {
                assert_eq!(
                    palette.color_with_contrast(0.3, contrast, &theme),
                    palette.color(0.3, &theme)
                );
            }
        }
        let cool = Palette::Heatmap.color_with_contrast(0.25, 3.0, &theme);
        let warm = Palette::Heatmap.color_with_contrast(0.75, 3.0, &theme);
        assert!(cool.b() > cool.g());
        assert!(warm.r() > warm.g());
        for step in 0..=100 {
            let t = step as f32 / 100.0;
            assert_eq!(
                Palette::Heatmap.color_with_contrast(t, 1.0, &theme),
                Palette::Heatmap.color(t, &theme)
            );
        }
    }
    #[test]
    fn every_component_starts_with_twelve_empty_macro_slots_in_a_single_row() {
        for kind in ModuleKind::ALL {
            for width in [600.0, 1400.0] {
                let ctx = egui::Context::default();
                let theme = AppTheme::default();
                let mut pane = ModulePane::new(kind);
                assert!(pane.macros.open);
                assert!(
                    pane.macros.banks[kind as usize]
                        .iter()
                        .all(|s| s.parameter.is_none())
                );
                let rect = Rect::from_min_size(
                    Pos2::ZERO,
                    egui::vec2(width, pane.performance_height(width).unwrap()),
                );
                let mut out = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(
                            Pos2::ZERO,
                            egui::vec2(width, 300.0),
                        )),
                        ..Default::default()
                    },
                    |ui| {
                        pane.performance_bar(ui, rect, &theme, 0);
                    },
                );
                out.textures_delta.clear();
                let rows: Vec<_> = out
                    .shapes
                    .iter()
                    .filter_map(|s| match &s.shape {
                        egui::Shape::Text(t) if t.galley.job.text.starts_with("Macro ") => {
                            Some(t.pos.y)
                        }
                        _ => None,
                    })
                    .collect();
                let names: std::collections::HashSet<_> = out
                    .shapes
                    .iter()
                    .filter_map(|s| match &s.shape {
                        egui::Shape::Text(t) if t.galley.job.text.starts_with("Macro ") => {
                            Some(t.galley.job.text.as_str())
                        }
                        _ => None,
                    })
                    .collect();
                assert_eq!(names.len(), 12);
                assert!(rows.iter().all(|y| (*y - rows[0]).abs() < 0.01));
                assert!(!out.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Text(t) if ["Hide", "Advanced", "Macros"].contains(&t.galley.job.text.as_str()) || t.galley.job.text.starts_with("Drag parameter labels"))));
                pane.toggle_performance();
                assert!(pane.performance_height(width).is_none());
                pane.toggle_performance();
                assert!(pane.performance_height(width).is_some());
            }
        }
    }
}
