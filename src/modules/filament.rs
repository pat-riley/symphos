//! Depth-sorted thread sculptures. Geometry is procedural; audio deforms its
//! resting form without replacing it. No history or generated mesh is serialized.
use super::{
    Palette, camera, camera_gizmo,
    frequency::{BANDS, FrequencyData, FrequencySettings},
    trackball,
};
use crate::{analysis::AnalysisFrame, help::HoverHelp, parameter::Parameter, theme::AppTheme};
use egui::{Color32, Pos2, Rect, Vec2};
use std::{
    f32::consts::{PI, TAU},
    time::Instant,
};

mod render;
#[cfg(test)]
mod tests;
module_palette!(Filament, settings.palette);

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Form {
    Loops,
    Spindles,
    Folds,
    Vortex,
    GravityWell,
    Orbital,
}
impl Form {
    const ALL: [Self; 6] = [
        Self::Loops,
        Self::Spindles,
        Self::Folds,
        Self::Vortex,
        Self::GravityWell,
        Self::Orbital,
    ];
    fn spirals(self) -> bool {
        matches!(self, Self::Vortex | Self::GravityWell | Self::Orbital)
    }
    fn label(self) -> &'static str {
        match self {
            Self::Loops => "Sculpted loops",
            Self::Spindles => "Spindles",
            Self::Folds => "Folded ribbons",
            Self::Vortex => "Spiral vortex",
            Self::GravityWell => "Gravity well",
            Self::Orbital => "Orbital currents",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum AudioResponse {
    Sculptural,
    Spectrum,
    Blend,
}
impl AudioResponse {
    fn label(self) -> &'static str {
        match self {
            Self::Sculptural => "Sculptural bend",
            Self::Spectrum => "Spectrum reach",
            Self::Blend => "Bend + spectrum reach",
        }
    }
    fn reaches(self) -> bool {
        self != Self::Sculptural
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum SpectrumMapping {
    Strands,
    AlongStrands,
}
impl SpectrumMapping {
    fn label(self) -> &'static str {
        match self {
            Self::Strands => "By strand",
            Self::AlongStrands => "Along each strand",
        }
    }
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub(crate) struct Settings {
    form: Form,
    shells: usize,
    strands: usize,
    detail: usize,
    spread: f32,
    depth: f32,
    lobes: usize,
    fold: f32,
    twist: f32,
    taper: f32,
    drift: f32,
    circulation: f32,
    spiral_turns: f32,
    central_pull: f32,
    audio_response: AudioResponse,
    spectrum_mapping: SpectrumMapping,
    spectrum_reach: f32,
    spectrum_visibility: f32,
    attack_ms: f32,
    audio_bend: f32,
    audio_expand: f32,
    response_ms: f32,
    frequency: FrequencySettings,
    breaks: f32,
    dash_count: usize,
    line_width: f32,
    fine_strands: bool,
    dense_weave: bool,
    highlights: f32,
    opacity: f32,
    brightness: f32,
    color_shift: f32,
    color_spread: f32,
    palette: Palette,
    yaw: f32,
    elevation: f32,
    roll: f32,
    zoom: f32,
    pan: Vec2,
    auto_orbit: bool,
    orbit_speed: f32,
    camera_motion: super::camera_motion::Settings,
    show_gizmo: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            form: Form::Loops,
            shells: 3,
            strands: 44,
            detail: 128,
            spread: 0.55,
            depth: 0.7,
            lobes: 3,
            fold: 0.32,
            twist: 1.2,
            taper: 0.75,
            drift: 0.12,
            circulation: 0.45,
            spiral_turns: 2.5,
            central_pull: 0.6,
            audio_response: AudioResponse::Sculptural,
            spectrum_mapping: SpectrumMapping::Strands,
            spectrum_reach: 0.85,
            spectrum_visibility: 0.7,
            attack_ms: 25.0,
            audio_bend: 0.32,
            audio_expand: 0.18,
            response_ms: 140.0,
            frequency: FrequencySettings {
                floor: -72.0,
                frequency_smoothing: 2,
                ..Default::default()
            },
            breaks: 0.18,
            dash_count: 7,
            line_width: 1.25,
            fine_strands: false,
            dense_weave: false,
            highlights: 0.5,
            opacity: 0.9,
            brightness: 1.15,
            color_shift: 0.0,
            color_spread: 0.65,
            palette: Palette::Kiln,
            yaw: -0.45,
            elevation: 0.45,
            roll: -0.3,
            zoom: 1.0,
            pan: Vec2::ZERO,
            auto_orbit: false,
            orbit_speed: 8.0,
            camera_motion: Default::default(),
            show_gizmo: false,
        }
    }
}
impl Settings {
    fn centripetal_preset(&mut self) {
        // A geometry/motion starting point: preserve the user's palette, camera,
        // background and signal response rather than resetting the whole pane.
        self.form = Form::GravityWell;
        self.shells = 1;
        self.strands = 40;
        self.detail = 192;
        self.spread = 0.0;
        self.depth = 1.1;
        self.fold = 0.12;
        self.twist = 1.0;
        self.taper = 0.8;
        self.drift = 0.1;
        self.circulation = 0.65;
        self.spiral_turns = 3.0;
        self.central_pull = 0.8;
        self.breaks = 0.32;
        self.dash_count = 5;
        self.line_width = 1.25;
        self.fine_strands = true;
        self.dense_weave = true;
    }
    fn sanitize(&mut self) {
        self.shells = self.shells.clamp(1, 5);
        self.strands = self.strands.clamp(8, 96);
        self.detail = self.detail.clamp(64, 256);
        self.lobes = self.lobes.clamp(1, 8);
        self.dash_count = self.dash_count.clamp(1, 24);
        let d = Self::default();
        macro_rules! bound { ($($field:ident: $lo:expr, $hi:expr);* $(;)?) => {$(
            self.$field = if self.$field.is_finite() { self.$field.clamp($lo, $hi) } else { d.$field };
        )*}; }
        bound! {
            spectrum_reach: 0.0, 2.5; spectrum_visibility: 0.0, 1.0; attack_ms: 0.0, 500.0;
            spread: 0.0, 1.5; depth: 0.1, 2.0; fold: 0.0, 0.75; twist: -4.0, 4.0;
            circulation: 0.0, 2.0; spiral_turns: 0.5, 5.0; central_pull: 0.0, 1.0;
            taper: 0.2, 2.0; drift: 0.0, 1.0; audio_bend: 0.0, 1.0; audio_expand: 0.0, 0.6;
            response_ms: 0.0, 1500.0; breaks: 0.0, 0.85; line_width: 0.3, 4.0;
            highlights: 0.0, 1.0; opacity: 0.0, 1.0; brightness: 0.0, 2.0;
            color_shift: 0.0, 1.0; color_spread: 0.0, 1.0; yaw: -100.0, 100.0;
            elevation: -100.0, 100.0; roll: -100.0, 100.0; zoom: 0.25, 4.0;
            orbit_speed: 0.0, 90.0;
        }
        for v in [&mut self.pan.x, &mut self.pan.y] {
            *v = if v.is_finite() {
                v.clamp(-4.0, 4.0)
            } else {
                0.0
            };
        }
    }
}

pub(super) struct Filament {
    settings: Settings,
    data: FrequencyData,
    bands: [f32; BANDS],
    reach_bands: [f32; BANDS],
    spectrum_gain: f32,
    energy: f32,
    phase: f32,
    flow_time: f64,
    epoch: Option<u64>,
    last_draw: Option<Instant>,
}
impl Default for Filament {
    fn default() -> Self {
        Self {
            settings: Default::default(),
            data: Default::default(),
            bands: [0.0; BANDS],
            reach_bands: [0.0; BANDS],
            spectrum_gain: 1.0,
            energy: 0.0,
            phase: 0.0,
            flow_time: 0.0,
            epoch: None,
            last_draw: None,
        }
    }
}
impl Filament {
    pub fn settings_snapshot(&self) -> Settings {
        self.settings.clone()
    }
    pub fn apply_settings(&mut self, settings: &Settings) {
        self.settings = settings.clone();
        self.settings.sanitize();
    }
    pub fn reset_settings(&mut self) {
        *self = Self::default();
    }
    pub fn clear(&mut self) {
        self.clear_input_history();
        self.phase = 0.0;
        self.flow_time = 0.0;
    }
    pub fn clear_input_history(&mut self) {
        self.data.clear();
        self.bands.fill(0.0);
        self.reach_bands.fill(0.0);
        self.spectrum_gain = 1.0;
        self.energy = 0.0;
        self.epoch = None;
        self.last_draw = None;
    }
    fn update(&mut self, frame: &AnalysisFrame, now: Instant, live: bool) {
        self.settings.sanitize();
        if self.epoch != Some(frame.capture_epoch) {
            self.clear_input_history();
            self.epoch = Some(frame.capture_epoch);
        }
        let dt = self.last_draw.map_or(0.0, |last| {
            now.saturating_duration_since(last).as_secs_f32().min(0.1)
        });
        self.last_draw = Some(now);
        self.phase = (self.phase + dt * self.settings.drift).rem_euclid(TAU);
        self.flow_time += dt as f64 * self.settings.circulation as f64;
        self.data.settings = self.settings.frequency.clone();
        if live {
            self.data.update(frame, now);
        }
        let alpha = if self.settings.response_ms <= 0.0 {
            1.0
        } else {
            -(-dt * 1000.0 / self.settings.response_ms).exp_m1()
        };
        for (v, db) in self.bands.iter_mut().zip(self.data.levels) {
            let level = if live && db.is_finite() {
                self.data.settings.intensity(db).clamp(0.0, 1.0)
            } else {
                0.0
            };
            *v += (level * level - *v) * alpha;
        }
        // The existing sculptural response retains its original envelope. Reach
        // uses linear display intensity and an independent fast attack so quiet
        // band detail and transients are visible without changing the input feed.
        let attack = if self.settings.attack_ms <= 0.0 {
            1.0
        } else {
            -(-dt * 1000.0 / self.settings.attack_ms).exp_m1()
        };
        for (value, db) in self.reach_bands.iter_mut().zip(self.data.levels) {
            let target = if live && db.is_finite() {
                self.data.settings.intensity(db).clamp(0.0, 1.0)
            } else {
                0.0
            };
            *value += (target - *value) * if target > *value { attack } else { alpha };
        }
        let peak = self.reach_bands.iter().copied().fold(0.0_f32, f32::max);
        let gate = ((peak - 0.03) / 0.12).clamp(0.0, 1.0);
        self.spectrum_gain = 1.0
            + ((1.0 / peak.max(0.001)).clamp(1.0, 3.0) - 1.0)
                * self.settings.spectrum_visibility
                * gate;
        self.energy = self.bands.iter().sum::<f32>() / BANDS as f32;
    }
    fn spectrum_level(&self, t: f32, theta: f32) -> f32 {
        if self.settings.spectrum_mapping == SpectrumMapping::Strands {
            let count = self.settings.strands * if self.settings.dense_weave { 2 } else { 1 };
            let strand = ((t.clamp(0.0, 1.0) * count as f32) as usize).min(count - 1);
            let first = (strand * BANDS / count).min(BANDS - 1);
            let last = ((strand + 1) * BANDS)
                .div_ceil(count)
                .clamp(first + 1, BANDS);
            return (self.reach_bands[first..last]
                .iter()
                .copied()
                .fold(0.0_f32, f32::max)
                * self.spectrum_gain)
                .clamp(0.0, 1.0);
        }
        let position = match self.settings.spectrum_mapping {
            SpectrumMapping::Strands => t,
            SpectrumMapping::AlongStrands
                if matches!(self.settings.form, Form::Vortex | Form::GravityWell) =>
            {
                theta / TAU
            }
            // A mirrored mapping closes the bass/treble seam on closed contours.
            SpectrumMapping::AlongStrands => 0.5 - 0.5 * theta.cos(),
        };
        let i = position.clamp(0.0, 1.0) * (BANDS - 1) as f32;
        let a = i as usize;
        let level = self.reach_bands[a]
            + (self.reach_bands[(a + 1).min(BANDS - 1)] - self.reach_bands[a]) * i.fract();
        (level * self.spectrum_gain).clamp(0.0, 1.0)
    }
    fn band(&self, t: f32) -> f32 {
        let i = t.clamp(0.0, 1.0) * (BANDS - 1) as f32;
        let a = i as usize;
        self.bands[a] + (self.bands[(a + 1).min(BANDS - 1)] - self.bands[a]) * i.fract()
    }
    pub fn controls(&mut self, ui: &mut egui::Ui) {
        use super::{properties_combo, property_row, settings_group, settings_panel};
        let s = &mut self.settings;
        let d = Settings::default();
        macro_rules! p {
            ($ui:ident, $field:ident, $range:expr, $name:expr, $help:expr) => {
                $ui.add(
                    Parameter::new(&mut s.$field, $range, d.$field)
                        .key(stringify!($field))
                        .text($name),
                )
                .help_text($help);
            };
        }
        settings_panel(ui, "Geometry", |ui| {
            settings_group(ui, "Sculpture", |ui| {
                property_row(ui, "Form", |ui| {
                    properties_combo("filament-form", ui.available_width(), Form::ALL.len())
                        .selected_text(s.form.label())
                        .show_ui(ui, |ui| {
                            for form in Form::ALL {
                                ui.selectable_value(&mut s.form, form, form.label());
                            }
                        });
                });
                property_row(ui, "Starting preset", |ui| {
                    properties_combo("filament-preset", ui.available_width(), 1)
                        .selected_text("Choose a preset…")
                        .show_ui(ui, |ui| {
                            if ui.selectable_label(false, "Centripetal flow").clicked() {
                                s.centripetal_preset();
                            }
                        }).response.help_text("Fine, dense threads spiral inward through a gravity well. Keeps your colors, background, camera and audio response settings.");
                });
                if s.form.spirals() {
                    p!(
                        ui,
                        spiral_turns,
                        0.5..=5.0,
                        "Spiral turns",
                        "Number of revolutions along each inward trail, or lobes of an orbital current."
                    );
                    p!(
                        ui,
                        central_pull,
                        0.0..=1.0,
                        "Central pull",
                        "Concentrate the threads near the center and increase the inner circulation rate. An artistic gravity-like field, not a physical simulation."
                    );
                }
                p!(
                    ui,
                    shells,
                    1..=5,
                    "Forms",
                    "Number of overlapping thread sculptures."
                );
                p!(
                    ui,
                    strands,
                    8..=96,
                    "Strands",
                    "Thread contours per form. Higher density creates a woven surface."
                );
                p!(
                    ui,
                    spread,
                    0.0..=1.5,
                    "Separation",
                    "Separate the centers of the forms; zero nests them together."
                );
                p!(
                    ui,
                    depth,
                    0.1..=2.0,
                    "Depth",
                    "Thickness along each form's central axis."
                );
                p!(
                    ui,
                    lobes,
                    1..=8,
                    "Lobes",
                    "Number of large folds around each loop."
                );
                p!(
                    ui,
                    fold,
                    0.0..=0.75,
                    "Fold amount",
                    "Bend circular loops into soft lobed contours."
                );
                p!(
                    ui,
                    twist,
                    -4.0..=4.0,
                    "Twist",
                    "Turn successive contours around the sculpture's axis."
                );
                p!(
                    ui,
                    taper,
                    0.2..=2.0,
                    "Taper",
                    "Shape the profile from broad bowls to narrow spindles."
                );
            });
            settings_group(ui, "Motion & audio", |ui| {
                property_row(ui, "Audio response", |ui| {
                    properties_combo("filament-audio-response", ui.available_width(), 3)
                        .selected_text(s.audio_response.label())
                        .show_ui(ui, |ui| {
                            for mode in [AudioResponse::Sculptural, AudioResponse::Spectrum, AudioResponse::Blend] {
                                ui.selectable_value(&mut s.audio_response, mode, mode.label());
                            }
                        }).response.help_text("Sculptural bend keeps the original gentle response. Spectrum reach extends threads with their frequency bands. Blend combines both; all use the pane's selected audio feed.");
                });
                if s.audio_response.reaches() {
                    property_row(ui, "Map spectrum", |ui| {
                        properties_combo("filament-spectrum-mapping", ui.available_width(), 2)
                            .selected_text(s.spectrum_mapping.label())
                            .show_ui(ui, |ui| {
                                for mapping in [SpectrumMapping::Strands, SpectrumMapping::AlongStrands] {
                                    ui.selectable_value(&mut s.spectrum_mapping, mapping, mapping.label());
                                }
                            }).response.help_text("By strand assigns bass through treble from the first contour to the last, repeated per form. Along each strand puts the spectrum along the curve, mirrored around closed loops.");
                    });
                    p!(
                        ui,
                        spectrum_reach,
                        0.0..=2.5,
                        "Spectrum reach",
                        "Extend strands radially with their assigned frequency energy. Zero restores the resting size. This is an artistic spectrum mapping."
                    );
                    p!(
                        ui,
                        spectrum_visibility,
                        0.0..=1.0,
                        "Spectrum visibility",
                        "Reveal quieter frequency differences with a bounded display boost. Near-silence stays still; the source signal is unchanged."
                    );
                    p!(
                        ui,
                        attack_ms,
                        0.0..=500.0,
                        "Attack ms",
                        "How quickly frequency-driven reach follows rising audio energy. Lower values make hits more immediate."
                    );
                }
                if s.form.spirals() {
                    p!(
                        ui,
                        circulation,
                        0.0..=2.0,
                        "Circulation speed",
                        "Move the threads around the center and carry the broken segments inward. Zero stops circulation independently of camera orbit and fold motion."
                    );
                }
                p!(
                    ui,
                    drift,
                    0.0..=1.0,
                    "Flow speed",
                    "Slow motion of the folds. Set to zero for a still resting sculpture; audio can still deform it."
                );
                if s.audio_response != AudioResponse::Spectrum {
                    p!(
                        ui,
                        audio_bend,
                        0.0..=1.0,
                        "Spectrum bend",
                        "Bass-to-treble energy bends different strands and points along each loop. Uses the pane's selected audio feed."
                    );
                }
                p!(
                    ui,
                    audio_expand,
                    0.0..=0.6,
                    "Audio expansion",
                    "Overall spectrum energy expands the sculpture while preserving its shape."
                );
                p!(
                    ui,
                    response_ms,
                    0.0..=1500.0,
                    "Response ms",
                    "Smooth audio-driven bending. Low values reveal transients; high values give slower motion."
                );
            });
            settings_group(ui, "Detail", |ui| {
                p!(
                    ui,
                    detail,
                    64..=256,
                    "Curve resolution",
                    "Samples per contour, bounded by a shared geometry budget at high strand counts."
                );
            });
        });
        settings_panel(ui, "Frequency Range", |ui| s.frequency.range_controls(ui));
        settings_panel(ui, "Signal Response", |ui| {
            s.frequency.response_controls(ui);
            settings_group(ui, "Display range", |ui| s.frequency.level_controls(ui));
        });
        settings_panel(ui, "Appearance", |ui| {
            settings_group(ui, "Threads", |ui| {
                ui.checkbox(&mut s.fine_strands, "Fine strands")
                    .help_text("Draw threads and their highlights at 45% width for a finer ink-like finish. Preserves your thread width setting.");
                ui.checkbox(&mut s.dense_weave, "Dense weave")
                    .help_text("Double the number of contours without changing the sculpture's shape. Uses more rendering detail; combine with Fine strands for a tightly woven finish.");
                p!(
                    ui,
                    line_width,
                    0.3..=4.0,
                    "Thread width px",
                    "Thickness of each strand in screen pixels."
                );
                p!(
                    ui,
                    highlights,
                    0.0..=1.0,
                    "Thread highlights",
                    "Fine light and dark edges give strands a rounded, ink-like surface."
                );
                p!(
                    ui,
                    breaks,
                    0.0..=0.85,
                    "Broken strands",
                    "Cut gaps in the contours. Zero draws continuous loops; increasing this reveals more background."
                );
                p!(
                    ui,
                    dash_count,
                    1..=24,
                    "Break frequency",
                    "Number of gaps around each contour. Adjacent strands have offset gaps."
                );
                p!(
                    ui,
                    opacity,
                    0.0..=1.0,
                    "Opacity",
                    "Transparency of the woven sculpture."
                );
                p!(
                    ui,
                    brightness,
                    0.0..=2.0,
                    "Brightness",
                    "Overall thread brightness."
                );
            });
            settings_group(ui, "Color", |ui| {
                s.palette.controls(ui);
                p!(
                    ui,
                    color_spread,
                    0.0..=1.0,
                    "Color spread",
                    "Spread the gradient through the contours; lower values emphasize separate colors for each form."
                );
                p!(
                    ui,
                    color_shift,
                    0.0..=1.0,
                    "Color offset",
                    "Move the gradient along the sculpture."
                );
            });
        });
        settings_panel(ui, "Camera", |ui| {
            settings_group(ui, "Transform", |ui| {
                for (key, label, angle, default) in [
                    ("yaw", "Rotation °", &mut s.yaw, d.yaw),
                    ("elevation", "Elevation °", &mut s.elevation, d.elevation),
                    ("roll", "Roll °", &mut s.roll, d.roll),
                ] {
                    let mut value = angle.to_degrees();
                    if ui
                        .add(
                            Parameter::new(&mut value, -180.0..=180.0, default.to_degrees())
                                .key(key)
                                .raw_scale(std::f64::consts::PI / 180.0)
                                .text(label),
                        )
                        .changed()
                    {
                        *angle = value.to_radians();
                    }
                }
                p!(
                    ui,
                    zoom,
                    0.25..=4.0,
                    "Zoom",
                    "Scroll over the pane to zoom. Drag rotates; right/Shift-drag pans."
                );
                for (key, label, v) in [
                    ("pan.x", "Pan X", &mut s.pan.x),
                    ("pan.y", "Pan Y", &mut s.pan.y),
                ] {
                    ui.add(
                        Parameter::new(v, -2.0..=2.0, 0.0)
                            .bounds(-4.0..=4.0)
                            .key(key)
                            .text(label),
                    );
                }
                if ui.button("Reset camera").clicked() {
                    s.yaw = d.yaw;
                    s.elevation = d.elevation;
                    s.roll = d.roll;
                    s.zoom = d.zoom;
                    s.pan = d.pan;
                }
            });
        });
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
        let s = &mut self.settings;
        let gizmo =
            Rect::from_min_size(rect.right_top() + Vec2::new(-88.0, 8.0), Vec2::splat(80.0));
        if s.show_gizmo {
            let action = camera_gizmo::draw_with_roll(ui, gizmo, s.yaw, s.elevation, s.roll, true);
            if action.orbit != Vec2::ZERO
                && let Some(p) = ui.input(|i| i.pointer.interact_pos())
            {
                trackball::drag(
                    &mut s.yaw,
                    &mut s.elevation,
                    &mut s.roll,
                    p - action.orbit,
                    p,
                    gizmo.center(),
                    32.0,
                );
            }
            if let Some((axis, positive)) = action.snap {
                (s.yaw, s.elevation) = axis.snap(positive, s.yaw, s.elevation);
                s.roll = 0.0;
            }
        }
        let r = ui.interact(rect, ui.id().with("filament-camera"), egui::Sense::click_and_drag())
            .help_text("Filament · Drag to rotate, right/Shift-drag to pan, scroll to zoom. Double-click resets the view.");
        let over = s.show_gizmo
            && ui.input(|i| {
                i.pointer
                    .press_origin()
                    .filter(|_| i.pointer.any_down())
                    .or_else(|| i.pointer.interact_pos())
                    .is_some_and(|p| gizmo.contains(p))
            });
        if !over {
            if r.dragged_by(egui::PointerButton::Secondary)
                || r.dragged_by(egui::PointerButton::Middle)
                || (r.dragged() && ui.input(|i| i.modifiers.shift))
            {
                s.pan += r.drag_delta() / rect.size().max(Vec2::splat(1.0));
            } else if r.dragged_by(egui::PointerButton::Primary)
                && let Some(p) = r.interact_pointer_pos()
            {
                trackball::drag(
                    &mut s.yaw,
                    &mut s.elevation,
                    &mut s.roll,
                    p - r.drag_delta(),
                    p,
                    rect.center() + s.pan * rect.size(),
                    rect.width().min(rect.height()) * 0.32 * s.zoom,
                );
            }
            if r.hovered() {
                s.zoom = (s.zoom * (ui.input(|i| i.smooth_scroll_delta.y) * 0.002).exp())
                    .clamp(0.25, 4.0);
            }
            if r.double_clicked() {
                let d = Settings::default();
                s.yaw = d.yaw;
                s.elevation = d.elevation;
                s.roll = d.roll;
                s.zoom = d.zoom;
                s.pan = d.pan;
            }
        }
        ui.painter_at(rect)
            .add(egui::Shape::mesh(self.build_mesh(rect, theme)));
    }
}
impl camera::Controlled for Filament {
    fn camera(&mut self) -> camera::Camera<'_> {
        let s = &mut self.settings;
        camera::Camera {
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
            default_speed: 8.0,
            speed_limit: 90.0,
            speed_key: "orbit_speed",
            reverse: None,
            motion: &mut s.camera_motion,
            gizmo: &mut s.show_gizmo,
        }
    }
}
