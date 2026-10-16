//! A bounded, fixed-step particle field with music-driven attraction and flow.
use super::{camera_gizmo::project_direction, mix, properties_combo, settings_panel};
use crate::{analysis::AnalysisFrame, help::HoverHelp, parameter::Parameter, theme::AppTheme};
use eframe::egui::{self, Color32, Pos2, Rect, Vec2};
use std::{f32::consts::TAU, time::Instant};

const STEP: f32 = 1.0 / 120.0;
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Palette {
    Spectrum,
    Solar,
    Ion,
    Theme,
}
impl Palette {
    fn label(self) -> &'static str {
        match self {
            Self::Spectrum => "Spectrum",
            Self::Solar => "Solar",
            Self::Ion => "Ion",
            Self::Theme => "Theme accent",
        }
    }
    fn color(self, t: f32, theme: &AppTheme) -> Color32 {
        let colors = match self {
            Self::Spectrum => [
                Color32::from_rgb(255, 212, 65),
                Color32::from_rgb(255, 113, 49),
                Color32::from_rgb(208, 86, 238),
                Color32::from_rgb(94, 103, 255),
                Color32::from_rgb(48, 225, 227),
                Color32::from_rgb(159, 231, 85),
            ],
            Self::Solar => [
                Color32::from_rgb(255, 239, 145),
                Color32::from_rgb(255, 179, 42),
                Color32::from_rgb(255, 93, 55),
                Color32::from_rgb(225, 61, 121),
                Color32::from_rgb(251, 130, 61),
                Color32::from_rgb(255, 213, 95),
            ],
            Self::Ion => [
                Color32::from_rgb(106, 247, 233),
                Color32::from_rgb(61, 164, 255),
                Color32::from_rgb(92, 83, 232),
                Color32::from_rgb(187, 97, 252),
                Color32::from_rgb(230, 172, 255),
                Color32::from_rgb(95, 223, 255),
            ],
            Self::Theme => [
                theme.accent,
                theme.accent_alt,
                theme.foreground,
                theme.accent_alt,
                theme.accent,
                theme.foreground,
            ],
        };
        let x = t.rem_euclid(1.0) * 6.0;
        mix(
            colors[x as usize % 6],
            colors[(x as usize + 1) % 6],
            x.fract(),
        )
    }
}
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
    count: u32,
    cores: u32,
    seed: u32,
    radius: f32,
    depth: f32,
    speed: f32,
    attraction: f32,
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
            count: 6000,
            cores: 5,
            seed: 41,
            radius: 1.0,
            depth: 1.0,
            speed: 1.0,
            attraction: 1.1,
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
            radius: 0.4, 2.0; depth: 0.2, 2.0; speed: 0.0, 2.0;
            attraction: 0.0, 3.0; swirl: -3.0, 3.0; turbulence: 0.0, 2.0; drag: 0.1, 3.0;
            diffusion: 0.0, 1.0; core_motion: 0.0, 2.0; gravity: -1.0, 1.0;
            sensitivity: 0.0, 4.0; gate_db: -90.0, -20.0; attack_ms: 5.0, 500.0; release_ms: 50.0, 3000.0;
            bass_push: 0.0, 3.0; mid_flow: 0.0, 3.0; high_scatter: 0.0, 3.0; stereo: 0.0, 2.0; transients: 0.0, 3.0;
            size: 0.3, 3.0; size_variation: 0.0, 1.0; opacity: 0.0, 1.0; trails: 0.0, 1.0;
            glow: 0.0, 1.0; twinkle: 0.0, 1.0; core_size: 1.0, 8.0;
            saturation: 0.0, 1.0; depth_fade: 0.0, 1.0;
            yaw: -180.0, 180.0; elevation: -85.0, 85.0; roll: -180.0, 180.0; zoom: 0.35, 3.0;
            perspective: 0.0, 1.0; orbit_speed: -15.0, 15.0;
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
struct Particle {
    position: [f32; 3],
    velocity: [f32; 3],
    variation: f32,
    group: usize,
}
#[derive(Default)]
pub struct Flux {
    settings: FluxSettings,
    particles: Vec<Particle>,
    pattern: Option<(u32, u32)>,
    last_time: Option<Instant>,
    accumulator: f32,
    phase: f64,
    orbit: f64,
    energy: [f32; 4],
    slow_bass: f32,
    balance: f32,
}
impl Flux {
    pub(crate) fn settings_snapshot(&self) -> FluxSettings {
        self.settings.clone()
    }
    pub(crate) fn apply_settings(&mut self, settings: &FluxSettings) {
        self.settings = settings.clone();
        self.settings.sanitize();
    }
    pub fn clear(&mut self) {
        self.particles.clear();
        self.pattern = None;
        self.last_time = None;
        self.accumulator = 0.0;
        self.phase = 0.0;
        self.orbit = 0.0;
        self.energy = [0.0; 4];
        self.slow_bass = 0.0;
        self.balance = 0.0;
    }
    pub fn reset_settings(&mut self) {
        self.settings = FluxSettings::default();
        self.clear();
    }
    fn reset_camera(&mut self) {
        let d = FluxSettings::default();
        self.settings.yaw = d.yaw;
        self.settings.elevation = d.elevation;
        self.settings.roll = 0.0;
        self.settings.zoom = 1.0;
        self.orbit = 0.0;
    }
    pub fn controls(&mut self, ui: &mut egui::Ui) {
        let s = &mut self.settings;
        macro_rules! p {
            ($ui:ident, $field:ident, $range:expr, $label:expr, $help:expr) => {
                $ui.add(
                    Parameter::new(&mut s.$field, $range, FluxSettings::default().$field)
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
                attraction,
                0.0..=3.0,
                "Attraction",
                "Pull toward the moving cores. Low values produce diffuse clouds; high values form dense streams."
            );
            p!(
                ui,
                swirl,
                -3.0..=3.0,
                "Swirl",
                "Rotating force around each core. Negative values reverse its direction."
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
                "Individual particle drift that prevents the cloud collapsing onto the cores."
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
        settings_panel(ui, "Signal Response", |ui| {
            p!(
                ui,
                sensitivity,
                0.0..=4.0,
                "Sensitivity",
                "Visual response gain. Zero produces ambient motion without audio modulation."
            );
            p!(
                ui,
                gate_db,
                -90.0..=-20.0,
                "Noise floor dB",
                "Ignore energy below this threshold."
            );
            p!(
                ui,
                attack_ms,
                5.0..=500.0,
                "Attack ms",
                "How quickly the forces and brightness respond to increasing energy."
            );
            p!(
                ui,
                release_ms,
                50.0..=3000.0,
                "Release ms",
                "How slowly the visual response settles after a sound."
            );
            p!(
                ui,
                bass_push,
                0.0..=3.0,
                "Bass expansion",
                "Low-frequency energy pushes particles away from their cores."
            );
            p!(
                ui,
                mid_flow,
                0.0..=3.0,
                "Mid currents",
                "Mid-frequency energy increases swirling flow."
            );
            p!(
                ui,
                high_scatter,
                0.0..=3.0,
                "High scatter",
                "High-frequency energy increases fine turbulence and sparkle."
            );
            p!(
                ui,
                transients,
                0.0..=3.0,
                "Transient bursts",
                "Extra outward force when bass energy rises above its slower envelope. Energy-driven; not BPM synchronization."
            );
            p!(
                ui,
                stereo,
                0.0..=2.0,
                "Stereo steering",
                "Left/right RMS balance steers the cloud horizontally."
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
            properties_combo("flux-palette", 176.0, 4)
                .selected_text(s.palette.label())
                .show_ui(ui, |ui| {
                    for palette in [
                        Palette::Spectrum,
                        Palette::Solar,
                        Palette::Ion,
                        Palette::Theme,
                    ] {
                        ui.selectable_value(&mut s.palette, palette, palette.label());
                    }
                })
                .response
                .help_text("Color family for the cloud and its currents.");
            properties_combo("flux-color-mode", 176.0, 4).selected_text(s.color_mode.label()).show_ui(ui, |ui| {
                for mode in [ColorMode::Current, ColorMode::Velocity, ColorMode::Frequency, ColorMode::Depth] { ui.selectable_value(&mut s.color_mode, mode, mode.label()); }
            }).response.help_text("Map color to current membership, speed, audio bands, or distance from the camera.");
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
                "Rotate the image plane; Shift-drag also adjusts roll."
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
                group,
            });
        }
    }
    fn advance(&mut self, now: Instant, frame: &AnalysisFrame, live: bool) {
        self.ensure_particles();
        let dt = self.last_time.map_or(0.0, |last| {
            now.saturating_duration_since(last).as_secs_f32().min(0.05)
        });
        self.last_time = Some(now);
        if dt == 0.0 {
            return;
        }
        let target = if live {
            super::reactive::energy(frame, self.settings.sensitivity, self.settings.gate_db)
        } else {
            [0.0; 4]
        };
        let sum = frame.rms[0] + frame.rms[1];
        let balance = if live && sum.is_finite() && sum > 0.0001 {
            ((frame.rms[1] - frame.rms[0]) / sum).clamp(-1.0, 1.0)
        } else {
            0.0
        };
        self.accumulator += dt;
        // At most six 120 Hz steps per frame after a stall, never an unbounded catch-up.
        while self.accumulator + 1e-7 >= STEP {
            self.accumulator = (self.accumulator - STEP).max(0.0);
            for (energy, target) in self.energy.iter_mut().zip(target) {
                let ms = if target > *energy {
                    self.settings.attack_ms
                } else {
                    self.settings.release_ms
                };
                *energy += (target - *energy) * (1.0 - (-STEP * 1000.0 / ms).exp());
            }
            self.slow_bass += (self.energy[0] - self.slow_bass) * (1.0 - (-STEP / 0.6).exp());
            self.balance += (balance - self.balance) * (1.0 - (-STEP / 0.25).exp());
            if self.settings.auto_orbit {
                self.orbit =
                    (self.orbit + f64::from(STEP * self.settings.orbit_speed)).rem_euclid(360.0);
            }
            self.step(STEP * self.settings.speed);
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
        let pulse = (self.energy[0] - self.slow_bass).max(0.0) * s.transients;
        let expansion = 0.08 + self.energy[0] * s.bass_push * 0.6 + pulse * 3.0;
        let turbulence = s.turbulence + self.energy[2] * s.high_scatter * 1.3;
        let swirl = s.swirl * (1.0 + self.energy[1] * s.mid_flow * 2.0);
        let damping = (-dt * s.drag).exp();
        for particle in &mut self.particles {
            let p = particle.position;
            let center = centers[particle.group];
            let d: [f32; 3] = std::array::from_fn(|axis| center[axis] - p[axis]);
            let distance = length(d).max(s.radius * 0.08);
            let individual = particle.variation * TAU;
            let axis = particle.group % 3;
            let mut tangent = [d[2], 0.0, -d[0]];
            if axis == 1 {
                tangent = [0.0, d[2], -d[1]];
            }
            if axis == 2 {
                tangent = [d[1], -d[0], 0.0];
            }
            let field = [
                (p[1] / s.radius * 2.1 + t * 0.7).sin() + (p[2] / s.radius * 1.7 - t * 0.4).cos(),
                (p[2] / s.radius * 2.3 + t * 0.5).sin() - (p[0] / s.radius * 1.8 + t * 0.6).cos(),
                (p[0] / s.radius * 2.0 - t * 0.4).sin() + (p[1] / s.radius * 2.5 + t * 0.3).cos(),
            ];
            for k in 0..3 {
                let diffusion = (t * 1.3 + individual + k as f32 * 2.1).sin() * s.diffusion;
                let mut force = d[k] * s.attraction + tangent[k] * swirl
                    - d[k] / distance * expansion * s.radius
                    + (field[k] * turbulence * 0.22 + diffusion * 0.18) * s.radius;
                if k == 0 {
                    force += self.balance * s.stereo * s.radius * 0.3;
                }
                if k == 1 {
                    force += s.gravity * s.radius;
                }
                // Smooth confinement keeps particles in view instead of teleporting them.
                force -= p[k] * (length(p) / s.radius - 1.6).max(0.0) * 2.0;
                particle.velocity[k] = (particle.velocity[k] + force * dt) * damping;
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
        if response.dragged() {
            let (delta, shift) = ui.input(|i| (i.pointer.delta(), i.modifiers.shift));
            self.settings.yaw = wrap(self.settings.yaw + self.orbit as f32);
            self.orbit = 0.0;
            if shift {
                self.settings.roll = wrap(self.settings.roll + delta.x * 0.4);
            } else {
                self.settings.yaw = wrap(self.settings.yaw + delta.x * 0.35);
                self.settings.elevation =
                    (self.settings.elevation + delta.y * 0.3).clamp(-85.0, 85.0);
            }
            self.settings.auto_orbit = false;
        }
        if response.double_clicked() {
            self.reset_camera();
        }
        if response.hovered() {
            self.settings.zoom = (self.settings.zoom
                * (ui.input(|i| i.smooth_scroll_delta.y) * 0.002).exp())
            .clamp(0.35, 3.0);
        }
        let camera = Camera::new(rect, &self.settings, self.orbit as f32);
        let mesh = self.mesh(&camera, theme);
        ui.painter_at(rect).add(egui::Shape::mesh(mesh));
        if self.settings.labels && rect.width() > 380.0 && rect.height() > 160.0 {
            ui.painter_at(rect).text(
                rect.left_bottom() + Vec2::new(10.0, -8.0),
                egui::Align2::LEFT_BOTTOM,
                "Drag: orbit · Shift-drag: roll · Scroll: zoom · Double-click: reset",
                egui::FontId::monospace(9.0),
                theme.muted,
            );
        }
    }
    fn mesh(&self, camera: &Camera, theme: &AppTheme) -> egui::Mesh {
        let s = &self.settings;
        let pulse = ((self.energy[0] - self.slow_bass).max(0.0) * s.transients).min(1.0);
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
                let tint = mix(theme.foreground, s.palette.color(hue, theme), s.saturation);
                let sparkle =
                    (self.phase * 2.7 + f64::from(p.variation * 48.0)).sin() as f32 * 0.5 + 0.5;
                // A quieter resting cloud leaves headroom for audible peaks. Band-specific
                // illumination and a small size bloom make changes visible at tiny dot sizes.
                let drive = self.energy[3] * 0.55 + self.energy[p.group % 3] * 0.45;
                let light = (0.24
                    + drive * 0.64
                    + pulse * 0.45
                    + sparkle * s.twinkle * (0.12 + self.energy[2] * s.high_scatter * 0.3))
                    .min(1.0);
                let fade =
                    1.0 - s.depth_fade * ((1.0 - depth / (s.radius * 3.4)) * 0.5).clamp(0.0, 1.0);
                let radius = s.size
                    * (1.0 - s.size_variation * 0.65
                        + p.variation.powi(3) * s.size_variation * 1.3)
                    * scale
                    * (1.0 + drive * 0.35 + pulse * 0.25);
                let tail = camera
                    .project(std::array::from_fn(|k| {
                        p.position[k] - p.velocity[k] * s.trails * 0.12
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
                    radius: s.core_size * scale * (1.0 + self.energy[i % 3] * 0.3),
                    color: mix(
                        s.palette.color(i as f32 / s.cores as f32, theme),
                        theme.foreground,
                        0.4,
                    )
                    .gamma_multiply(s.opacity * (0.4 + self.energy[i % 3] * 0.6)),
                    core: true,
                });
            }
        }
        dots.sort_by(|a, b| a.depth.total_cmp(&b.depth));
        let mut mesh = egui::Mesh::default();
        for dot in dots {
            if s.trails > 0.0 && !dot.core {
                streak(
                    &mut mesh,
                    dot.tail,
                    dot.position,
                    dot.radius * 0.7,
                    dot.color.gamma_multiply(0.5),
                );
            }
            if s.glow > 0.0 {
                disc(
                    &mut mesh,
                    dot.position,
                    dot.radius * if dot.core { 4.0 } else { 2.6 },
                    dot.color.gamma_multiply(s.glow * 0.18),
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
fn wrap(degrees: f32) -> f32 {
    (degrees + 180.0).rem_euclid(360.0) - 180.0
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
            center: rect.center(),
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
        assert!(slow.phase - phase < 0.051);
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
        assert!(live.balance > 0.4);
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
        for _ in 0..2400 {
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
