//! Lotus Flower: artistic layered petals with direct spectrum-driven motion.
//! This is a visual primitive, not a physical black-hole simulation.
mod controls;
mod render;
module_palette!(Resonance, settings.palette);

#[cfg(test)]
mod render_reference;
#[cfg(test)]
mod tests;
use super::{
    Palette, camera_gizmo,
    frequency::{BANDS, FrequencyData, FrequencySettings},
    trackball,
};
use crate::{analysis::AnalysisFrame, help::HoverHelp, theme::AppTheme};
use egui::{self, Pos2, Rect, Sense, Vec2};
use std::{
    f32::consts::{PI, TAU},
    time::Instant,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Style {
    Membranes,
    Wireframe,
    Contours,
    Points,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum ColorBy {
    Layer,
    Petal,
    Energy,
    Position,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum SpectrumLayout {
    Layers,
    PoleToPole,
    AroundPetals,
}
impl SpectrumLayout {
    fn label(self) -> &'static str {
        match self {
            Self::Layers => "By layer",
            Self::PoleToPole => "Pole to pole",
            Self::AroundPetals => "Around petals",
        }
    }
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub(crate) struct ResonanceSettings {
    camera_motion: super::camera_motion::Settings,
    auto_orbit: bool,
    orbit_speed: f32,
    layers: usize,
    petals: usize,
    radius: f32,
    spacing: f32,
    layer_depth: f32,
    flattening: f32,
    waist: f32,
    sharpness: f32,
    twist: f32,
    opening: f32,
    detail: usize,
    flow: f32,
    spin: f32,
    ripple_speed: f32,
    ripple_amount: f32,
    ripple_count: f32,
    phase_spread: f32,
    alternate: bool,
    spectrum_layout: SpectrumLayout,
    spectrum_depth: f32,
    spectrum_visibility: f32,
    attack_accent: f32,
    spectrum_light: f32,
    audio_expand: f32,
    audio_ripple: f32,
    audio_light: f32,
    response_ms: f32,
    frequency: FrequencySettings,
    style: Style,
    palette: Palette,
    color_by: ColorBy,
    reverse: bool,
    color_shift: f32,
    contrast: f32,
    opacity: f32,
    rim: f32,
    brightness: f32,
    wire_opacity: f32,
    line_width: f32,
    grid_stride: usize,
    point_size: f32,
    black_canvas: bool,
    guides: bool,
    show_gizmo: bool,
    yaw: f32,
    elevation: f32,
    roll: f32,
    zoom: f32,
    pan: Vec2,
}
impl Default for ResonanceSettings {
    fn default() -> Self {
        Self {
            camera_motion: Default::default(),
            auto_orbit: false,
            orbit_speed: 10.0,
            layers: 9,
            petals: 2,
            radius: 0.28,
            spacing: 0.13,
            layer_depth: 0.3,
            flattening: 0.24,
            waist: 0.18,
            sharpness: 0.7,
            twist: 4.0,
            opening: 1.0,
            detail: 28,
            flow: 0.045,
            spin: 5.0,
            ripple_speed: 0.18,
            ripple_amount: 0.035,
            ripple_count: 3.0,
            phase_spread: 0.16,
            alternate: false,
            spectrum_layout: SpectrumLayout::Layers,
            spectrum_depth: 0.35,
            spectrum_visibility: 0.7,
            attack_accent: 0.15,
            spectrum_light: 0.55,
            audio_expand: 0.22,
            audio_ripple: 0.12,
            audio_light: 0.35,
            response_ms: 90.0,
            frequency: FrequencySettings {
                floor: -72.0,
                ..Default::default()
            },
            style: Style::Membranes,
            palette: Palette::Custom(crate::colors::Gradient {
                colors: [
                    [255, 205, 109],
                    [255, 241, 169],
                    [167, 218, 221],
                    [70, 147, 192],
                    [29, 65, 128],
                    [0, 0, 0],
                    [0, 0, 0],
                    [0, 0, 0],
                ],
                count: 5,
                cyclic: false,
            }),
            color_by: ColorBy::Layer,
            reverse: false,
            color_shift: 0.0,
            contrast: 1.0,
            opacity: 0.12,
            rim: 0.5,
            brightness: 1.3,
            wire_opacity: 0.0,
            line_width: 0.7,
            grid_stride: 4,
            point_size: 1.4,
            black_canvas: true,
            guides: false,
            show_gizmo: false,
            yaw: -0.45,
            elevation: 0.3,
            roll: 0.35,
            zoom: 1.0,
            pan: Vec2::ZERO,
        }
    }
}
impl ResonanceSettings {
    fn sanitize(&mut self) {
        self.layers = self.layers.clamp(1, 16);
        self.petals = self.petals.clamp(1, 8);
        self.detail = self.detail.clamp(12, 48);
        self.grid_stride = self.grid_stride.clamp(1, 12);
        let defaults = Self::default();
        macro_rules! bound { ($($field:ident: $lo:expr, $hi:expr);* $(;)?) => {$(
            self.$field = if self.$field.is_finite() { self.$field.clamp($lo, $hi) } else { defaults.$field };
        )*}; }
        bound! {
            radius: 0.05, 2.0; spacing: 0.01, 0.4; layer_depth: -3.0, 3.0;
            flattening: 0.03, 1.5; waist: 0.02, 1.0; sharpness: 0.2, 4.0; twist: -90.0, 90.0; opening: 0.1, 1.0;
            flow: -0.5, 0.5; spin: -90.0, 90.0; ripple_speed: -3.0, 3.0; ripple_amount: 0.0, 0.5; ripple_count: 0.0, 12.0; phase_spread: 0.0, 1.0;
            spectrum_depth: 0.0, 1.0; spectrum_visibility: 0.0, 1.0; attack_accent: 0.0, 1.0; spectrum_light: 0.0, 2.0;
            audio_expand: 0.0, 1.5; audio_ripple: 0.0, 0.5; audio_light: 0.0, 1.0; response_ms: 0.0, 1000.0;
            color_shift: -1.0, 1.0; contrast: 0.25, 4.0; opacity: 0.0, 1.0; rim: 0.0, 1.0; brightness: 0.0, 3.0; wire_opacity: 0.0, 1.0;
            line_width: 0.25, 4.0; point_size: 0.5, 5.0; yaw: -PI, PI; elevation: -PI, PI; roll: -PI, PI; zoom: 0.25, 8.0;
        }
        if !self.pan.is_finite() {
            self.pan = Vec2::ZERO;
        }
        self.pan = self.pan.clamp(Vec2::splat(-4.0), Vec2::splat(4.0));
    }
}
pub struct Resonance {
    settings: ResonanceSettings,
    data: FrequencyData,
    energy: f32,
    bands: [f32; BANDS],
    spectrum_gain: f32,
    attack_fast: f32,
    attack_slow: f32,
    transient: f32,
    last_draw: Option<Instant>,
    epoch: Option<u64>,
    flow_phase: f32,
    spin_phase: f32,
    ripple_phase: f32,
}
impl Default for Resonance {
    fn default() -> Self {
        Self {
            settings: Default::default(),
            data: Default::default(),
            energy: 0.0,
            bands: [0.0; BANDS],
            spectrum_gain: 1.0,
            attack_fast: 0.0,
            attack_slow: 0.0,
            transient: 0.0,
            last_draw: None,
            epoch: None,
            flow_phase: 0.0,
            spin_phase: 0.0,
            ripple_phase: 0.0,
        }
    }
}
impl Resonance {
    pub fn settings_snapshot(&self) -> ResonanceSettings {
        self.settings.clone()
    }
    pub fn apply_settings(&mut self, settings: &ResonanceSettings) {
        self.settings = settings.clone();
        self.settings.sanitize();
    }
    pub fn reset_settings(&mut self) {
        *self = Self::default();
    }
    pub fn clear(&mut self) {
        self.data.clear();
        self.energy = 0.0;
        self.bands.fill(0.0);
        self.spectrum_gain = 1.0;
        self.attack_fast = 0.0;
        self.attack_slow = 0.0;
        self.transient = 0.0;
        self.last_draw = None;
        self.epoch = None;
        self.flow_phase = 0.0;
        self.spin_phase = 0.0;
        self.ripple_phase = 0.0;
    }
    fn update(&mut self, frame: &AnalysisFrame, now: Instant, live: bool) {
        self.settings.sanitize();
        if self.epoch != Some(frame.capture_epoch) {
            self.clear();
            self.epoch = Some(frame.capture_epoch);
        }
        let dt = self.last_draw.map_or(1.0 / 60.0, |last| {
            now.saturating_duration_since(last).as_secs_f32().min(0.1)
        });
        self.last_draw = Some(now);
        let s = &self.settings;
        self.flow_phase = (self.flow_phase + dt * s.flow).rem_euclid(1.0);
        self.spin_phase = (self.spin_phase + dt * s.spin.to_radians()).rem_euclid(TAU);
        self.ripple_phase = (self.ripple_phase + dt * s.ripple_speed * TAU).rem_euclid(TAU);
        self.data.settings = s.frequency.clone();
        if live {
            self.data.update(frame, now);
        }
        let alpha = if s.response_ms <= 0.0 {
            1.0
        } else {
            -(-dt * 1000.0 / s.response_ms).exp_m1()
        };
        let rms = if live && frame.mix_rms.is_finite() {
            frame.mix_rms.max(0.0)
        } else {
            0.0
        };
        let target = s
            .frequency
            .intensity(20.0 * rms.max(1e-7).log10() + s.frequency.gain);
        let target = if target.is_finite() { target } else { 0.0 };
        self.energy += (target - self.energy) * alpha;
        // A fast/slow level follower accents attacks without pretending to
        // detect musical beats. Both envelopes use the pane's virtual clock.
        let pulse_target = rms.clamp(0.0, 16.0);
        self.attack_fast += (pulse_target - self.attack_fast) * -(-dt / 0.008).exp_m1();
        self.attack_slow += (pulse_target - self.attack_slow) * -(-dt / 0.180).exp_m1();
        let onset = (self.attack_fast - self.attack_slow).max(0.0) / self.attack_slow.max(0.001);
        let quiet_gate = ((rms - 0.001) / 0.004).clamp(0.0, 1.0);
        self.transient = (onset * 0.6).clamp(0.0, 1.0) * quiet_gate;
        for (value, db) in self.bands.iter_mut().zip(self.data.levels) {
            let target = if live && db.is_finite() {
                s.frequency.intensity(db)
            } else {
                0.0
            };
            let target = if target.is_finite() { target } else { 0.0 };
            *value += (target - *value) * alpha;
        }
        // A bounded display boost reveals quiet spectral differences without
        // changing the routed signal or amplifying a near-silent FFT floor.
        let peak = self.bands.iter().copied().fold(0.0_f32, f32::max);
        let gate = ((peak - 0.03) / 0.12).clamp(0.0, 1.0);
        self.spectrum_gain =
            1.0 + ((1.0 / peak.max(0.001)).clamp(1.0, 4.0) - 1.0) * s.spectrum_visibility * gate;
    }
    fn spectrum_level(&self, layer: usize, u: f32, v: f32) -> f32 {
        let level = match self.settings.spectrum_layout {
            SpectrumLayout::Layers => {
                let count = self.settings.layers.max(1);
                let first = (layer * BANDS / count).min(BANDS - 1);
                let last = ((layer + 1) * BANDS / count).max(first + 1).min(BANDS);
                self.bands[first..last]
                    .iter()
                    .copied()
                    .fold(0.0_f32, f32::max)
            }
            SpectrumLayout::PoleToPole => self.band(v),
            SpectrumLayout::AroundPetals => {
                // Mirror the spectrum in each petal, retaining the closed seam.
                let phase = (u * self.settings.petals as f32).rem_euclid(1.0);
                self.band((2.0 * phase - 1.0).abs())
            }
        };
        (level * self.spectrum_gain).clamp(0.0, 1.0).powf(1.5)
    }
    fn expansion(&self) -> f32 {
        1.0 + self.energy * self.settings.audio_expand
            + self.transient * self.settings.attack_accent
    }

    fn layer_fraction(&self, layer: usize) -> f32 {
        if self.settings.flow.abs() > 0.0001 {
            (layer as f32 / self.settings.layers as f32 + self.flow_phase).fract()
        } else {
            layer as f32 / self.settings.layers.saturating_sub(1).max(1) as f32
        }
    }
    fn band(&self, u: f32) -> f32 {
        let x = u.clamp(0.0, 1.0) * (BANDS - 1) as f32;
        let i = x as usize;
        egui::lerp(
            self.bands[i]..=self.bands[(i + 1).min(BANDS - 1)],
            x.fract(),
        )
    }
    #[cfg(test)]
    fn point(&self, layer: usize, u: f32, v: f32) -> [f32; 3] {
        let s = &self.settings;
        let t = self.layer_fraction(layer);
        let phi = PI * v * s.opening;
        let angle = u * TAU;
        let petal = (0.5 + 0.5 * (angle * s.petals as f32).cos())
            .max(0.0)
            .powf(s.sharpness);
        let profile = s.waist + (1.0 - s.waist) * petal;
        let ripple = (v * PI * s.ripple_count - self.ripple_phase + t * TAU * s.phase_spread).sin()
            * (s.ripple_amount + s.audio_ripple * self.band(v));
        let radius =
            (s.radius + t * s.spacing * s.layers.saturating_sub(1) as f32) * self.expansion();
        let spectral = 1.0 + s.spectrum_depth * self.spectrum_level(layer, u, v);
        let radial = radius * phi.sin() * profile * (1.0 + ripple) * spectral;
        let turn = self.spin_phase
            * if s.alternate && layer % 2 == 1 {
                -1.0
            } else {
                1.0
            }
            + t * s.layers.saturating_sub(1) as f32 * s.twist.to_radians();
        let theta = angle + turn;
        [
            radial * theta.cos(),
            radial * theta.sin(),
            radius * phi.cos() * s.flattening * spectral + (t - 0.5) * s.layer_depth,
        ]
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
        if !rect.is_positive() {
            return;
        }
        self.update(frame, now, live);
        let painter = ui.painter_at(rect);
        if self.settings.black_canvas
            && ui
                .data(|d| {
                    d.get_temp::<crate::background::Background>(egui::Id::new(
                        crate::background::CONTEXT,
                    ))
                })
                .is_none()
        {
            painter.rect_filled(rect, 0.0, egui::Color32::BLACK);
        }
        let scene = painter.add(egui::Shape::Noop);
        let plot = rect.shrink(24.0);
        let gizmo =
            Rect::from_min_size(rect.right_top() + Vec2::new(-88.0, 8.0), Vec2::splat(80.0));
        let s = &mut self.settings;
        if s.show_gizmo {
            let action = camera_gizmo::draw_with_roll(ui, gizmo, s.yaw, s.elevation, s.roll, true);
            if action.orbit != Vec2::ZERO
                && let Some(pointer) = ui.input(|i| i.pointer.interact_pos())
            {
                trackball::drag(
                    &mut s.yaw,
                    &mut s.elevation,
                    &mut s.roll,
                    pointer - action.orbit,
                    pointer,
                    gizmo.center(),
                    32.0,
                );
            }
            if let Some((axis, positive)) = action.snap {
                (s.yaw, s.elevation) = axis.snap(positive, s.yaw, s.elevation);
                s.roll = 0.0;
            }
        }
        let response=ui.interact(rect,ui.id().with("resonance-camera"),Sense::click_and_drag()).help_text("Drag to rotate freely; edge drag rolls. Right/Shift-drag pans, scroll zooms, double-click resets.");
        let over_gizmo = s.show_gizmo
            && ui.input(|i| {
                i.pointer
                    .press_origin()
                    .filter(|_| i.pointer.any_down())
                    .or_else(|| i.pointer.interact_pos())
                    .is_some_and(|p| gizmo.contains(p))
            });
        if !over_gizmo {
            if response.dragged_by(egui::PointerButton::Secondary)
                || response.dragged_by(egui::PointerButton::Middle)
                || (response.dragged() && ui.input(|i| i.modifiers.shift))
            {
                s.pan += response.drag_delta() / plot.size().max(Vec2::splat(1.0));
            } else if response.dragged_by(egui::PointerButton::Primary)
                && let Some(pointer) = response.interact_pointer_pos()
            {
                trackball::drag(
                    &mut s.yaw,
                    &mut s.elevation,
                    &mut s.roll,
                    pointer - response.drag_delta(),
                    pointer,
                    plot.center() + s.pan * plot.size(),
                    plot.width().min(plot.height()) * 0.32 * s.zoom,
                );
            }
            if response.hovered() {
                s.zoom = (s.zoom * (ui.input(|i| i.smooth_scroll_delta.y) * 0.002).exp())
                    .clamp(0.25, 8.0);
            }
            if response.double_clicked() {
                s.yaw = -0.45;
                s.elevation = 0.3;
                s.roll = 0.35;
                s.zoom = 1.0;
                s.pan = Vec2::ZERO;
            }
        }
        painter.set(scene, egui::Shape::mesh(self.build_mesh(plot, theme)));
    }
}

impl super::camera::Controlled for Resonance {
    fn camera(&mut self) -> super::camera::Camera<'_> {
        let s = &mut self.settings;
        super::camera::Camera {
            yaw: &mut s.yaw,
            elevation: &mut s.elevation,
            roll: &mut s.roll,
            zoom: &mut s.zoom,
            pan_x: &mut s.pan.x,
            pan_y: &mut s.pan.y,
            degrees: false,
            y_up: false,
            enabled: &mut s.auto_orbit,
            speed: &mut s.orbit_speed,
            speed_radians: false,
            default_speed: 10.0,
            speed_limit: 360.0,
            speed_key: "orbit_speed",
            reverse: None,
            motion: &mut s.camera_motion,
            gizmo: &mut s.show_gizmo,
        }
    }
}
