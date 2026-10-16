//! A depth-sorted, translucent wire sculpture of audio-reactive membranes.
//! All simulation is bounded UI work; no audio callback or platform dependencies.
use super::{camera_gizmo::project_direction, mix, properties_combo, settings_panel};
use crate::{analysis::AnalysisFrame, help::HoverHelp, parameter::Parameter, theme::AppTheme};
use eframe::egui::{self, Color32, Pos2, Rect, Vec2};
use std::{f32::consts::TAU, time::Instant};

const MAX_LAYERS: usize = 12;
const LOW_HZ: f32 = 20.0;
const HIGH_HZ: f32 = 20_000.0;

fn layer_range(layer: u32, layers: u32) -> (f32, f32) {
    let edge = |index: u32| LOW_HZ * (HIGH_HZ / LOW_HZ).powf(index as f32 / layers as f32);
    (edge(layer), edge(layer + 1))
}

// Each layer owns a nonoverlapping logarithmic band. Summed bin power keeps the
// response independent of the number of FFT bins in a band (no bin-count average).
fn layer_targets(frame: &AnalysisFrame, settings: &StrataSettings) -> [f32; MAX_LAYERS] {
    let mut powers = [0.0_f32; MAX_LAYERS];
    let edges: Vec<_> = (0..settings.layers)
        .map(|i| layer_range(i, settings.layers).1)
        .collect();
    for bin in &frame.bins {
        if !bin.frequency_hz.is_finite()
            || !(LOW_HZ..=HIGH_HZ).contains(&bin.frequency_hz)
            || !bin.magnitude.is_finite()
            || bin.magnitude <= 0.0
        {
            continue;
        }
        let layer = edges
            .partition_point(|edge| bin.frequency_hz >= *edge)
            .min(settings.layers as usize - 1);
        powers[layer] += bin.magnitude.powi(2);
    }
    powers.map(|power| {
        super::reactive::amplitude_response(power.sqrt(), settings.sensitivity, settings.gate_db)
    })
}

// Blend the existing bass/mid/high controls by overlap, including broad bands
// when only two layers are present. Their weights always sum to one.
fn layer_weights(layer: u32, layers: u32) -> [f32; 3] {
    let (low, high) = layer_range(layer, layers);
    let width = (high / low).ln();
    [(LOW_HZ, 250.0_f32), (250.0, 4000.0), (4000.0, HIGH_HZ)]
        .map(|(a, b)| (high.min(b) / low.max(a)).ln().max(0.0) / width)
}

fn range_label(layer: u32, layers: u32) -> String {
    let (low, high) = layer_range(layer, layers);
    format!("Layer {}: {:.0}–{:.0} Hz", layer + 1, low, high)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Palette {
    Kiln,
    Glacier,
    Pearl,
    Theme,
}
impl Palette {
    fn label(self) -> &'static str {
        match self {
            Self::Kiln => "Kiln · ivory / coral",
            Self::Glacier => "Glacier · blue / ice",
            Self::Pearl => "Pearl · monochrome",
            Self::Theme => "Theme accent",
        }
    }
    fn colors(self, theme: &AppTheme) -> (Color32, Color32) {
        match self {
            Self::Kiln => (
                Color32::from_rgb(225, 218, 170),
                Color32::from_rgb(255, 76, 50),
            ),
            Self::Glacier => (
                Color32::from_rgb(160, 227, 245),
                Color32::from_rgb(67, 100, 250),
            ),
            Self::Pearl => (theme.foreground, theme.foreground),
            Self::Theme => (theme.foreground, theme.accent),
        }
    }
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub(crate) struct StrataSettings {
    layers: u32,
    resolution: u32,
    width: f32,
    depth: f32,
    height: f32,
    sag: f32,
    tension: f32,
    speed: f32,
    ripple_scale: f32,
    turbulence: f32,
    twist: f32,
    sensitivity: f32,
    gate_db: f32,
    attack_ms: f32,
    release_ms: f32,
    bass_bend: f32,
    mid_ripple: f32,
    high_shimmer: f32,
    particles: u32,
    grains: u32,
    seed: u32,
    lift: f32,
    spread: f32,
    grain_size: f32,
    point_size: f32,
    trails: f32,
    palette: Palette,
    line_width: f32,
    mesh_opacity: f32,
    particle_opacity: f32,
    glow: f32,
    show_frame: bool,
    show_layers: bool,
    labels: bool,
    yaw: f32,
    elevation: f32,
    zoom: f32,
    auto_orbit: bool,
    orbit_speed: f32,
}
impl Default for StrataSettings {
    fn default() -> Self {
        Self {
            layers: 7,
            resolution: 24,
            width: 1.8,
            depth: 1.4,
            height: 2.2,
            sag: 0.6,
            tension: 0.5,
            speed: 0.7,
            ripple_scale: 2.6,
            turbulence: 0.4,
            twist: 0.18,
            sensitivity: 1.25,
            gate_db: -66.0,
            attack_ms: 55.0,
            release_ms: 700.0,
            bass_bend: 0.9,
            mid_ripple: 0.7,
            high_shimmer: 0.6,
            particles: 260,
            grains: 18,
            seed: 23,
            lift: 0.45,
            spread: 0.85,
            grain_size: 0.09,
            point_size: 1.15,
            trails: 0.35,
            palette: Palette::Kiln,
            line_width: 0.65,
            mesh_opacity: 0.52,
            particle_opacity: 0.55,
            glow: 0.25,
            show_frame: true,
            show_layers: true,
            labels: true,
            yaw: -32.0,
            elevation: 18.0,
            zoom: 1.0,
            auto_orbit: true,
            orbit_speed: 2.0,
        }
    }
}
impl StrataSettings {
    fn sanitize(&mut self) {
        self.layers = self.layers.clamp(2, 12);
        self.resolution = self.resolution.clamp(8, 36);
        self.particles = self.particles.min(900);
        self.grains = self.grains.min(48);
        self.seed = self.seed.min(9999);
        let defaults = Self::default();
        macro_rules! bound { ($($field:ident: $lo:expr, $hi:expr);* $(;)?) => {$(
            self.$field = if self.$field.is_finite() { self.$field.clamp($lo, $hi) } else { defaults.$field };
        )*}; }
        bound! {
            width: 0.5, 3.0; depth: 0.5, 3.0; height: 0.5, 3.5;
            sag: 0.0, 1.0; tension: 0.0, 1.0; speed: 0.0, 2.0;
            ripple_scale: 0.5, 6.0; turbulence: 0.0, 1.0; twist: 0.0, 1.0;
            sensitivity: 0.0, 4.0; gate_db: -90.0, -20.0;
            attack_ms: 5.0, 500.0; release_ms: 50.0, 3000.0;
            bass_bend: 0.0, 2.0; mid_ripple: 0.0, 2.0; high_shimmer: 0.0, 2.0;
            lift: -1.0, 1.0; spread: 0.1, 1.0; grain_size: 0.025, 0.18;
            point_size: 0.4, 3.0; trails: 0.0, 1.0;
            line_width: 0.25, 2.0; mesh_opacity: 0.0, 1.0; particle_opacity: 0.0, 1.0; glow: 0.0, 1.0;
            yaw: -180.0, 180.0; elevation: -70.0, 70.0; zoom: 0.4, 2.5; orbit_speed: -12.0, 12.0;
        }
    }
}

#[derive(Default)]
pub struct Strata {
    settings: StrataSettings,
    last_time: Option<Instant>,
    phase: f64,
    orbit: f64,
    energy: [f32; 4],
    layer_energy: [f32; MAX_LAYERS],
    layer_phase: [f64; MAX_LAYERS],
    mapped_layers: u32,
}
impl Strata {
    pub(crate) fn settings_snapshot(&self) -> StrataSettings {
        self.settings.clone()
    }
    pub(crate) fn apply_settings(&mut self, settings: &StrataSettings) {
        self.settings = settings.clone();
        self.settings.sanitize();
    }
    pub fn clear(&mut self) {
        self.last_time = None;
        self.phase = 0.0;
        self.orbit = 0.0;
        self.energy = [0.0; 4];
        self.layer_energy = [0.0; MAX_LAYERS];
        self.layer_phase = [0.0; MAX_LAYERS];
        self.mapped_layers = 0;
    }
    pub fn reset_settings(&mut self) {
        self.settings = StrataSettings::default();
        self.orbit = 0.0;
    }
    fn reset_camera(&mut self) {
        let defaults = StrataSettings::default();
        self.settings.yaw = defaults.yaw;
        self.settings.elevation = defaults.elevation;
        self.settings.zoom = defaults.zoom;
        self.orbit = 0.0;
    }
    pub fn controls(&mut self, ui: &mut egui::Ui) {
        let s = &mut self.settings;
        macro_rules! p {
            ($ui:ident, $field:ident, $range:expr, $label:expr, $help:expr) => {
                $ui.add(
                    Parameter::new(&mut s.$field, $range, StrataSettings::default().$field)
                        .text($label),
                )
                .help_text($help);
            };
        }
        settings_panel(ui, "Geometry", |ui| {
            p!(
                ui,
                layers,
                2..=12,
                "Layers",
                "One logarithmic frequency band per layer, from 20 Hz at the bottom to 20 kHz at the top. Bands redistribute automatically. Movement stays within each layer's space."
            );
            ui.small("20 Hz → 20 kHz · bottom to top").on_hover_text(
                (0..s.layers)
                    .map(|layer| range_label(layer, s.layers))
                    .collect::<Vec<_>>()
                    .join("\n"),
            );
            p!(
                ui,
                resolution,
                8..=36,
                "Mesh detail",
                "Grid divisions per surface. Higher detail and more layers increase drawing cost."
            );
            p!(
                ui,
                width,
                0.5..=3.0,
                "Width",
                "Width of the sculpture in scene units."
            );
            p!(
                ui,
                depth,
                0.5..=3.0,
                "Depth",
                "Front-to-back extent of each layer."
            );
            p!(
                ui,
                height,
                0.5..=3.5,
                "Stack height",
                "Vertical separation of the layers and height of the surrounding frame."
            );
            p!(
                ui,
                sag,
                0.0..=1.0,
                "Membrane sag",
                "Depth of the hanging bowl shape. Bass deepens this deformation."
            );
            p!(
                ui,
                tension,
                0.0..=1.0,
                "Edge tension",
                "How strongly deformation concentrates toward the center. Edges remain anchored."
            );
        });
        settings_panel(ui, "Flow & Motion", |ui| {
            p!(
                ui,
                speed,
                0.0..=2.0,
                "Motion speed",
                "Continuous wave and particle clock. Zero holds ambient motion; audio deformation remains active."
            );
            p!(
                ui,
                ripple_scale,
                0.5..=6.0,
                "Ripple frequency",
                "Number of wave undulations across each membrane."
            );
            p!(
                ui,
                turbulence,
                0.0..=1.0,
                "Turbulence",
                "Add crossing currents to the surface waves."
            );
            p!(
                ui,
                twist,
                0.0..=1.0,
                "Layer twist",
                "Gently rotate the membranes relative to each other inside the frame."
            );
        });
        settings_panel(ui, "Signal Response", |ui| {
            p!(
                ui,
                sensitivity,
                0.0..=4.0,
                "Sensitivity",
                "Visual response gain. Zero leaves only ambient movement; audio volume is unchanged."
            );
            p!(
                ui,
                gate_db,
                -90.0..=-20.0,
                "Noise floor dB",
                "Ignore energy below this level."
            );
            p!(
                ui,
                attack_ms,
                5.0..=500.0,
                "Attack ms",
                "How quickly the sculpture responds to rising energy."
            );
            p!(
                ui,
                release_ms,
                50.0..=3000.0,
                "Release ms",
                "How slowly the layers and highlights settle after a sound."
            );
            p!(
                ui,
                bass_bend,
                0.0..=2.0,
                "Bass bend",
                "Strength of low-frequency deformation and flow in layers covering 20–250 Hz. Each layer listens only to its assigned band."
            );
            p!(
                ui,
                mid_ripple,
                0.0..=2.0,
                "Mid ripples",
                "Strength of traveling waves in layers covering 250 Hz–4 kHz, driven by each layer's assigned band."
            );
            p!(
                ui,
                high_shimmer,
                0.0..=2.0,
                "High shimmer",
                "Fine waves and highlights in layers covering 4–20 kHz; also energizes particles and wire grains."
            );
        });
        settings_panel(ui, "Particles", |ui| {
            p!(
                ui,
                particles,
                0..=900,
                "Dust count",
                "Small luminous motes drifting through the stack. Zero hides them."
            );
            p!(
                ui,
                grains,
                0..=48,
                "Wire grains",
                "Slowly tumbling faceted wire spheres. Zero hides them."
            );
            p!(
                ui,
                seed,
                0..=9999,
                "Pattern seed",
                "Repeatable particle arrangement stored in presets."
            );
            p!(
                ui,
                lift,
                -1.0..=1.0,
                "Lift",
                "Positive values rise; negative values fall. Particles fade at the frame boundaries."
            );
            p!(
                ui,
                spread,
                0.1..=1.0,
                "Cloud spread",
                "Concentrate the particles around the center or spread them across the volume."
            );
            p!(
                ui,
                grain_size,
                0.025..=0.18,
                "Grain radius",
                "Wire-sphere size relative to the sculpture."
            );
            p!(
                ui,
                point_size,
                0.4..=3.0,
                "Dust size",
                "Point radius in screen points."
            );
            p!(
                ui,
                trails,
                0.0..=1.0,
                "Dust trails",
                "Short motion streaks follow the current flow; zero hides them."
            );
        });
        settings_panel(ui, "Appearance", |ui| {
            properties_combo("strata-palette", 176.0, 4)
                .selected_text(s.palette.label())
                .show_ui(ui, |ui| {
                    for palette in [
                        Palette::Kiln,
                        Palette::Glacier,
                        Palette::Pearl,
                        Palette::Theme,
                    ] {
                        ui.selectable_value(&mut s.palette, palette, palette.label());
                    }
                })
                .response
                .help_text("Cool mesh edges transition to warm, energy-driven highlights.");
            p!(
                ui,
                line_width,
                0.25..=2.0,
                "Wire width",
                "Membrane wire thickness in screen points."
            );
            p!(
                ui,
                mesh_opacity,
                0.0..=1.0,
                "Mesh opacity",
                "Visibility of the suspended wire surfaces."
            );
            p!(
                ui,
                particle_opacity,
                0.0..=1.0,
                "Particle opacity",
                "Visibility of dust and wire grains."
            );
            p!(
                ui,
                glow,
                0.0..=1.0,
                "Glow",
                "Soft halos around particles and the strongest surface highlights."
            );
            ui.checkbox(&mut s.show_layers, "Show membranes");
            ui.checkbox(&mut s.show_frame, "Show volume frame");
            ui.checkbox(&mut s.labels, "Show navigation hints");
        });
        settings_panel(ui, "Camera", |ui| {
            p!(
                ui,
                yaw,
                -180.0..=180.0,
                "Rotation °",
                "Rotate around the vertical axis. Drag the scene to orbit directly."
            );
            p!(
                ui,
                elevation,
                -70.0..=70.0,
                "Elevation °",
                "Raise or lower the viewing angle."
            );
            p!(
                ui,
                zoom,
                0.4..=2.5,
                "Zoom",
                "Magnification; scrolling over the scene also adjusts zoom."
            );
            ui.checkbox(&mut s.auto_orbit, "Auto orbit").help_text(
                "Slowly orbit the sculpture. Dragging stops auto orbit at the current angle.",
            );
            p!(
                ui,
                orbit_speed,
                -12.0..=12.0,
                "Orbit °/s",
                "Automatic camera rotation speed and direction; independent of the wave clock."
            );
        });
    }
    fn advance(&mut self, now: Instant, frame: &AnalysisFrame, live: bool) {
        if self.mapped_layers != self.settings.layers {
            // Don't carry bass energy into a newly treble-mapped layer after a count change.
            self.layer_energy = [0.0; MAX_LAYERS];
            self.layer_phase = [self.phase; MAX_LAYERS];
            self.mapped_layers = self.settings.layers;
        }
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
        for (energy, target) in self.energy.iter_mut().zip(target) {
            let ms = if target > *energy {
                self.settings.attack_ms
            } else {
                self.settings.release_ms
            };
            *energy += (target - *energy) * (1.0 - (-dt * 1000.0 / ms).exp());
        }
        let targets = if live {
            layer_targets(frame, &self.settings)
        } else {
            [0.0; MAX_LAYERS]
        };
        for (layer, target) in targets
            .iter()
            .enumerate()
            .take(self.settings.layers as usize)
        {
            let energy = &mut self.layer_energy[layer];
            let ms = if target > energy {
                self.settings.attack_ms
            } else {
                self.settings.release_ms
            };
            *energy += (target - *energy) * (1.0 - (-dt * 1000.0 / ms).exp());
            let weights = layer_weights(layer as u32, self.settings.layers);
            let gain = weights[0] * self.settings.bass_bend
                + weights[1] * self.settings.mid_ripple
                + weights[2] * self.settings.high_shimmer;
            self.layer_phase[layer] +=
                f64::from(dt * self.settings.speed * (0.65 + *energy * gain * 0.4));
        }
        self.phase += f64::from(dt * self.settings.speed * 0.65);
        if self.settings.auto_orbit {
            self.orbit = (self.orbit + f64::from(dt * self.settings.orbit_speed)).rem_euclid(360.0);
        }
    }

    fn layer_response(&self, layer: u32) -> [f32; 3] {
        let s = &self.settings;
        let energy = self.layer_energy[layer as usize];
        let weights = layer_weights(layer, s.layers);
        [
            energy * weights[0] * s.bass_bend,
            energy * weights[1] * s.mid_ripple,
            energy * weights[2] * s.high_shimmer,
        ]
    }

    #[cfg(test)]
    fn surface(&self, layer: u32, u: f32, v: f32) -> ([f32; 3], f32) {
        self.surface_with_response(layer, u, v, self.layer_response(layer))
    }

    fn surface_with_response(
        &self,
        layer: u32,
        u: f32,
        v: f32,
        [bass, mid, high]: [f32; 3],
    ) -> ([f32; 3], f32) {
        let s = &self.settings;
        let t = self.layer_phase[layer as usize];
        let l = layer as f32 / (s.layers - 1) as f32;
        let spacing = s.height / (s.layers - 1) as f32;
        let envelope = ((1.0 - u * u) * (1.0 - v * v))
            .max(0.0)
            .powf(0.6 + s.tension * 1.6);
        let wave = (f64::from(u * s.ripple_scale + v * 1.8 + l * 2.4) - t * 1.7).sin() as f32;
        let cross =
            (f64::from(v * s.ripple_scale * 1.4 - u * 2.7 + l * 3.5) + t * 1.2).sin() as f32;
        let shimmer = (f64::from((u + v) * s.ripple_scale * 2.5) - t * 3.1).sin() as f32;
        let bend = s.sag * (0.3 + bass * 0.25);
        let ripple =
            wave * (0.08 + mid * 0.18) + cross * s.turbulence * 0.13 + shimmer * high * 0.12;
        // A strict fraction of layer spacing bounds motion even at maximum gain.
        let displacement = (ripple - bend).tanh() * envelope * spacing * 0.44;
        let angle = s.twist * 0.35 * (t * 0.3 + f64::from(l * 3.0)).sin() as f32;
        // Twist an inscribed rectangle, so no layer escapes the volume frame.
        let fit = 1.0
            / (angle.cos().abs() + angle.sin().abs() * (s.width / s.depth).max(s.depth / s.width));
        let x = u * s.width * 0.5 * fit;
        let z = v * s.depth * 0.5 * fit;
        (
            [
                x * angle.cos() - z * angle.sin(),
                (l - 0.5) * s.height + displacement,
                x * angle.sin() + z * angle.cos(),
            ],
            (envelope
                * (0.25
                    + displacement.abs() / spacing * 1.7
                    + (bass + mid + high) * 0.4
                    + (mid * wave.abs() + high * shimmer.abs()) * 0.2))
                .clamp(0.0, 1.0),
        )
    }
    fn particle(&self, index: u32, lag: f64) -> ([f32; 3], f32) {
        let s = &self.settings;
        let hash = |salt| random(index.wrapping_add(s.seed.wrapping_mul(8191)), salt);
        let t = self.phase - lag;
        let cycle = (f64::from(hash(1)) + t * f64::from(s.lift) * 0.12).rem_euclid(1.0) as f32;
        let fade = (cycle.min(1.0 - cycle) * 12.0).clamp(0.0, 1.0);
        let a = f64::from(hash(2) * TAU) + t * 0.12;
        let radius = hash(3).sqrt() * s.spread * 0.43;
        let wobble = (t * 0.6 + f64::from(hash(4) * TAU)).sin() as f32 * 0.05 * s.turbulence;
        (
            [
                (a.cos() as f32 * radius + wobble) * s.width,
                (cycle - 0.5) * s.height,
                (a.sin() as f32 * radius - wobble) * s.depth,
            ],
            fade,
        )
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
        if rect.width() < 4.0 || rect.height() < 4.0 {
            return;
        }
        self.advance(now, frame, live);
        let response = ui.interact(
            rect,
            ui.id().with("strata-orbit"),
            egui::Sense::click_and_drag(),
        );
        if response.dragged() {
            let delta = ui.input(|i| i.pointer.delta());
            self.settings.yaw =
                wrap_degrees(self.settings.yaw + self.orbit as f32 + delta.x * 0.35);
            self.orbit = 0.0;
            self.settings.elevation = (self.settings.elevation + delta.y * 0.25).clamp(-70.0, 70.0);
            self.settings.auto_orbit = false;
        }
        if response.double_clicked() {
            self.reset_camera();
        }
        if response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            self.settings.zoom = (self.settings.zoom * (scroll * 0.002).exp()).clamp(0.4, 2.5);
        }
        let camera = Camera::fit(rect.shrink(24.0), &self.settings, self.orbit as f32);
        let items = self.scene(&camera, theme);
        let mut mesh = egui::Mesh::default();
        for item in items {
            match item.shape {
                Primitive::Line(a, b, width) => {
                    if item.glow > 0.0 {
                        mesh_line(
                            &mut mesh,
                            a,
                            b,
                            width + 2.5,
                            item.color.gamma_multiply(item.glow * 0.12),
                        );
                    }
                    mesh_line(&mut mesh, a, b, width, item.color);
                }
                Primitive::Dot(p, radius) => {
                    if item.glow > 0.0 {
                        mesh_dot(
                            &mut mesh,
                            p,
                            radius * 3.0,
                            item.color.gamma_multiply(item.glow * 0.12),
                        );
                    }
                    mesh_dot(&mut mesh, p, radius, item.color);
                }
            }
        }
        ui.painter_at(rect).add(egui::Shape::mesh(mesh));
        if self.settings.labels && rect.width() > 290.0 && rect.height() > 180.0 {
            ui.painter_at(rect).text(
                rect.left_bottom() + Vec2::new(10.0, -8.0),
                egui::Align2::LEFT_BOTTOM,
                "Drag: orbit · Scroll: zoom · Double-click: reset view",
                egui::FontId::monospace(9.0),
                theme.muted,
            );
        }
    }
    fn scene(&self, camera: &Camera, theme: &AppTheme) -> Vec<DrawItem> {
        let s = &self.settings;
        let (cool, hot) = s.palette.colors(theme);
        let n = s.resolution as usize;
        let mut items = Vec::with_capacity(
            s.layers as usize * n * (n + 1) * 2 + s.particles as usize * 2 + s.grains as usize * 30,
        );
        let add_line = |items: &mut Vec<DrawItem>, a, b, color, width, glow| {
            let (a, da) = camera.project(a);
            let (b, db) = camera.project(b);
            items.push(DrawItem {
                depth: (da + db) * 0.5,
                color,
                glow,
                shape: Primitive::Line(a, b, width),
            });
        };
        if s.show_frame {
            let corners = corners(
                s.width * 0.51,
                s.height * 0.5 + s.grain_size,
                s.depth * 0.51,
            );
            for i in 0..8 {
                for bit in [1, 2, 4] {
                    let j = i ^ bit;
                    if i < j {
                        add_line(
                            &mut items,
                            corners[i],
                            corners[j],
                            theme.foreground.gamma_multiply(0.14),
                            0.6,
                            0.0,
                        );
                    }
                }
            }
        }
        if s.show_layers {
            for layer in 0..s.layers {
                // Frequency overlap/gain is constant across this entire membrane.
                let response = self.layer_response(layer);
                let points: Vec<_> = (0..=n)
                    .flat_map(|z| (0..=n).map(move |x| (x, z)))
                    .map(|(x, z)| {
                        self.surface_with_response(
                            layer,
                            x as f32 / n as f32 * 2.0 - 1.0,
                            z as f32 / n as f32 * 2.0 - 1.0,
                            response,
                        )
                    })
                    .collect();
                for z in 0..=n {
                    for x in 0..=n {
                        let index = z * (n + 1) + x;
                        for next in [
                            if x < n { Some(index + 1) } else { None },
                            if z < n { Some(index + n + 1) } else { None },
                        ]
                        .into_iter()
                        .flatten()
                        {
                            let (a, wa) = points[index];
                            let (b, wb) = points[next];
                            let heat = (wa + wb) * 0.5;
                            let color = mix(cool, hot, heat)
                                .gamma_multiply(s.mesh_opacity * (0.40 + heat * 0.60));
                            add_line(
                                &mut items,
                                a,
                                b,
                                color,
                                s.line_width,
                                if heat > 0.5 { s.glow } else { 0.0 },
                            );
                        }
                    }
                }
            }
        }
        for i in 0..s.particles {
            let (p, fade) = self.particle(i, 0.0);
            let brightness =
                (0.35 + random(i, 8) * 0.4 + self.energy[2] * s.high_shimmer * 0.4).min(1.0);
            let color = mix(theme.foreground, cool, 0.35)
                .gamma_multiply(s.particle_opacity * fade * brightness);
            let (screen, depth) = camera.project(p);
            items.push(DrawItem {
                depth,
                color,
                glow: s.glow,
                shape: Primitive::Dot(screen, s.point_size * (0.55 + brightness * 0.65)),
            });
            if s.trails > 0.0 {
                let (tail, _) = self.particle(i, f64::from(s.trails * 0.35));
                // Skip the wrap segment so a fading particle never draws a streak through the entire stack.
                if (p[1] - tail[1]).abs() < s.height * 0.25 {
                    add_line(&mut items, p, tail, color.gamma_multiply(0.5), 0.5, 0.0);
                }
            }
        }
        let (vertices, edges) = icosahedron();
        for i in 0..s.grains {
            let (center, fade) = self.particle(i + 1009, 0.0);
            let radius = s.grain_size
                * (0.65 + random(i, 15) * 0.5)
                * (1.0 + self.energy[2] * s.high_shimmer * 0.3);
            let angle = (self.phase * 0.4 + f64::from(random(i, 16) * TAU))
                .rem_euclid(f64::from(TAU)) as f32;
            let transformed: Vec<_> = vertices
                .iter()
                .map(|p| {
                    let x = p[0] * angle.cos() - p[2] * angle.sin();
                    let z = p[0] * angle.sin() + p[2] * angle.cos();
                    [
                        center[0] + x * radius,
                        center[1] + (p[1] * angle.cos() - z * angle.sin()) * radius,
                        center[2] + (p[1] * angle.sin() + z * angle.cos()) * radius,
                    ]
                })
                .collect();
            let color = theme.foreground.gamma_multiply(
                s.particle_opacity
                    * fade
                    * (0.24 + self.energy[2] * s.high_shimmer * 0.2).min(0.75),
            );
            for &(a, b) in &edges {
                add_line(&mut items, transformed[a], transformed[b], color, 0.55, 0.0);
            }
        }
        // Positive camera depth is nearer; translucent primitives paint back to front.
        items.sort_by(|a, b| a.depth.total_cmp(&b.depth));
        items
    }
}

fn random(index: u32, salt: u32) -> f32 {
    let mut x = index
        .wrapping_mul(747_796_405)
        .wrapping_add(salt.wrapping_mul(2_891_336_453));
    x = ((x >> ((x >> 28) + 4)) ^ x).wrapping_mul(277_803_737);
    (((x >> 22) ^ x) >> 8) as f32 / 16_777_216.0
}
fn wrap_degrees(degrees: f32) -> f32 {
    (degrees + 180.0).rem_euclid(360.0) - 180.0
}
fn corners(x: f32, y: f32, z: f32) -> [[f32; 3]; 8] {
    std::array::from_fn(|i| {
        [
            if i & 1 == 0 { -x } else { x },
            if i & 2 == 0 { -y } else { y },
            if i & 4 == 0 { -z } else { z },
        ]
    })
}
struct Camera {
    yaw: f32,
    elevation: f32,
    scale: f32,
    center: Pos2,
}
impl Camera {
    fn fit(rect: Rect, settings: &StrataSettings, orbit: f32) -> Self {
        let yaw = (settings.yaw + orbit).to_radians();
        let elevation = settings.elevation.to_radians();
        let mut projected = Rect::NOTHING;
        for corner in corners(
            settings.width * 0.5 + settings.grain_size,
            settings.height * 0.5 + settings.grain_size,
            settings.depth * 0.5 + settings.grain_size,
        ) {
            projected.extend_with(Pos2::ZERO + project_direction(yaw, elevation, corner).0);
        }
        let scale = (rect.width().max(1.0) / projected.width())
            .min(rect.height().max(1.0) / projected.height())
            * settings.zoom;
        Self {
            yaw,
            elevation,
            scale,
            center: rect.center(),
        }
    }
    fn project(&self, p: [f32; 3]) -> (Pos2, f32) {
        let (point, depth) = project_direction(self.yaw, self.elevation, p);
        (self.center + point * self.scale, depth)
    }
}
enum Primitive {
    Line(Pos2, Pos2, f32),
    Dot(Pos2, f32),
}
struct DrawItem {
    depth: f32,
    color: Color32,
    glow: f32,
    shape: Primitive,
}

fn icosahedron() -> (Vec<[f32; 3]>, Vec<(usize, usize)>) {
    let phi = (1.0 + 5.0_f32.sqrt()) * 0.5;
    let mut points = Vec::with_capacity(12);
    for a in [-1.0, 1.0] {
        for b in [-phi, phi] {
            points.extend([[0.0, a, b], [a, b, 0.0], [b, 0.0, a]]);
        }
    }
    let mut edges = Vec::with_capacity(30);
    for i in 0..points.len() {
        for j in i + 1..points.len() {
            let distance: f32 = (0..3)
                .map(|axis| (points[i][axis] - points[j][axis]).powi(2))
                .sum();
            if (distance - 4.0).abs() < 0.001 {
                edges.push((i, j));
            }
        }
    }
    let length = (1.0 + phi * phi).sqrt();
    for p in &mut points {
        for coordinate in p {
            *coordinate /= length;
        }
    }
    (points, edges)
}
fn mesh_line(mesh: &mut egui::Mesh, a: Pos2, b: Pos2, width: f32, color: Color32) {
    let direction = b - a;
    if direction.length_sq() < 0.0001 {
        return;
    }
    let normal = Vec2::new(-direction.y, direction.x).normalized();
    let inner = normal * width * 0.5;
    let outer = normal * (width * 0.5 + 0.6);
    let base = mesh.vertices.len() as u32;
    for (p, c) in [
        (a - outer, Color32::TRANSPARENT),
        (b - outer, Color32::TRANSPARENT),
        (a - inner, color),
        (b - inner, color),
        (a + inner, color),
        (b + inner, color),
        (a + outer, Color32::TRANSPARENT),
        (b + outer, Color32::TRANSPARENT),
    ] {
        mesh.colored_vertex(p, c);
    }
    for row in 0..3 {
        let v = base + row * 2;
        mesh.add_triangle(v, v + 1, v + 2);
        mesh.add_triangle(v + 1, v + 3, v + 2);
    }
}
fn mesh_dot(mesh: &mut egui::Mesh, center: Pos2, radius: f32, color: Color32) {
    let base = mesh.vertices.len() as u32;
    mesh.colored_vertex(center, color);
    for i in 0..8 {
        mesh.colored_vertex(center + Vec2::angled(i as f32 * TAU / 8.0) * radius, color);
    }
    for i in 0..8 {
        mesh.add_triangle(base, base + 1 + i, base + 1 + (i + 1) % 8);
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
            mix_rms: 0.25,
            ..Default::default()
        }
    }

    #[test]
    fn membranes_stay_in_separate_slabs_with_anchored_edges() {
        let mut strata = Strata {
            phase: 8.2,
            energy: [1.0; 4],
            layer_phase: [8.2; MAX_LAYERS],
            layer_energy: [1.0; MAX_LAYERS],
            ..Default::default()
        };
        strata.settings.sag = 1.0;
        strata.settings.bass_bend = 2.0;
        strata.settings.mid_ripple = 2.0;
        strata.settings.high_shimmer = 2.0;
        strata.settings.turbulence = 1.0;
        strata.settings.twist = 1.0;
        for layers in [2, 7, 12] {
            strata.settings.layers = layers;
            for (width, depth) in [(0.5, 3.0), (3.0, 0.5), (1.8, 1.4)] {
                strata.settings.width = width;
                strata.settings.depth = depth;
                let spacing = strata.settings.height / (layers - 1) as f32;
                for layer in 0..layers {
                    let base = (layer as f32 / (layers - 1) as f32 - 0.5) * strata.settings.height;
                    for x in 0..=20 {
                        for z in 0..=20 {
                            let (p, heat) =
                                strata.surface(layer, x as f32 * 0.1 - 1.0, z as f32 * 0.1 - 1.0);
                            assert!(p.iter().all(|value| value.is_finite()));
                            assert!((p[1] - base).abs() < spacing * 0.45);
                            assert!(
                                p[0].abs() <= width * 0.5 + 0.0001
                                    && p[2].abs() <= depth * 0.5 + 0.0001
                            );
                            assert!((0.0..=1.0).contains(&heat));
                            if x == 0 || x == 20 || z == 0 || z == 20 {
                                assert!((p[1] - base).abs() < 0.0001);
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn music_deforms_membranes_and_frozen_time_stops_motion_and_orbit() {
        let start = Instant::now();
        let mut strata = Strata::default();
        let silent = strata.surface(3, 0.2, 0.3);
        for i in 0..=60 {
            strata.advance(start + Duration::from_millis(i * 16), &signal(), true);
        }
        assert!(strata.energy.iter().all(|e| *e > 0.5));
        let active = strata.surface(3, 0.2, 0.3);
        assert_ne!(active, silent);
        let particle = strata.particle(6, 0.0);
        let orbit = strata.orbit;
        let layers = strata.layer_energy;
        let phases = strata.layer_phase;
        for _ in 0..120 {
            strata.advance(
                start + Duration::from_millis(60 * 16),
                &AnalysisFrame::default(),
                true,
            );
        }
        assert_eq!(strata.surface(3, 0.2, 0.3), active);
        assert_eq!(strata.particle(6, 0.0), particle);
        assert_eq!(strata.orbit, orbit);
        assert_eq!(strata.layer_energy, layers);
        assert_eq!(strata.layer_phase, phases);
        for i in 61..=500 {
            strata.advance(start + Duration::from_millis(i * 16), &signal(), false);
        }
        assert!(strata.energy.iter().all(|e| *e < 0.001));
        assert!(strata.layer_energy.iter().all(|e| *e < 0.001));
    }

    #[test]
    fn logarithmic_bands_cover_spectrum_and_each_tone_moves_only_its_layer() {
        let start = Instant::now();
        for count in 2..=12 {
            let mut previous_high = LOW_HZ;
            for layer in 0..count {
                let (low, high) = layer_range(layer, count);
                assert!((low - previous_high).abs() < 0.01);
                assert!(high > low);
                previous_high = high;
                let weights = layer_weights(layer, count);
                assert!((weights.iter().sum::<f32>() - 1.0).abs() < 0.0001);
                let frame = AnalysisFrame {
                    bins: vec![FrequencyBin {
                        frequency_hz: (low * high).sqrt(),
                        magnitude: 0.3,
                        dbfs: -10.0,
                    }],
                    mix_rms: 0.3,
                    ..Default::default()
                };
                let mut active = Strata::default();
                active.settings.layers = count;
                let mut silent = Strata::default();
                silent.settings.layers = count;
                for i in 0..=40 {
                    let now = start + Duration::from_millis(i * 16);
                    active.advance(now, &frame, true);
                    silent.advance(now, &AnalysisFrame::default(), true);
                }
                for other in 0..count {
                    if other == layer {
                        assert!(active.layer_energy[other as usize] > 0.5);
                        assert_ne!(
                            active.surface(other, 0.2, 0.3),
                            silent.surface(other, 0.2, 0.3)
                        );
                    } else {
                        assert_eq!(active.layer_energy[other as usize], 0.0);
                        assert_eq!(
                            active.surface(other, 0.2, 0.3),
                            silent.surface(other, 0.2, 0.3)
                        );
                    }
                }
            }
            assert!((previous_high - HIGH_HZ).abs() < 0.1);
        }
    }

    #[test]
    fn frequency_boundaries_are_unique_and_count_changes_remap_without_stale_energy() {
        let start = Instant::now();
        let mut strata = Strata::default();
        let tone = |frequency_hz| AnalysisFrame {
            bins: vec![FrequencyBin {
                frequency_hz,
                magnitude: 0.3,
                dbfs: -10.0,
            }],
            ..Default::default()
        };
        for count in [2, 7, 12] {
            strata.settings.layers = count;
            for layer in 0..count {
                let (low, high) = layer_range(layer, count);
                let targets = layer_targets(&tone(low), &strata.settings);
                assert!(targets[layer as usize] > 0.5);
                assert_eq!(targets.iter().filter(|e| **e > 0.0).count(), 1);
                if layer == count - 1 {
                    assert!(layer_targets(&tone(high), &strata.settings)[layer as usize] > 0.5);
                }
            }
        }
        strata.settings.layers = 7;
        for i in 0..=40 {
            strata.advance(start + Duration::from_millis(i * 16), &tone(80.0), true);
        }
        assert!(strata.layer_energy[1] > 0.5);
        strata.settings.layers = 12;
        // Count changes also remap safely while paused, without advancing clocks.
        strata.advance(start + Duration::from_millis(40 * 16), &tone(80.0), true);
        assert_eq!(strata.layer_energy, [0.0; MAX_LAYERS]);
        for i in 41..=80 {
            strata.advance(start + Duration::from_millis(i * 16), &tone(80.0), true);
        }
        assert!(strata.layer_energy[2] > 0.5);
        assert_eq!(strata.layer_energy[1], 0.0);
        for frequency in [0.0, 19.0, 20_001.0, f32::NAN, f32::INFINITY] {
            assert_eq!(
                layer_targets(&tone(frequency), &strata.settings),
                [0.0; MAX_LAYERS]
            );
        }
        let mut invalid = tone(80.0);
        invalid.bins[0].magnitude = f32::NAN;
        assert_eq!(layer_targets(&invalid, &strata.settings), [0.0; MAX_LAYERS]);
        strata.settings.sensitivity = 0.0;
        assert_eq!(
            layer_targets(&tone(80.0), &strata.settings),
            [0.0; MAX_LAYERS]
        );
        strata.clear();
        assert_eq!(strata.layer_energy, [0.0; MAX_LAYERS]);
        assert_eq!(strata.layer_phase, [0.0; MAX_LAYERS]);
    }

    #[test]
    fn particles_wrap_invisibly_and_settings_exclude_simulation() {
        let mut strata = Strata {
            phase: 12.4,
            orbit: 80.0,
            energy: [0.8; 4],
            ..Default::default()
        };
        for i in 0..900 {
            let (p, alpha) = strata.particle(i, 0.0);
            assert!(p.iter().all(|v| v.is_finite()));
            assert!(p[1].abs() <= strata.settings.height * 0.5);
            assert!((0.0..=1.0).contains(&alpha));
        }
        let encoded = serde_json::to_string(&strata.settings_snapshot()).unwrap();
        assert!(
            !encoded.contains("energy")
                && !encoded.contains("phase")
                && !encoded.contains("last_time")
        );
        let settings = serde_json::from_str(&encoded).unwrap();
        strata.apply_settings(&settings);
        assert_eq!(strata.settings_snapshot(), settings);
        let mut unsafe_settings = settings;
        unsafe_settings.layers = 0;
        unsafe_settings.resolution = u32::MAX;
        unsafe_settings.particles = u32::MAX;
        unsafe_settings.zoom = f32::NAN;
        unsafe_settings.release_ms = 0.0;
        strata.apply_settings(&unsafe_settings);
        assert_eq!(strata.settings.layers, 2);
        assert_eq!(strata.settings.resolution, 36);
        assert_eq!(strata.settings.particles, 900);
        assert!(strata.settings.zoom.is_finite());
        assert_eq!(strata.settings.release_ms, 50.0);
    }

    #[test]
    fn scene_is_depth_sorted_bounded_and_has_valid_mesh_indices() {
        let (points, edges) = icosahedron();
        assert_eq!(points.len(), 12);
        assert_eq!(edges.len(), 30);
        let mut strata = Strata {
            energy: [1.0; 4],
            layer_energy: [1.0; MAX_LAYERS],
            ..Default::default()
        };
        strata.settings.layers = 12;
        strata.settings.resolution = 36;
        strata.settings.particles = 900;
        strata.settings.grains = 48;
        let theme = AppTheme::default();
        for (size, yaw, elevation) in [
            (Vec2::new(1600.0, 900.0), -32.0, 18.0),
            (Vec2::new(180.0, 300.0), 90.0, 70.0),
            (Vec2::splat(20.0), -180.0, -70.0),
        ] {
            strata.settings.yaw = yaw;
            strata.settings.elevation = elevation;
            let camera = Camera::fit(Rect::from_min_size(Pos2::ZERO, size), &strata.settings, 0.0);
            let scene = strata.scene(&camera, &theme);
            assert!(scene.len() < 36_000);
            assert!(scene.windows(2).all(|pair| pair[0].depth <= pair[1].depth));
            let mut mesh = egui::Mesh::default();
            for item in scene {
                assert!(item.depth.is_finite());
                match item.shape {
                    Primitive::Line(a, b, width) => mesh_line(&mut mesh, a, b, width, item.color),
                    Primitive::Dot(p, radius) => mesh_dot(&mut mesh, p, radius, item.color),
                }
            }
            assert!(mesh.is_valid());
            assert!(mesh.vertices.iter().all(|v| v.pos.is_finite()));
            assert!(mesh.vertices.len() < 300_000);
        }
    }

    #[test]
    #[ignore = "manual rendering timing probe"]
    fn render_timing() {
        let ctx = egui::Context::default();
        let theme = AppTheme::default();
        let frame = signal();
        let mut strata = Strata::default();
        for max_detail in [false, true] {
            if max_detail {
                strata.settings.layers = 12;
                strata.settings.resolution = 36;
                strata.settings.particles = 900;
                strata.settings.grains = 48;
            }
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
                        strata.draw(
                            ui,
                            ui.max_rect(),
                            &frame,
                            &theme,
                            start + Duration::from_millis(i * 16),
                            true,
                        );
                    },
                );
                output.textures_delta.clear();
                std::hint::black_box(ctx.tessellate(output.shapes, output.pixels_per_point));
            }
            eprintln!(
                "Strata {}: {:.2} ms/frame for UI + tessellation",
                if max_detail {
                    "maximum detail"
                } else {
                    "default"
                },
                start.elapsed().as_secs_f64() * 1000.0 / 120.0
            );
        }
    }
}
