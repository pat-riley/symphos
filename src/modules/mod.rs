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

mod camera_gizmo;
mod cymatics;
mod flux;
mod frequency;
mod milkdrop;
mod orbital;
mod oscilloscope;
mod particle_waves;
pub(crate) mod performance;
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
            Self::Resonance => "3D Resonance",
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
            Self::Resonance => "Layered petal wavefronts",
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
                "Translucent, layered petal shells inspired by radiation wavefronts. Adjust layers, spacing, lobes, twist and ripples; audio expands and illuminates the form. An artistic primitive, not a physical simulation. Drag to rotate freely; Shift/right-drag to pan."
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
    /// Auto-orbit advances presentation state without a user edit.
    pub fn automatic_yaw(&mut self) -> Option<&mut f32> {
        match self {
            Self::Waterfall(s) => s.automatic_yaw(),
            Self::Sphere(s) | Self::Cylinder(s) => s.automatic_yaw(),
            Self::Orbital(s) => s.automatic_yaw(),
            Self::ParticleWaves(s) | Self::SpectralBloom(s) => s.automatic_yaw(),
            _ => None,
        }
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

impl SettingsSection {
    fn icon(self) -> Icon {
        match self {
            Self::Camera => Icon::Camera,
            Self::Appearance => Icon::Palette,
            Self::History | Self::TimeWindow => Icon::Clock,
            Self::Geometry => Icon::Waterfall,
            Self::Frequency => Icon::Bars,
            Self::Response | Self::Amplitude => Icon::Settings,
            Self::Modulation => Icon::AudioMappings,
            Self::Channels | Self::Correlation => Icon::Stereo,
            Self::Display => Icon::Surface,
            Self::Sync => Icon::Oscilloscope,
            Self::Flow => Icon::Waveform,
            Self::Particles => Icon::Flux,
            Self::Orbs => Icon::Sphere,
            Self::Rendering => Icon::Palette,
            Self::Presets => Icon::Surface,
            Self::Feedback => Icon::Rings,
        }
    }
}

impl ModulePane {
    /// Source lifecycle is owned by Signals. Reconcile every retained visualizer,
    /// including inactive types, without creating or rebinding any channels.
    pub fn reconcile_signals(&mut self, bank: &crate::signals::Bank) {
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
        let before = self.snapshot();
        for saved in &state.settings {
            let mut settings = saved.clone();
            if let Some(old) = before.settings.iter().find(|s| s.kind() == settings.kind()) {
                let mut old = old.clone();
                if let (Some(current), Some(restored)) =
                    (old.automatic_yaw(), settings.automatic_yaw())
                {
                    *restored = *current;
                }
            }
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

    pub fn new(kind: ModuleKind) -> Self {
        Self {
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

    pub fn inspector_tabs(&mut self, ui: &mut egui::Ui) {
        if let Some(section) = self.inspector_focus.take() {
            self.select_section(section);
        }
        for &section in self.sections() {
            let selected = self.active_section() == section;
            let response =
                crate::icons::sized_button(ui, section.icon(), selected, 28.0, section.title());
            response.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::SelectableLabel, true, section.title())
            });
            if response.clone().help_text(section.title()).clicked() {
                self.select_section(section);
            }
        }
    }

    pub fn inspector_controls(&mut self, ui: &mut egui::Ui, frame: &AnalysisFrame) {
        let section = self.active_section();
        ui.data_mut(|d| d.insert_temp(egui::Id::new("property-row-index"), 0_usize));
        ui.push_id(self.kind.label(), |ui| {
            inspector_section(ui, section.title(), |ui| {
                ui.data_mut(|d| d.insert_temp(ui.id().with("active-module-section"), section));
                self.section_controls(ui, frame);
            });
        });
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

    pub fn draw(
        &mut self,
        ui: &mut egui::Ui,
        rect: Rect,
        frame: &AnalysisFrame,
        theme: &AppTheme,
        live: bool,
    ) {
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
        ui.painter().rect_filled(rect, 4.0, theme.background);
        ui.interact(rect, ui.id().with("module-help"), egui::Sense::hover())
            .help_text(self.kind.description());
        match self.kind {
            ModuleKind::Milkdrop => self.milkdrop.draw(ui, rect, frame, now, live, frozen),
            ModuleKind::Cymatics => self.cymatics.draw(ui, rect, frame, theme, now, live),
            ModuleKind::SpectralBloom => {
                self.spectral_bloom.draw(ui, rect, frame, theme, now, live)
            }
            ModuleKind::ParticleWaves => {
                self.particle_waves.draw(ui, rect, frame, theme, now, live)
            }
            ModuleKind::Orbital => self.orbital.draw(ui, rect, frame, theme, now, live),
            ModuleKind::Resonance => self.resonance.draw(ui, rect, frame, theme, now, live),
            ModuleKind::Rings => self.rings.draw(ui, rect, frame, theme, now, live),
            ModuleKind::Cylinder => self.cylinder.draw(ui, rect, frame, theme, now),
            ModuleKind::Sphere => self.sphere.draw(ui, rect, frame, theme, now),
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
            ModuleKind::Strata => self.strata.draw(ui, rect, frame, theme, now, live),
            ModuleKind::Flux => self.flux.draw(ui, rect, frame, theme, now, live),
        }
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
fn properties_combo(id: &str, width: f32, rows: usize) -> egui::ComboBox {
    egui::ComboBox::from_id_salt(id)
        .width(width)
        .height(rows as f32 * 26.0)
        .truncate()
        .popup_style(egui::style::StyleModifier::new(|style| {
            style.spacing.item_spacing.y = 2.0;
            style.spacing.interact_size.y = 24.0;
            style.spacing.button_padding = egui::vec2(5.0, 2.0);
            style.override_font_id = Some(FontId::proportional(13.0));
        }))
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
        properties_combo("trace-style", 176.0, 3)
            .selected_text(self.label())
            .show_ui(ui, |ui| {
                for style in [Self::Bars, Self::Line, Self::Filled] {
                    if style == Self::Bars && !bars {
                        continue;
                    }
                    let (rect, response) =
                        ui.allocate_exact_size(egui::vec2(176.0, 24.0), egui::Sense::click());
                    if response.hovered() || *self == style {
                        ui.painter()
                            .rect_filled(rect, 3.0, ui.visuals().selection.bg_fill);
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
                        FontId::proportional(13.0),
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
    let selected =
        ui.data(|data| data.get_temp::<SettingsSection>(ui.id().with("active-module-section")));
    if selected.is_some_and(|section| section.title() != title) {
        return;
    }
    // Render the requested section inside its inspector disclosure.
    ui.push_id(title, body);
}

/// The inspector's outer bounds allow rules to span padding and the scroll gutter.
pub(crate) fn inspector_bounds(ui: &egui::Ui) -> Option<Rect> {
    ui.data(|data| data.get_temp::<Rect>(egui::Id::new("inspector-bounds")))
}

pub(crate) fn inspector_rule(ui: &mut egui::Ui) {
    crate::workspace_ui::sidebar_rule(ui, inspector_bounds(ui).unwrap_or(ui.max_rect()));
}

/// Shared active-section framing for either inspector: rules reach the pane edges.
pub(crate) fn inspector_section(ui: &mut egui::Ui, title: &str, body: impl FnOnce(&mut egui::Ui)) {
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
    if inspector_bounds(ui).is_some() {
        ui.add_space(3.0);
        inspector_rule(ui);
        egui::CollapsingHeader::new(egui::RichText::new(title).size(12.0).strong())
            .default_open(open)
            .show_unindented(ui, |ui| {
                ui.push_id(title, body);
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
    if inspector_bounds(ui).is_none() {
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

fn mix(a: Color32, b: Color32, amount: f32) -> Color32 {
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
    fn icon_tabs_select_sections_and_all_properties_fit_compact_width() {
        for kind in ModuleKind::ALL {
            let context = egui::Context::default();
            let mut pane = ModulePane::new(kind);
            let render = |pane: &mut ModulePane, events| {
                let mut output = context.run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(
                            Pos2::ZERO,
                            egui::vec2(334.0, 2400.0),
                        )),
                        events,
                        ..Default::default()
                    },
                    |ui| {
                        crate::workspace_ui::sidebar_style(ui, &AppTheme::default());
                        let mut rail =
                            ui.new_child(egui::UiBuilder::new().id_salt("rail").max_rect(
                                Rect::from_min_size(egui::pos2(3.0, 4.0), egui::vec2(28.0, 2000.0)),
                            ));
                        rail.spacing_mut().button_padding = egui::vec2(2.0, 2.0);
                        rail.spacing_mut().interact_size = egui::vec2(28.0, 28.0);
                        pane.inspector_tabs(&mut rail);
                        let bounds =
                            Rect::from_min_size(egui::pos2(34.0, 0.0), egui::vec2(300.0, 2400.0));
                        let mut body =
                            ui.new_child(egui::UiBuilder::new().id_salt("body").max_rect(bounds));
                        body.data_mut(|d| d.insert_temp(egui::Id::new("inspector-bounds"), bounds));
                        pane.inspector_controls(&mut body, &AnalysisFrame::default());
                        assert!(
                            body.min_rect().width() <= 301.0,
                            "{kind:?} / {:?} fits",
                            pane.active_section()
                        );
                    },
                );
                output.textures_delta.clear();
                output
            };
            render(&mut pane, vec![]);
            render(&mut pane, vec![]);
            let last = *pane.sections().last().unwrap();
            let pos = egui::pos2(17.0, 18.0 + (pane.sections().len() - 1) as f32 * 31.0);
            for pressed in [true, false] {
                render(
                    &mut pane,
                    vec![
                        egui::Event::PointerMoved(pos),
                        egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                );
            }
            assert_eq!(pane.active_section(), last, "{kind:?} tab click");
            let restored = pane.snapshot();
            let mut other = ModulePane::new(kind);
            other.restore(&restored);
            assert_eq!(other.active_section(), last);
            for &section in pane.sections() {
                pane.select_section(section);
                let output = render(&mut pane, vec![]);
                assert!(output.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Text(t) if t.galley.job.text==section.title())));
            }
        }
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
                            egui::vec2(216.0, 1000.0),
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
