//! A bounded, fixed-step particle field with music-driven attraction and flow.
use super::{camera_gizmo::project_direction, mix, properties_combo, settings_panel};
use crate::modulation::{ModulationSettings, Modulator, Target, Visualizer};
use crate::{analysis::AnalysisFrame, help::HoverHelp, parameter::Parameter, theme::AppTheme};
use eframe::egui::{self, Color32, Pos2, Rect, Vec2};
use std::{f32::consts::TAU, time::Instant};

mod performance;
use performance::{Macro, Performance, orbit_position};

const STEP: f32 = 1.0 / 120.0;
use crate::colors::Palette;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum ColorMode {
    Current,
    Velocity,
    Frequency,
    Depth,
}
impl ColorMode {
    fn label(self) -> &'static str {
        match self {
            Self::Current => "By current",
            Self::Velocity => "By velocity",
            Self::Frequency => "By audio band",
            Self::Depth => "By depth",
        }
    }
}
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub(crate) struct FluxSettings {
    camera_motion: super::camera_motion::Settings,
    show_gizmo: bool,
    show_grid: bool,
    pan_x: f32,
    pan_y: f32,
    #[serde(default)]
    pub(crate) modulation: Option<ModulationSettings>,
    brightness: f32,
    expansion: f32,
    impact: f32,
    orbit_shape: f32,
    color_drift: f32,
    performance_open: bool,
    count: u32,
    cores: u32,
    seed: u32,
    radius: f32,
    depth: f32,
    speed: f32,
    attraction: f32,
    orbit_magnetism: f32,
    swirl: f32,
    turbulence: f32,
    drag: f32,
    diffusion: f32,
    core_motion: f32,
    gravity: f32,
    sensitivity: f32,
    gate_db: f32,
    attack_ms: f32,
    release_ms: f32,
    bass_push: f32,
    mid_flow: f32,
    high_scatter: f32,
    stereo: f32,
    transients: f32,
    size: f32,
    size_variation: f32,
    opacity: f32,
    trails: f32,
    glow: f32,
    twinkle: f32,
    show_cores: bool,
    core_size: f32,
    palette: Palette,
    color_mode: ColorMode,
    saturation: f32,
    depth_fade: f32,
    yaw: f32,
    elevation: f32,
    roll: f32,
    zoom: f32,
    perspective: f32,
    auto_orbit: bool,
    orbit_speed: f32,
    labels: bool,
}
impl Default for FluxSettings {
    fn default() -> Self {
        Self {
            camera_motion: Default::default(),
            show_gizmo: false,
            show_grid: false,
            pan_x: 0.0,
            pan_y: 0.0,
            modulation: Some(ModulationSettings::for_visualizer(Visualizer::Flux)),
            brightness: 1.0,
            expansion: 1.0,
            impact: 0.65,
            orbit_shape: 0.0,
            color_drift: 0.10,
            performance_open: true,
            count: 6000,
            cores: 5,
            seed: 41,
            radius: 1.0,
            depth: 1.0,
            speed: 1.0,
            attraction: 1.1,
            orbit_magnetism: 0.0,
            swirl: 1.2,
            turbulence: 0.65,
            drag: 0.85,
            diffusion: 0.3,
            core_motion: 0.6,
            gravity: 0.0,
            sensitivity: 1.3,
            gate_db: -66.0,
            attack_ms: 40.0,
            release_ms: 600.0,
            bass_push: 1.1,
            mid_flow: 0.8,
            high_scatter: 0.7,
            stereo: 0.35,
            transients: 1.2,
            size: 1.0,
            size_variation: 0.65,
            opacity: 0.72,
            trails: 0.45,
            glow: 0.45,
            twinkle: 0.35,
            show_cores: true,
            core_size: 3.0,
            palette: Palette::Spectrum,
            color_mode: ColorMode::Current,
            saturation: 0.9,
            depth_fade: 0.5,
            yaw: -25.0,
            elevation: 18.0,
            roll: 0.0,
            zoom: 1.0,
            perspective: 0.45,
            auto_orbit: true,
            orbit_speed: 3.0,
            labels: true,
        }
    }
}
impl FluxSettings {
    fn sanitize(&mut self) {
        self.count = self.count.clamp(200, 20_000);
        self.cores = self.cores.clamp(1, 8);
        self.seed = self.seed.min(9999);
        let defaults = Self::default();
        macro_rules! bound { ($($field:ident: $lo:expr, $hi:expr);* $(;)?) => {$(
            self.$field = if self.$field.is_finite() { self.$field.clamp($lo, $hi) } else { defaults.$field };
        )*}; }
        bound! {
            brightness: 0.0, 2.0; expansion: 0.35, 2.0; impact: 0.0, 2.0; orbit_shape: 0.0, 1.0; color_drift: 0.0, 1.0;
            radius: 0.4, 2.0; depth: 0.2, 2.0; speed: 0.0, 2.0;
            attraction: 0.0, 3.0; orbit_magnetism: 0.0, 1.0; swirl: -3.0, 3.0; turbulence: 0.0, 2.0; drag: 0.1, 3.0;
            diffusion: 0.0, 1.0; core_motion: 0.0, 2.0; gravity: -1.0, 1.0;
            sensitivity: 0.0, 4.0; gate_db: -90.0, -20.0; attack_ms: 5.0, 500.0; release_ms: 50.0, 3000.0;
            bass_push: 0.0, 3.0; mid_flow: 0.0, 3.0; high_scatter: 0.0, 3.0; stereo: 0.0, 2.0; transients: 0.0, 3.0;
            size: 0.3, 3.0; size_variation: 0.0, 1.0; opacity: 0.0, 1.0; trails: 0.0, 1.0;
            glow: 0.0, 1.0; twinkle: 0.0, 1.0; core_size: 1.0, 8.0;
            saturation: 0.0, 1.0; depth_fade: 0.0, 1.0;
            yaw: -180.0, 180.0; elevation: -180.0, 180.0; roll: -180.0, 180.0; zoom: 0.35, 3.0;
            perspective: 0.0, 1.0; orbit_speed: -15.0, 15.0;
        }
        let modulation = self.modulation.get_or_insert_with(|| {
            let mut m = ModulationSettings::migrate(
                Visualizer::Flux,
                self.sensitivity,
                self.gate_db,
                self.attack_ms,
                self.release_ms,
            );
            for r in &mut m.routes {
                match r.target {
                    Target::Expansion => r.amount = self.bass_push,
                    Target::Swirl => r.amount = self.mid_flow,
                    Target::Turbulence => r.amount = self.high_scatter,
                    Target::Steering => r.amount = self.stereo,
                    _ => {}
                }
            }
            m
        });
        modulation.sanitize(Visualizer::Flux);
    }
}
#[derive(Clone, Debug, PartialEq)]
struct Particle {
    position: [f32; 3],
    velocity: [f32; 3],
    variation: f32,
    home: [f32; 3],
    group: usize,
}
#[derive(Default)]
pub struct Flux {
    modulator: Modulator,
    performance: Performance,
    color_phase: f64,
    pub(crate) settings: FluxSettings,
    particles: Vec<Particle>,
    pattern: Option<(u32, u32)>,
    last_time: Option<Instant>,
    accumulator: f32,
    phase: f64,
    flow_phase: f64,
    orbit: f64,
    energy: [f32; 4],
    balance: f32,
}
impl Flux {
    pub(crate) fn settings_snapshot(&self) -> FluxSettings {
        self.settings.clone()
    }
    pub(crate) fn apply_settings(&mut self, settings: &FluxSettings) {
        self.modulator = Modulator::default();
        self.performance = Performance::default();
        self.settings = settings.clone();
        self.settings.sanitize();
    }
    pub(crate) fn resume_signals(&mut self, frame: &AnalysisFrame) {
        if let Some(s) = &self.settings.modulation {
            self.modulator.consume_current_events(frame, s);
        }
    }
    pub fn clear(&mut self) {
        self.modulator = Modulator::default();
        self.performance = Performance::default();
        self.particles.clear();
        self.pattern = None;
        self.last_time = None;
        self.accumulator = 0.0;
        self.phase = 0.0;
        self.color_phase = 0.0;
        self.flow_phase = 0.0;
        self.orbit = 0.0;
        self.energy = [0.0; 4];
        self.balance = 0.0;
    }
    pub fn reset_settings(&mut self) {
        self.settings = FluxSettings::default();
        self.clear();
    }
    fn reset_camera(&mut self) {
        self.settings.auto_orbit = false;
        self.settings.pan_x = 0.0;
        self.settings.pan_y = 0.0;
        self.settings.roll = 0.0;
        let d = FluxSettings::default();
        self.settings.yaw = d.yaw;
        self.settings.elevation = d.elevation;
        self.settings.roll = 0.0;
        self.settings.zoom = 1.0;
        self.orbit = 0.0;
    }
    pub fn controls(&mut self, ui: &mut egui::Ui, frame: &AnalysisFrame) {
        self.settings.sanitize();
        let s = &mut self.settings;
        macro_rules! p {
            ($ui:ident, $field:ident, $range:expr, $label:expr, $help:expr) => {
                $ui.add(
                    Parameter::new(&mut s.$field, $range, FluxSettings::default().$field)
                        .key(stringify!($field))
                        .text($label),
                )
                .help_text($help);
            };
        }
        settings_panel(ui, "Geometry", |ui| {
            p!(
                ui,
                count,
                200..=20_000,
                "Particle count",
                "Number of particles in the 3D cloud. More particles and glow increase rendering cost."
            );
            p!(
                ui,
                cores,
                1..=8,
                "Attraction points",
                "Number of moving centers that gather particles into separate currents."
            );
            p!(
                ui,
                seed,
                0..=9999,
                "Pattern seed",
                "Repeatable initial particle distribution. Changing it restarts the cloud."
            );
            p!(
                ui,
                radius,
                0.4..=2.0,
                "Cloud spread",
                "Extent of the attractor field; particles flow toward the new arrangement."
            );
            p!(
                ui,
                depth,
                0.2..=2.0,
                "Depth stretch",
                "Compress the cloud into a sheet or extend its third dimension."
            );
        });
        settings_panel(ui, "Flow & Motion", |ui| {
            p!(
                ui,
                speed,
                0.0..=2.0,
                "Simulation speed",
                "Rate of particle travel. Zero holds particle positions; auto orbit remains independent."
            );
            p!(
                ui,
                expansion,
                0.35..=2.0,
                "Base expansion",
                "Resting extent of the formation; music adds expansion on top."
            );
            p!(
                ui,
                impact,
                0.0..=2.0,
                "Impact",
                "Transient emphasis: rising mapped energy adds short expansion, brightness, and momentum accents."
            );
            p!(
                ui,
                orbit_shape,
                0.0..=1.0,
                "Orbit shape",
                "Morph Rings / Helix / Knot / Vortex / Petals. Raise Orbit magnetism to reveal the path."
            );
            p!(
                ui,
                orbit_magnetism,
                0.0..=1.0,
                "Orbit magnetism",
                "Zero keeps particles in a dispersed, moving 3D cloud. Raise this to gather them into orbital bands; one tightly binds the bands. Audio still expands and disturbs both formations."
            );
            p!(
                ui,
                attraction,
                0.0..=3.0,
                "Attraction",
                "How firmly particles follow their moving cloud or orbital paths. Low values drift loosely; high values track the field more closely."
            );
            p!(
                ui,
                swirl,
                -3.0..=3.0,
                "Swirl",
                "Circulation through the cloud or around magnetized bands. Negative values reverse its direction."
            );
            p!(
                ui,
                turbulence,
                0.0..=2.0,
                "Turbulence",
                "Spatially varying currents that bend streams into filaments."
            );
            p!(
                ui,
                drag,
                0.1..=3.0,
                "Damping",
                "Velocity friction. Higher values make the cloud smoother and slower."
            );
            p!(
                ui,
                diffusion,
                0.0..=1.0,
                "Diffusion",
                "Amount of individual wandering around each particle’s path through the cloud."
            );
            p!(
                ui,
                core_motion,
                0.0..=2.0,
                "Core movement",
                "Travel of the attraction points through the cloud."
            );
            p!(
                ui,
                gravity,
                -1.0..=1.0,
                "Vertical pull",
                "Bias the field upward or downward while keeping it bounded."
            );
        });
        settings_panel(ui, "Audio mappings", |ui| {
            self.modulator.editor(
                ui,
                frame,
                s.modulation.as_mut().unwrap(),
                Visualizer::Flux,
                1,
            );
        });
        settings_panel(ui, "Particles", |ui| {
            p!(
                ui,
                size,
                0.3..=3.0,
                "Point size",
                "Base particle radius in screen points; perspective also scales it with depth."
            );
            p!(
                ui,
                size_variation,
                0.0..=1.0,
                "Size variation",
                "Mix fine dust with a few larger particles."
            );
            p!(
                ui,
                opacity,
                0.0..=1.0,
                "Opacity",
                "Visibility of the cloud."
            );
            p!(
                ui,
                trails,
                0.0..=1.0,
                "Trail length",
                "Short velocity streaks reveal the direction of each current."
            );
            p!(
                ui,
                glow,
                0.0..=1.0,
                "Glow",
                "Soft halos around the particles and cores."
            );
            p!(
                ui,
                twinkle,
                0.0..=1.0,
                "Sparkle",
                "Individual brightness variation, intensified by highs."
            );
            ui.checkbox(&mut s.show_cores, "Show attraction points");
            p!(
                ui,
                core_size,
                1.0..=8.0,
                "Core size",
                "Radius of the luminous markers at each attraction point."
            );
        });
        settings_panel(ui, "Appearance", |ui| {
            p!(
                ui,
                brightness,
                0.0..=2.0,
                "Brightness",
                "Master visual light level, separate from audio volume and particle opacity."
            );
            p!(
                ui,
                color_drift,
                0.0..=1.0,
                "Color drift",
                "Rate of continuous movement through the selected palette."
            );
            s.palette.controls(ui);
            super::property_row(ui, "Color by", |ui| {
                properties_combo("flux-color-mode", ui.available_width(), 4).selected_text(s.color_mode.label()).show_ui(ui, |ui| {
                for mode in [ColorMode::Current, ColorMode::Velocity, ColorMode::Frequency, ColorMode::Depth] { ui.selectable_value(&mut s.color_mode, mode, mode.label()); }
            }).response.help_text("Map color to current membership, speed, audio bands, or distance from the camera.");
            });
            p!(
                ui,
                saturation,
                0.0..=1.0,
                "Saturation",
                "Blend the palette toward a neutral pearl color."
            );
            p!(
                ui,
                depth_fade,
                0.0..=1.0,
                "Depth fade",
                "Dim distant particles to emphasize the cloud's volume."
            );
            ui.checkbox(&mut s.labels, "Show navigation hints");
        });
        settings_panel(ui, "Camera", |ui| {
            super::settings_group(ui, "Framing", |ui| {
                for (key, name, value) in [
                    ("pan_x", "Pan X", &mut s.pan_x),
                    ("pan_y", "Pan Y", &mut s.pan_y),
                ] {
                    ui.add(
                        Parameter::new(value, -2.0..=2.0, 0.0)
                            .bounds(-4.0..=4.0)
                            .key(key)
                            .text(name),
                    );
                }
                ui.checkbox(&mut s.show_grid, "XYZ reference grid");
            });

            p!(
                ui,
                yaw,
                -180.0..=180.0,
                "Rotation °",
                "Horizontal camera orbit; drag the scene to rotate directly."
            );
            p!(
                ui,
                elevation,
                -85.0..=85.0,
                "Elevation °",
                "Vertical viewing angle."
            );
            p!(
                ui,
                roll,
                -180.0..=180.0,
                "Roll °",
                "Rotate the image plane; dragging near the viewport edge also adjusts roll."
            );
            p!(
                ui,
                zoom,
                0.35..=3.0,
                "Zoom",
                "Magnification; scroll over the scene to adjust."
            );
            p!(
                ui,
                perspective,
                0.0..=1.0,
                "Perspective",
                "Zero is orthographic; higher values emphasize near/far size differences."
            );
            if super::visual_inspector(ui).is_none() {
                ui.checkbox(&mut s.auto_orbit, "Auto orbit").help_text(
                "Rotate the view slowly. Manual dragging stops auto orbit at the current angle.",
            );
                p!(
                    ui,
                    orbit_speed,
                    -15.0..=15.0,
                    "Orbit °/s",
                    "Automatic rotation speed and direction."
                );
            }
        });
    }

    fn centers(&self) -> Vec<[f32; 3]> {
        let s = &self.settings;
        (0..s.cores)
            .map(|i| {
                let a = i as f64 / f64::from(s.cores) * std::f64::consts::TAU;
                let t = self.phase * f64::from(s.core_motion);
                let r = s.radius * if s.cores == 1 { 0.2 } else { 0.85 };
                [
                    ((a + t * 0.21).cos() as f32 + 0.2 * (a * 2.0 - t * 0.31).sin() as f32) * r,
                    (a * 1.7 + t * 0.17).sin() as f32 * r * 0.65,
                    (a + t * 0.23).sin() as f32 * r * 0.7,
                ]
            })
            .collect()
    }
    fn ensure_particles(&mut self) {
        let key = (self.settings.seed, self.settings.cores);
        if self.pattern != Some(key) {
            self.particles.clear();
            self.pattern = Some(key);
        }
        self.particles.truncate(self.settings.count as usize);
        let centers = self.centers();
        for i in self.particles.len() as u32..self.settings.count {
            let group = i as usize % centers.len();
            let h = |salt| random(i.wrapping_add(self.settings.seed.wrapping_mul(7919)), salt);
            let direction = sphere(h(1), h(2));
            let distance = h(3).cbrt() * self.settings.radius * 0.9;
            self.particles.push(Particle {
                position: std::array::from_fn(|axis| {
                    centers[group][axis] + direction[axis] * distance
                }),
                velocity: [
                    -direction[2] * 0.15,
                    direction[0] * 0.1,
                    direction[1] * 0.15,
                ],
                variation: h(4),
                home: direction.map(|v| v * h(3).cbrt()),
                group,
            });
        }
    }
    fn advance(&mut self, now: Instant, frame: &AnalysisFrame, live: bool) {
        if self.settings.modulation.is_none() {
            self.settings.sanitize();
        }
        self.ensure_particles();
        let dt = self.last_time.map_or(0.0, |last| {
            now.saturating_duration_since(last).as_secs_f32().min(0.05)
        });
        self.last_time = Some(now);
        if dt == 0.0 {
            return;
        }
        self.accumulator += dt;
        // At most six 120 Hz steps per frame after a stall, never an unbounded catch-up.
        while self.accumulator + 1e-7 >= STEP {
            self.accumulator = (self.accumulator - STEP).max(0.0);
            self.modulator.advance(
                frame,
                live,
                self.settings.modulation.as_ref().unwrap(),
                STEP,
                1,
            );
            self.energy = [
                Target::Expansion,
                Target::Swirl,
                Target::Turbulence,
                Target::Brightness,
            ]
            .map(|t| self.modulator.value(t));
            self.balance = self.modulator.value(Target::Steering);
            let input = self
                .modulator
                .input_for(
                    Target::Expansion,
                    self.settings.modulation.as_ref().unwrap(),
                )
                .max(
                    self.modulator.input_for(
                        Target::Turbulence,
                        self.settings.modulation.as_ref().unwrap(),
                    ) * 0.55,
                );
            for m in performance::Macro::ALL {
                self.performance.modulation[m as usize] = self.modulator.value(m.target().unwrap());
            }
            self.performance.impact_event = self.modulator.event_strength;
            self.performance.modulation[performance::Macro::Impact as usize] =
                self.modulator.value(Target::Impact);
            self.performance.advance(&self.settings, input, STEP);

            if self.settings.auto_orbit {
                self.orbit =
                    (self.orbit + f64::from(STEP * self.settings.orbit_speed)).rem_euclid(360.0);
            }
            // Force mappings can wake a still cloud even without a separate speed route.
            let movement = self.energy[..3]
                .iter()
                .map(|v| v.abs())
                .fold(self.balance.abs(), f32::max);
            self.step(
                STEP * self.performance.value(&self.settings, Macro::Speed)
                    * (1.0
                        + self.performance.pulse
                            * self.performance.value(&self.settings, Macro::Impact)
                            * 0.35)
                    * self
                        .settings
                        .modulation
                        .as_ref()
                        .unwrap()
                        .ambient
                        .max(movement)
                        .clamp(0.0, 3.0),
            );
        }
    }
    fn step(&mut self, dt: f32) {
        if dt == 0.0 {
            return;
        }
        self.phase += f64::from(dt);
        let centers = self.centers();
        let s = &self.settings;
        let t = self.phase.rem_euclid(f64::from(TAU) * 1000.0) as f32;
        let magnetism = self.performance.value(s, Macro::Magnetism);
        let impact = self.performance.value(s, Macro::Impact) * self.performance.pulse;
        let base_expansion = self.performance.value(s, Macro::Expansion);
        let shape = self.performance.value(s, Macro::Shape);
        self.color_phase += f64::from(dt * self.performance.value(s, Macro::Color) * 0.18);
        let expansion = (base_expansion * (1.0 + impact * 0.5)).clamp(0.2, 2.6);
        let turbulence = self.performance.value(s, Macro::Turbulence).clamp(0.0, 4.0);
        let swirl = self.performance.value(s, Macro::Swirl).clamp(-8.0, 8.0);
        self.flow_phase += f64::from(dt * swirl * 0.45);
        let flow = self.flow_phase.rem_euclid(f64::from(TAU)) as f32;
        let damping = (-dt * (s.drag + magnetism * 1.5)).exp();
        let tracking = 0.9 + s.attraction * 1.2 + magnetism * 3.0;
        let kick = self.performance.impulse * self.performance.value(s, Macro::Impact);
        for particle in &mut self.particles {
            let p = particle.position;
            let center = centers[particle.group];
            let individual = particle.variation * TAU;
            // Persistent, volume-distributed homes prevent damped particles from
            // converging on the same attractor or a common radial equilibrium.
            // Two rotating axes and individual rates retain depth as time passes.
            let a = flow + t * (0.11 + particle.variation * 0.13) + individual;
            let b = t * (0.17 + particle.home[0].abs() * 0.19) + individual * 2.3;
            let (sa, ca) = a.sin_cos();
            let (sb, cb) = b.sin_cos();
            let [hx, hy, hz] = particle.home;
            let x = hx * ca - hz * sa;
            let z = hx * sa + hz * ca;
            let cloud = [x, hy * cb - z * sb, hy * sb + z * cb];
            let angle = flow + individual;
            let path = if magnetism > 0.0001 {
                orbit_position(shape, angle, particle.home)
            } else {
                [0.0; 3]
            };
            let ring = match particle.group % 3 {
                0 => path,
                1 => [path[1], path[0], path[2]],
                _ => [path[0], path[2], path[1]],
            };
            let radial: [f32; 3] = std::array::from_fn(|k| p[k] - center[k]);
            let distance = length(radial).max(s.radius * 0.05);
            for k in 0..3 {
                let wander = (t * (0.47 + particle.variation * 0.31)
                    + individual * (k as f32 + 1.7)
                    + particle.home[(k + 1) % 3] * 4.0)
                    .sin();
                let eddy = (t * 1.1 + particle.home[k] * 7.0 + individual + k as f32 * 2.1).cos();
                let cloud_offset = cloud[k] * expansion * 0.95
                    + wander * (0.18 + s.diffusion * 0.35)
                    + eddy * turbulence * 0.18;
                let ring_offset = ring[k] * expansion + eddy * turbulence * 0.018;
                let target = center[k]
                    + s.radius * (cloud_offset * (1.0 - magnetism) + ring_offset * magnetism);
                // Track a bounded moving volume, rather than continuously injecting
                // tangential acceleration that flings everything into planar rings.
                let mut force = (target - p[k]) * tracking;
                if k == 0 {
                    force += self.balance * s.radius * 0.3;
                }
                if k == 1 {
                    force += s.gravity * s.radius;
                }
                force -= p[k] * (length(p) / s.radius - 2.1).max(0.0) * 2.0;
                particle.velocity[k] = (particle.velocity[k]
                    + force * dt
                    + radial[k] / distance * kick * s.radius * 0.35)
                    * damping;
            }
            let velocity = length(particle.velocity);
            if velocity > s.radius * 4.0 {
                for v in &mut particle.velocity {
                    *v *= s.radius * 4.0 / velocity;
                }
            }
            for k in 0..3 {
                particle.position[k] += particle.velocity[k] * dt;
            }
            let distance = length(particle.position);
            if distance > s.radius * 3.4 {
                for k in 0..3 {
                    particle.position[k] *= s.radius * 3.4 / distance;
                    particle.velocity[k] *= 0.5;
                }
            }
        }
        self.performance.impulse = 0.0;
    }

    pub fn draw(
        &mut self,
        ui: &mut egui::Ui,
        rect: Rect,
        frame: &AnalysisFrame,
        theme: &AppTheme,
        now: Instant,
        live: bool,
    ) {
        self.advance(now, frame, live);
        if rect.width() < 4.0 || rect.height() < 4.0 {
            return;
        }
        let response = ui.interact(
            rect,
            ui.id().with("flux-camera"),
            egui::Sense::click_and_drag(),
        );
        if response.dragged() && !super::camera::over_navigator(ui) {
            let delta = ui.input(|i| i.pointer.delta());
            if response.dragged_by(egui::PointerButton::Secondary)
                || response.dragged_by(egui::PointerButton::Middle)
                || ui.input(|i| i.modifiers.shift)
            {
                self.settings.pan_x += delta.x / rect.width();
                self.settings.pan_y += delta.y / rect.height();
            } else if let Some(p) = response.interact_pointer_pos() {
                let mut yaw = self.settings.yaw.to_radians();
                let mut elevation = self.settings.elevation.to_radians();
                let mut roll = self.settings.roll.to_radians();
                super::trackball::drag(
                    &mut yaw,
                    &mut elevation,
                    &mut roll,
                    p - delta,
                    p,
                    rect.center(),
                    rect.width().min(rect.height()) * 0.4,
                );
                self.settings.yaw = yaw.to_degrees();
                self.settings.elevation = elevation.to_degrees();
                self.settings.roll = roll.to_degrees();
            }
            self.settings.auto_orbit = false;
        }
        if response.double_clicked() && !super::camera::over_navigator(ui) {
            self.reset_camera();
        }
        if response.hovered() && !super::camera::over_navigator(ui) {
            self.settings.zoom = (self.settings.zoom
                * (ui.input(|i| i.smooth_scroll_delta.y) * 0.002).exp())
            .clamp(0.35, 3.0);
        }
        let mut camera = Camera::new(rect, &self.settings, self.orbit as f32);
        camera.depth = self.performance.value(&self.settings, Macro::Depth);
        if self.settings.show_grid {
            super::camera::grid(ui, rect, |p| camera.project(p).0);
        }
        let mesh = self.mesh(&camera, theme);
        ui.painter_at(rect).add(egui::Shape::mesh(mesh));
        if self.settings.labels && rect.width() > 380.0 && rect.height() > 160.0 {
            ui.painter_at(rect).text(
                rect.left_bottom() + Vec2::new(10.0, -8.0),
                egui::Align2::LEFT_BOTTOM,
                "Drag: free rotate · Edge: roll · Right/Shift-drag: pan · Scroll: zoom",
                egui::FontId::monospace(9.0),
                theme.muted,
            );
        }
    }
    fn mesh(&self, camera: &Camera, theme: &AppTheme) -> egui::Mesh {
        let s = &self.settings;
        let brightness = self.performance.value(s, Macro::Brightness);
        let impact = self.performance.pulse * self.performance.value(s, Macro::Impact);
        let trails = (self.performance.value(s, Macro::Trails) + impact * 0.14).clamp(0.0, 1.0);
        let glow = self.performance.value(s, Macro::Glow);
        let color = self.color_phase.rem_euclid(1.0) as f32;

        let mut dots: Vec<_> = self
            .particles
            .iter()
            .map(|p| {
                let (position, depth, scale) = camera.project(p.position);
                let velocity = length(p.velocity) / s.radius;
                let hue = match s.color_mode {
                    ColorMode::Current => {
                        p.group as f32 / s.cores as f32 + p.variation * 0.09 + velocity * 0.04
                    }
                    ColorMode::Velocity => velocity * 0.22,
                    ColorMode::Frequency => {
                        p.group as f32 % 3.0 / 3.0 + self.energy[p.group % 3] * 0.16
                    }
                    ColorMode::Depth => depth / (s.radius * 5.0) + 0.5,
                };
                let tint = mix(
                    theme.foreground,
                    s.palette.cycle(hue + color, theme),
                    s.saturation,
                );
                let sparkle =
                    (self.phase * 2.7 + f64::from(p.variation * 48.0)).sin() as f32 * 0.5 + 0.5;
                // A quieter resting cloud leaves headroom for audible peaks. Band-specific
                // illumination and a small size bloom make changes visible at tiny dot sizes.
                let drive = 0.0;
                let light = (brightness
                    * (0.35
                        + drive * 0.60
                        + impact * 0.45
                        + sparkle * s.twinkle * (0.10 + drive * 0.22)))
                    .clamp(0.0, 1.0);
                let fade =
                    1.0 - s.depth_fade * ((1.0 - depth / (s.radius * 3.4)) * 0.5).clamp(0.0, 1.0);
                let radius = s.size
                    * (1.0 - s.size_variation * 0.65
                        + p.variation.powi(3) * s.size_variation * 1.3)
                    * scale
                    * (1.0 + self.modulator.value(Target::Size) * 0.65).max(0.05);
                let tail = camera
                    .project(std::array::from_fn(|k| {
                        p.position[k] - p.velocity[k] * trails * 0.12
                    }))
                    .0;
                Dot {
                    position,
                    tail,
                    depth,
                    radius,
                    color: tint.gamma_multiply(s.opacity * light * fade),
                    core: false,
                }
            })
            .collect();
        if s.show_cores {
            for (i, center) in self.centers().into_iter().enumerate() {
                let (position, depth, scale) = camera.project(center);
                dots.push(Dot {
                    position,
                    tail: position,
                    depth,
                    radius: s.core_size
                        * scale
                        * (1.0 + self.modulator.value(Target::Size) * 0.3).max(0.05),
                    color: mix(
                        s.palette.cycle(i as f32 / s.cores as f32, theme),
                        theme.foreground,
                        0.4,
                    )
                    .gamma_multiply(
                        s.opacity * (brightness * (0.4 + impact * 0.3)).clamp(0.0, 1.0),
                    ),
                    core: true,
                });
            }
        }
        dots.sort_by(|a, b| a.depth.total_cmp(&b.depth));
        let mut mesh = egui::Mesh::default();
        for dot in dots {
            if trails > 0.0 && !dot.core {
                streak(
                    &mut mesh,
                    dot.tail,
                    dot.position,
                    dot.radius * 0.7,
                    dot.color.gamma_multiply(0.5),
                );
            }
            if glow > 0.0 {
                disc(
                    &mut mesh,
                    dot.position,
                    dot.radius * if dot.core { 4.0 } else { 2.6 },
                    dot.color.gamma_multiply(glow * 0.18),
                );
            }
            disc(&mut mesh, dot.position, dot.radius, dot.color);
        }
        mesh
    }
}
fn random(index: u32, salt: u32) -> f32 {
    let mut x = index
        .wrapping_mul(747_796_405)
        .wrapping_add(salt.wrapping_mul(2_891_336_453));
    x = ((x >> ((x >> 28) + 4)) ^ x).wrapping_mul(277_803_737);
    (((x >> 22) ^ x) >> 8) as f32 / 16_777_216.0
}
fn sphere(u: f32, v: f32) -> [f32; 3] {
    let y = u * 2.0 - 1.0;
    let r = (1.0 - y * y).sqrt();
    [r * (v * TAU).cos(), y, r * (v * TAU).sin()]
}
fn length(p: [f32; 3]) -> f32 {
    (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt()
}
struct Camera {
    yaw: f32,
    elevation: f32,
    roll: f32,
    scale: f32,
    radius: f32,
    depth: f32,
    perspective: f32,
    center: Pos2,
}
impl Camera {
    fn new(rect: Rect, s: &FluxSettings, orbit: f32) -> Self {
        Self {
            yaw: (s.yaw + orbit).to_radians(),
            elevation: s.elevation.to_radians(),
            roll: s.roll.to_radians(),
            scale: rect.width().min(rect.height()).max(1.0) * s.zoom / 4.2,
            radius: s.radius,
            depth: s.depth,
            perspective: s.perspective,
            center: rect.center() + Vec2::new(s.pan_x, s.pan_y) * rect.size(),
        }
    }
    fn project(&self, p: [f32; 3]) -> (Pos2, f32, f32) {
        let (point, depth) =
            project_direction(self.yaw, self.elevation, [p[0], p[1], p[2] * self.depth]);
        let perspective = (1.0 / (1.0 - depth / (self.radius * 8.0) * self.perspective).max(0.15))
            .clamp(0.4, 2.0);
        let rotated = Vec2::new(
            point.x * self.roll.cos() - point.y * self.roll.sin(),
            point.x * self.roll.sin() + point.y * self.roll.cos(),
        );
        (
            self.center + rotated * self.scale * perspective,
            depth,
            perspective,
        )
    }
}
struct Dot {
    position: Pos2,
    tail: Pos2,
    depth: f32,
    radius: f32,
    color: Color32,
    core: bool,
}
fn disc(mesh: &mut egui::Mesh, p: Pos2, radius: f32, color: Color32) {
    let base = mesh.vertices.len() as u32;
    mesh.colored_vertex(p, color);
    for i in 0..8 {
        mesh.colored_vertex(
            p + Vec2::angled(i as f32 * TAU / 8.0) * (radius + 0.5),
            Color32::TRANSPARENT,
        );
    }
    for i in 0..8 {
        mesh.add_triangle(base, base + 1 + i, base + 1 + (i + 1) % 8);
    }
}
fn streak(mesh: &mut egui::Mesh, a: Pos2, b: Pos2, width: f32, color: Color32) {
    let d = b - a;
    if d.length_sq() < 0.5 {
        return;
    }
    let normal = Vec2::new(-d.y, d.x).normalized() * width.max(0.35);
    let base = mesh.vertices.len() as u32;
    for (p, c) in [
        (a - normal, Color32::TRANSPARENT),
        (a + normal, Color32::TRANSPARENT),
        (b - normal, color),
        (b + normal, color),
    ] {
        mesh.colored_vertex(p, c);
    }
    mesh.add_triangle(base, base + 1, base + 2);
    mesh.add_triangle(base + 1, base + 3, base + 2);
}

impl super::camera::Controlled for Flux {
    fn camera(&mut self) -> super::camera::Camera<'_> {
        let s = &mut self.settings;
        super::camera::Camera {
            yaw: &mut s.yaw,
            elevation: &mut s.elevation,
            roll: &mut s.roll,
            zoom: &mut s.zoom,
            pan_x: &mut s.pan_x,
            pan_y: &mut s.pan_y,
            degrees: true,
            y_up: false,
            enabled: &mut s.auto_orbit,
            speed: &mut s.orbit_speed,
            speed_radians: false,
            default_speed: 3.0,
            speed_limit: 15.0,
            speed_key: "orbit_speed",
            reverse: None,
            motion: &mut s.camera_motion,
            gizmo: &mut s.show_gizmo,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::FrequencyBin;
    use std::time::Duration;

    fn signal() -> AnalysisFrame {
        AnalysisFrame {
            bins: [80.0, 1000.0, 8000.0]
                .into_iter()
                .map(|frequency_hz| FrequencyBin {
                    frequency_hz,
                    magnitude: 0.3,
                    dbfs: -10.0,
                })
                .collect(),
            rms: [0.1, 0.3],
            mix_rms: 0.2,
            ..Default::default()
        }
    }
    fn small_cloud() -> Flux {
        Flux {
            settings: FluxSettings {
                count: 240,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn particles_are_deterministic_and_settings_never_save_simulation() {
        let mut a = small_cloud();
        let mut b = small_cloud();
        a.ensure_particles();
        b.ensure_particles();
        assert_eq!(a.particles, b.particles);
        let original = a.particles.clone();
        a.settings.count = 300;
        a.ensure_particles();
        assert_eq!(&a.particles[..240], original.as_slice());
        a.settings.seed += 1;
        a.ensure_particles();
        assert_ne!(&a.particles[..240], original.as_slice());
        let encoded = serde_json::to_string(&a.settings_snapshot()).unwrap();
        assert!(
            !encoded.contains("position")
                && !encoded.contains("velocity")
                && !encoded.contains("phase")
                && !encoded.contains("energy")
        );
        b.apply_settings(&serde_json::from_str(&encoded).unwrap());
        assert_eq!(b.settings_snapshot(), a.settings_snapshot());
        let mut invalid = b.settings_snapshot();
        invalid.count = u32::MAX;
        invalid.cores = 0;
        invalid.speed = f32::NAN;
        invalid.drag = 0.0;
        b.apply_settings(&invalid);
        assert_eq!(b.settings.count, 20_000);
        assert_eq!(b.settings.cores, 1);
        assert!(b.settings.speed.is_finite());
        assert_eq!(b.settings.drag, 0.1);
    }

    #[test]
    fn fixed_steps_match_across_frame_rates_and_pause_freezes_every_clock() {
        let start = Instant::now();
        let frame = signal();
        let mut fast = small_cloud();
        let mut slow = small_cloud();
        for i in 0..=120 {
            fast.advance(
                start + Duration::from_secs_f64(i as f64 / 120.0),
                &frame,
                true,
            );
        }
        for i in 0..=30 {
            slow.advance(
                start + Duration::from_secs_f64(i as f64 / 30.0),
                &frame,
                true,
            );
        }
        assert!((fast.phase - slow.phase).abs() < 0.0001);
        for (a, b) in fast.particles.iter().zip(&slow.particles) {
            assert!(length(std::array::from_fn(|k| a.position[k] - b.position[k])) < 0.0001);
        }
        let frozen = slow.particles.clone();
        let phase = slow.phase;
        let orbit = slow.orbit;
        for _ in 0..100 {
            slow.advance(
                start + Duration::from_secs(1),
                &AnalysisFrame::default(),
                true,
            );
        }
        assert_eq!(slow.particles, frozen);
        assert_eq!(slow.phase, phase);
        assert_eq!(slow.orbit, orbit);
        // A hidden pane resumes with bounded work instead of simulating every missed second.
        slow.advance(start + Duration::from_secs(3600), &frame, true);
        assert!(slow.phase - phase < 0.151);
    }

    #[test]
    fn sound_changes_trajectories_and_silence_releases_response() {
        let start = Instant::now();
        let mut live = small_cloud();
        let mut silent = small_cloud();
        for i in 0..=120 {
            let now = start + Duration::from_secs_f64(i as f64 / 120.0);
            live.advance(now, &signal(), true);
            silent.advance(now, &AnalysisFrame::default(), true);
        }
        assert!(live.energy.iter().all(|e| *e > 0.5));
        assert!(live.balance > 0.15);
        let kinetic_energy = |cloud: &Flux| {
            cloud
                .particles
                .iter()
                .map(|p| length(p.velocity).powi(2))
                .sum::<f32>()
        };
        assert!(kinetic_energy(&live) > kinetic_energy(&silent) * 1.5);
        let camera = Camera::new(
            Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 900.0)),
            &live.settings,
            0.0,
        );
        let brightness = |cloud: &Flux| {
            cloud
                .mesh(&camera, &AppTheme::default())
                .vertices
                .iter()
                .map(|v| u64::from(v.color.r()) + u64::from(v.color.g()) + u64::from(v.color.b()))
                .sum::<u64>()
        };
        assert!(brightness(&live) > brightness(&silent) * 2);
        assert!(
            live.particles
                .iter()
                .zip(silent.particles)
                .any(|(a, b)| length(std::array::from_fn(|k| a.position[k] - b.position[k])) > 0.1)
        );
        for i in 121..=1080 {
            live.advance(
                start + Duration::from_secs_f64(i as f64 / 120.0),
                &signal(),
                false,
            );
        }
        assert!(live.energy.iter().all(|e| *e < 0.0001));
        assert!(live.balance.abs() < 0.0001);
    }

    #[test]
    fn force_route_wakes_zero_ambient_cloud_without_a_speed_route() {
        let mut active = small_cloud();
        let m = active.settings.modulation.as_mut().unwrap();
        m.ambient = 0.0;
        m.routes.retain(|r| r.target == Target::Expansion);
        let mut silent = small_cloud();
        silent.settings = active.settings.clone();
        let start = Instant::now();
        for i in 0..=120 {
            let now = start + Duration::from_secs_f64(i as f64 / 120.0);
            active.advance(now, &signal(), true);
            silent.advance(now, &AnalysisFrame::default(), true);
        }
        assert_eq!(silent.phase, 0.0);
        assert!(active.phase > 0.5);
        assert_ne!(active.particles, silent.particles);
    }

    #[test]
    fn brightness_only_mapping_leaves_particle_trajectories_unchanged() {
        let mut active = small_cloud();
        active
            .settings
            .modulation
            .as_mut()
            .unwrap()
            .routes
            .retain(|r| r.target == Target::Brightness);
        let mut silent = small_cloud();
        silent.settings = active.settings.clone();
        let start = Instant::now();
        for i in 0..=120 {
            let now = start + Duration::from_secs_f64(i as f64 / 120.0);
            active.advance(now, &signal(), true);
            silent.advance(now, &AnalysisFrame::default(), true);
        }
        assert_eq!(active.particles, silent.particles);
        assert!(active.energy[3] > 0.5);
        assert_eq!(silent.energy[3], 0.0);
        let camera = Camera::new(
            Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 700.0)),
            &active.settings,
            0.0,
        );
        let a = active.mesh(&camera, &AppTheme::default());
        let b = silent.mesh(&camera, &AppTheme::default());
        assert_eq!(a.vertices.len(), b.vertices.len());
        assert!(
            a.vertices
                .iter()
                .zip(&b.vertices)
                .any(|(a, b)| a.color != b.color)
        );
        assert!(
            a.vertices
                .iter()
                .zip(&b.vertices)
                .all(|(a, b)| a.pos == b.pos)
        );
    }

    fn formation_spread(cloud: &Flux) -> (f32, f32, f32) {
        let centers = cloud.centers();
        let mut normal_power = 0.0;
        let mut radial = 0.0;
        let mut radial_power = 0.0;
        for p in &cloud.particles {
            let d: [f32; 3] = std::array::from_fn(|k| p.position[k] - centers[p.group][k]);
            let axis = [1, 0, 2][p.group % 3];
            normal_power += d[axis].powi(2);
            let r = length(d);
            radial += r;
            radial_power += r * r;
        }
        let n = cloud.particles.len() as f32;
        (
            (normal_power / n).sqrt(),
            (radial_power / n - (radial / n).powi(2)).max(0.0).sqrt(),
            radial / n,
        )
    }

    #[test]
    fn cloud_stays_volumetric_and_magnetism_progressively_forms_bands() {
        let mut formations: Vec<_> = [0.0, 0.5, 1.0]
            .into_iter()
            .map(|magnetism| {
                let mut cloud = small_cloud();
                cloud.settings.orbit_magnetism = magnetism;
                cloud.settings.core_motion = 0.0;
                cloud.energy = [0.7, 0.5, 0.3, 0.8];
                cloud.ensure_particles();
                cloud
            })
            .collect();
        // Sample repeatedly over 30 seconds, well beyond the initial seeded cloud.
        for step in 0..3600 {
            for cloud in &mut formations {
                cloud.step(STEP);
            }
            if step >= 1200 && step % 120 == 0 {
                let spreads: Vec<_> = formations.iter().map(formation_spread).collect();
                assert!(spreads[0].0 > 0.25, "cloud lost depth: {spreads:?}");
                assert!(
                    spreads[0].1 > 0.15,
                    "cloud collapsed to a shell: {spreads:?}"
                );
                assert!(
                    spreads[0].0 > spreads[1].0 * 1.4,
                    "intermediate blend: {spreads:?}"
                );
                assert!(
                    spreads[1].0 > spreads[2].0 * 3.0,
                    "band not magnetized: {spreads:?}"
                );
                assert!(spreads[2].0 < 0.08, "ring too thick: {spreads:?}");
            }
        }
        // Releasing magnetism restores depth without reseeding or teleporting.
        let mut ring = formations.remove(2);
        let before = ring.particles.clone();
        ring.settings.orbit_magnetism = 0.0;
        ring.step(STEP);
        for (a, b) in before.iter().zip(&ring.particles) {
            assert_eq!(a.home, b.home);
            assert!(length(std::array::from_fn(|k| a.position[k] - b.position[k])) < 0.04);
        }
        for _ in 0..1200 {
            ring.step(STEP);
        }
        assert!(formation_spread(&ring).0 > 0.25);
    }

    #[test]
    fn audio_expansion_changes_both_cloud_and_orbital_radius() {
        for magnetism in [0.0, 1.0] {
            let mut quiet = small_cloud();
            quiet.settings.orbit_magnetism = magnetism;
            quiet.settings.core_motion = 0.0;
            quiet.ensure_particles();
            let mut loud = small_cloud();
            loud.settings = quiet.settings.clone();
            loud.performance.modulation[Macro::Expansion as usize] = 0.35;
            loud.ensure_particles();
            for _ in 0..2400 {
                quiet.performance.advance(&quiet.settings, 0.0, STEP);
                loud.performance.advance(&loud.settings, 0.0, STEP);
                quiet.step(STEP);
                loud.step(STEP);
            }
            assert!(formation_spread(&loud).2 > formation_spread(&quiet).2 * 1.2);
        }
    }

    #[test]
    fn magnetism_round_trips_and_old_presets_start_as_clouds() {
        let mut settings = FluxSettings {
            orbit_magnetism: 0.75,
            ..Default::default()
        };
        let mut json = serde_json::to_value(&settings).unwrap();
        let saved: FluxSettings = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(saved.orbit_magnetism, 0.75);
        json.as_object_mut().unwrap().remove("orbit_magnetism");
        let old: FluxSettings = serde_json::from_value(json).unwrap();
        assert_eq!(old.orbit_magnetism, 0.0);
        settings.orbit_magnetism = f32::NAN;
        settings.sanitize();
        assert_eq!(settings.orbit_magnetism, 0.0);
        settings.orbit_magnetism = 4.0;
        settings.sanitize();
        assert_eq!(settings.orbit_magnetism, 1.0);
    }

    #[test]
    fn extreme_forces_stay_bounded_and_core_changes_reseed_safely() {
        let mut cloud = small_cloud();
        cloud.settings.radius = 0.4;
        cloud.settings.cores = 8;
        cloud.settings.attraction = 3.0;
        cloud.settings.swirl = -3.0;
        cloud.settings.drag = 0.1;
        cloud.settings.turbulence = 2.0;
        cloud.settings.diffusion = 1.0;
        cloud.settings.gravity = 1.0;
        cloud.settings.bass_push = 3.0;
        cloud.settings.mid_flow = 3.0;
        cloud.settings.high_scatter = 3.0;
        cloud.settings.transients = 3.0;
        cloud.energy = [1.0; 4];
        cloud.ensure_particles();
        for i in 0..2400 {
            cloud.settings.orbit_magnetism = if i < 1200 { 0.0 } else { 1.0 };
            cloud.step(STEP * 2.0);
        }
        for p in &cloud.particles {
            assert!(p.position.iter().chain(&p.velocity).all(|v| v.is_finite()));
            assert!(length(p.position) <= cloud.settings.radius * 3.401);
            assert!(length(p.velocity) <= cloud.settings.radius * 4.001);
        }
        cloud.settings.cores = 1;
        cloud.ensure_particles();
        assert!(cloud.particles.iter().all(|p| p.group == 0));
    }

    #[test]
    fn every_orbital_formation_and_impact_stays_bounded_under_live_audio() {
        let start = Instant::now();
        for shape in [0.0, 0.25, 0.5, 0.75, 1.0] {
            let mut cloud = small_cloud();
            cloud.settings.orbit_shape = shape;
            cloud.settings.orbit_magnetism = 1.0;
            cloud.settings.impact = 2.0;
            cloud.settings.speed = 2.0;
            cloud.settings.expansion = 2.0;
            for i in 0..=1200 {
                if i % 120 == 0 {
                    cloud.performance.manual_burst = true;
                }
                cloud.advance(
                    start + Duration::from_secs_f64(i as f64 / 120.0),
                    &signal(),
                    true,
                );
            }
            assert!(cloud.particles.iter().all(|p| {
                p.position.iter().chain(&p.velocity).all(|v| v.is_finite())
                    && length(p.position) <= cloud.settings.radius * 3.401
            }));
            let camera = Camera::new(
                Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 700.0)),
                &cloud.settings,
                0.0,
            );
            let mesh = cloud.mesh(&camera, &AppTheme::default());
            assert!(mesh.is_valid());
            assert!(mesh.vertices.iter().all(|v| v.pos.is_finite()));
        }
    }

    #[test]
    fn impact_adds_a_short_music_accent_and_brightness_can_black_out_the_scene() {
        let start = Instant::now();
        let mut accented = small_cloud();
        accented.settings.impact = 2.0;
        let mut plain = small_cloud();
        plain.settings.impact = 0.0;
        for i in 0..=18 {
            let time = start + Duration::from_secs_f64(i as f64 / 120.0);
            accented.advance(time, &signal(), true);
            plain.advance(time, &signal(), true);
        }
        assert!(accented.performance.pulse > 0.3);
        assert_ne!(accented.particles, plain.particles);
        let phase = accented.phase;
        let pulse = accented.performance.pulse;
        accented.advance(
            start + Duration::from_secs_f64(18.0 / 120.0),
            &signal(),
            true,
        );
        assert_eq!(accented.phase, phase);
        assert_eq!(accented.performance.pulse, pulse);
        for i in 19..=480 {
            accented.advance(
                start + Duration::from_secs_f64(i as f64 / 120.0),
                &signal(),
                true,
            );
        }
        assert!(
            accented.performance.pulse < 0.0001,
            "a sustained tone should settle instead of endlessly retriggering"
        );
        Macro::Brightness.set_normalized(&mut accented.settings, 0.0);
        for i in 481..=600 {
            accented.advance(
                start + Duration::from_secs_f64(i as f64 / 120.0),
                &signal(),
                true,
            );
        }
        let camera = Camera::new(
            Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 700.0)),
            &accented.settings,
            0.0,
        );
        assert!(
            accented
                .mesh(&camera, &AppTheme::default())
                .vertices
                .iter()
                .all(|v| v.color == Color32::TRANSPARENT)
        );
    }

    #[test]
    fn rotated_perspective_cloud_produces_finite_bounded_mesh() {
        let mut cloud = Flux::default();
        cloud.settings.count = 20_000;
        cloud.settings.cores = 8;
        cloud.settings.depth = 2.0;
        cloud.settings.perspective = 1.0;
        cloud.settings.glow = 1.0;
        cloud.ensure_particles();
        cloud.step(STEP);
        let theme = AppTheme::default();
        for (yaw, elevation, roll) in [
            (-25.0, 18.0, 0.0),
            (180.0, 85.0, 180.0),
            (-180.0, -85.0, -45.0),
        ] {
            cloud.settings.yaw = yaw;
            cloud.settings.elevation = elevation;
            cloud.settings.roll = roll;
            let camera = Camera::new(
                Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 900.0)),
                &cloud.settings,
                0.0,
            );
            let mesh = cloud.mesh(&camera, &theme);
            assert!(mesh.is_valid());
            assert!(mesh.vertices.len() < 450_000);
            assert!(mesh.vertices.iter().all(|v| v.pos.is_finite()));
        }
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 700.0));
        cloud.settings.yaw = 0.0;
        cloud.settings.elevation = 0.0;
        cloud.settings.roll = 0.0;
        let a = Camera::new(rect, &cloud.settings, 0.0)
            .project([0.5, 0.0, 0.0])
            .0;
        cloud.settings.roll = 90.0;
        let b = Camera::new(rect, &cloud.settings, 0.0)
            .project([0.5, 0.0, 0.0])
            .0;
        assert!(a.x > rect.center().x && b.y > rect.center().y);
        assert!((a.y - rect.center().y).abs() < 0.001 && (b.x - rect.center().x).abs() < 0.001);
    }

    #[test]
    #[ignore = "manual simulation/render timing probe"]
    fn render_timing() {
        let ctx = egui::Context::default();
        let theme = AppTheme::default();
        let frame = signal();
        for count in [6000, 20_000] {
            let mut cloud = Flux {
                settings: FluxSettings {
                    count,
                    ..Default::default()
                },
                ..Default::default()
            };
            let start = Instant::now();
            for i in 0..120 {
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(
                            Pos2::ZERO,
                            Vec2::new(1600.0, 900.0),
                        )),
                        ..Default::default()
                    },
                    |ui| {
                        cloud.draw(
                            ui,
                            ui.max_rect(),
                            &frame,
                            &theme,
                            start + Duration::from_secs_f64(i as f64 / 60.0),
                            true,
                        );
                    },
                );
                output.textures_delta.clear();
                std::hint::black_box(ctx.tessellate(output.shapes, output.pixels_per_point));
            }
            eprintln!(
                "Flux {count} particles: {:.2} ms/frame for simulation + UI + tessellation",
                start.elapsed().as_secs_f64() * 1000.0 / 120.0
            );
        }
    }
}
