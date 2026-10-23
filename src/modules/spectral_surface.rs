//! Shared spherical/cylindrical spectral surface renderer. Frequency wraps around
//! the surface, history flows along it, and audio level displaces the radius.
//! Both primitives use Waterfall's capture/FFT processing and a free trackball.
use super::{
    Palette,
    camera_gizmo::{self, Axis},
    frequency::{BANDS, History},
    settings_group, settings_panel,
    waterfall::{mesh_dot, mesh_segment},
};
use crate::{analysis::AnalysisFrame, help::HoverHelp, parameter::Parameter, theme::AppTheme};
use eframe::egui::{self, Color32, Pos2, Rect, Sense, Vec2};
use std::{
    f32::consts::{PI, TAU},
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Style {
    Wireframe,
    Surface,
    Rings,
    Meridians,
    Points,
    Spikes,
}
impl Style {
    const ALL: [Self; 6] = [
        Self::Wireframe,
        Self::Surface,
        Self::Rings,
        Self::Meridians,
        Self::Points,
        Self::Spikes,
    ];
    fn label(self) -> &'static str {
        match self {
            Self::Wireframe => "Wireframe",
            Self::Surface => "Surface",
            Self::Rings => "Rings",
            Self::Meridians => "Meridians",
            Self::Points => "Points",
            Self::Spikes => "Radial spikes",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum ColorBy {
    Level,
    Frequency,
    History,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Motion {
    #[default]
    WrappedSpectrum,
    PoleRipples,
}
fn default_ripple_speed() -> f32 {
    1.0
}
fn default_ripple_rate() -> f32 {
    3.0
}
fn default_ripple_damping() -> f32 {
    1.0
}
fn default_ripple_impulse() -> f32 {
    0.65
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Shape {
    Sphere,
    Cylinder,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub(crate) struct CylinderGeometry {
    height: f32,
    top_radius: f32,
    bottom_radius: f32,
    edge_falloff: f32,
    caps: bool,
}
impl Default for CylinderGeometry {
    fn default() -> Self {
        Self {
            height: 2.4,
            top_radius: 1.0,
            bottom_radius: 1.0,
            edge_falloff: 0.15,
            caps: true,
        }
    }
}

pub struct SpectralSurface {
    camera_motion: super::camera_motion::Settings,
    shape: Shape,
    cylinder: CylinderGeometry,
    history: History,
    radius: f32,
    displacement: f32,
    stretch_x: f32,
    stretch_y: f32,
    stretch_z: f32,
    seconds: f32,
    latitudes: usize,
    longitudes: usize,
    repeats: usize,
    twist: f32,
    offset: f32,
    pole_taper: f32,
    reverse_flow: bool,
    motion: Motion,
    ripple_speed: f32,
    ripple_rate: f32,
    ripple_damping: f32,
    ripple_impulse: f32,
    ripple_origin: Option<Instant>,
    ripple_trigger: Option<Instant>,
    style: Style,
    palette: Palette,
    color_by: ColorBy,
    brightness: f32,
    contrast: f32,
    opacity: f32,
    rear_opacity: f32,
    idle_brightness: f32,
    line_width: f32,
    ring_spacing: usize,
    meridian_spacing: usize,
    dot_size: f32,
    dot_spacing: usize,
    stem_width: f32,
    stem_spacing: usize,
    surface_mesh: bool,
    yaw: f32,
    elevation: f32,
    roll: f32,
    zoom: f32,
    pan: Vec2,
    auto_orbit: bool,
    orbit_speed: f32,
    orbit_reverse: bool,
    guides: bool,
    show_gizmo: bool,
    last_draw: Option<Instant>,
    mesh_cache: Option<(MeshKey, std::sync::Arc<egui::Mesh>)>,
}
#[derive(PartialEq)]
struct MeshKey {
    time: Instant,
    settings: SurfaceSettings,
    rect: Rect,
    theme: AppTheme,
    sequence: Option<u64>,
}
module_settings!(SpectralSurface, SurfaceSettings, {
    #[serde(default)] camera_motion: super::camera_motion::Settings => camera_motion,
    #[serde(default)] cylinder: CylinderGeometry => cylinder,
    frequency: super::frequency::FrequencySettings => history.data.settings,
    radius: f32 => radius, displacement: f32 => displacement,
    stretch_x: f32 => stretch_x, stretch_y: f32 => stretch_y, stretch_z: f32 => stretch_z,
    seconds: f32 => seconds, latitudes: usize => latitudes, longitudes: usize => longitudes,
    repeats: usize => repeats, twist: f32 => twist, offset: f32 => offset,
    pole_taper: f32 => pole_taper, reverse_flow: bool => reverse_flow,
    #[serde(default)] motion: Motion => motion,
    #[serde(default="default_ripple_speed")] ripple_speed: f32 => ripple_speed,
    #[serde(default="default_ripple_rate")] ripple_rate: f32 => ripple_rate,
    #[serde(default="default_ripple_damping")] ripple_damping: f32 => ripple_damping,
    #[serde(default="default_ripple_impulse")] ripple_impulse: f32 => ripple_impulse,
    style: Style => style, palette: Palette => palette, color_by: ColorBy => color_by,
    brightness: f32 => brightness, contrast: f32 => contrast, opacity: f32 => opacity,
    rear_opacity: f32 => rear_opacity, idle_brightness: f32 => idle_brightness,
    line_width: f32 => line_width, ring_spacing: usize => ring_spacing,
    meridian_spacing: usize => meridian_spacing, dot_size: f32 => dot_size,
    dot_spacing: usize => dot_spacing, stem_width: f32 => stem_width,
    stem_spacing: usize => stem_spacing, surface_mesh: bool => surface_mesh,
    yaw: f32 => yaw, elevation: f32 => elevation, roll: f32 => roll,
    zoom: f32 => zoom, pan: Vec2 => pan, auto_orbit: bool => auto_orbit,
    orbit_speed: f32 => orbit_speed, orbit_reverse: bool => orbit_reverse,
    guides: bool => guides, show_gizmo: bool => show_gizmo,
});
impl Default for SpectralSurface {
    fn default() -> Self {
        Self {
            camera_motion: Default::default(),
            shape: Shape::Sphere,
            cylinder: CylinderGeometry::default(),
            history: History::default(),
            radius: 1.0,
            displacement: 0.65,
            stretch_x: 1.0,
            stretch_y: 1.0,
            stretch_z: 1.0,
            seconds: 3.0,
            latitudes: 64,
            longitudes: 128,
            repeats: 1,
            twist: 35.0,
            offset: 0.0,
            pole_taper: 1.0,
            reverse_flow: false,
            motion: Motion::WrappedSpectrum,
            ripple_speed: default_ripple_speed(),
            ripple_rate: default_ripple_rate(),
            ripple_damping: default_ripple_damping(),
            ripple_impulse: default_ripple_impulse(),
            ripple_origin: None,
            ripple_trigger: None,
            style: Style::Wireframe,
            palette: Palette::Spectrum,
            color_by: ColorBy::Frequency,
            brightness: 1.6,
            contrast: 1.0,
            opacity: 0.95,
            rear_opacity: 0.18,
            idle_brightness: 0.18,
            line_width: 0.85,
            ring_spacing: 1,
            meridian_spacing: 2,
            dot_size: 2.0,
            dot_spacing: 2,
            stem_width: 1.0,
            stem_spacing: 4,
            surface_mesh: true,
            yaw: -0.35,
            elevation: 0.35,
            roll: 0.0,
            zoom: 1.0,
            pan: Vec2::ZERO,
            auto_orbit: false,
            orbit_speed: 8.0,
            orbit_reverse: false,
            guides: true,
            show_gizmo: false,
            last_draw: None,
            mesh_cache: None,
        }
    }
}
impl SurfaceSettings {}

impl SpectralSurface {
    fn cached_mesh(
        &mut self,
        rect: Rect,
        now: Instant,
        theme: &AppTheme,
    ) -> std::sync::Arc<egui::Mesh> {
        let settings = self.settings_snapshot();
        let sequence = self.history.rows.back().map(|row| row.sequence);
        if let Some((key, mesh)) = &self.mesh_cache
            && key.time == now
            && key.settings == settings
            && key.rect == rect
            && key.theme == *theme
            && key.sequence == sequence
        {
            return mesh.clone();
        }
        let mesh = std::sync::Arc::new(self.build_mesh(rect, now, theme));
        self.mesh_cache = Some((
            MeshKey {
                time: now,
                settings,
                rect,
                theme: theme.clone(),
                sequence,
            },
            mesh.clone(),
        ));
        mesh
    }
    pub fn cylinder() -> Self {
        Self {
            shape: Shape::Cylinder,
            ..Self::default()
        }
    }
    fn is_cylinder(&self) -> bool {
        self.shape == Shape::Cylinder
    }

    pub fn ingest(&mut self, frame: &crate::capture_history::SpectralFrame, now: Instant) {
        self.ripple_origin.get_or_insert(now);
        self.history.ingest(frame, now);
    }
    pub fn clear(&mut self) {
        self.mesh_cache = None;
        self.history.clear();
        self.last_draw = None;
        self.ripple_origin = None;
        self.ripple_trigger = None;
    }
    pub fn reset_settings(&mut self) {
        let mut history = std::mem::take(&mut self.history);
        history.data.settings = Default::default();
        *self = Self {
            history,
            shape: self.shape,
            ..Self::default()
        };
    }
    fn reset_camera(&mut self) {
        self.yaw = -0.35;
        self.elevation = 0.35;
        self.roll = 0.0;
        self.zoom = 1.0;
        self.pan = Vec2::ZERO;
        self.auto_orbit = false;
    }

    #[cfg(test)]
    fn view_vector(&self, point: [f32; 3]) -> [f32; 3] {
        let (pos, depth) = camera_gizmo::project_direction(self.yaw, self.elevation, point);
        let (sin, cos) = self.roll.sin_cos();
        [pos.x * cos - pos.y * sin, pos.x * sin + pos.y * cos, depth]
    }
    #[cfg(test)]
    fn orientation_axes(&self) -> [[f64; 3]; 3] {
        super::trackball::orientation_axes(self.yaw, self.elevation, self.roll)
    }
    fn trackball_bounds(&self, rect: Rect) -> (Pos2, f32) {
        let plot = rect.shrink(24.0);
        let center = plot.center() + self.pan * plot.size();
        let extent = (self.radius.clamp(0.05, 4.0) + self.displacement.clamp(0.0, 4.0) * 0.5)
            * self
                .stretch_x
                .max(self.stretch_y)
                .max(self.stretch_z)
                .clamp(0.1, 4.0);
        let extent = if self.is_cylinder() {
            extent
                .max(self.cylinder.height.clamp(0.2, 8.0) * 0.5 * self.stretch_y.clamp(0.1, 4.0))
                .max(
                    self.radius.clamp(0.05, 4.0)
                        * self
                            .cylinder
                            .top_radius
                            .max(self.cylinder.bottom_radius)
                            .clamp(0.1, 3.0)
                        * self.stretch_x.max(self.stretch_z).clamp(0.1, 4.0),
                )
        } else {
            extent
        };
        let radius = (plot.width().min(plot.height()) * 0.27 * self.zoom.clamp(0.25, 8.0) * extent)
            .max(40.0);
        (center, radius)
    }
    fn trackball_drag(&mut self, from: Pos2, to: Pos2, center: Pos2, radius: f32) {
        super::trackball::drag(
            &mut self.yaw,
            &mut self.elevation,
            &mut self.roll,
            from,
            to,
            center,
            radius,
        );
        if from != to {
            self.auto_orbit = false;
        }
    }
    fn advance_camera(&mut self, now: Instant) {
        let dt = self.last_draw.map_or(0.0, |last| {
            now.saturating_duration_since(last).as_secs_f32().min(0.1)
        });
        self.last_draw = Some(now);
        if self.auto_orbit {
            self.yaw = wrap(
                self.yaw
                    + dt * self.orbit_speed.clamp(0.0, 360.0).to_radians()
                        * if self.orbit_reverse { -1.0 } else { 1.0 },
            );
        }
    }
    pub fn controls(&mut self, ui: &mut egui::Ui) {
        let cylinder = self.is_cylinder();
        let ripple_label = if cylinder {
            "Axial ripples"
        } else {
            "Pole ripples"
        };
        macro_rules! p {
            ($ui:ident, $field:ident, $range:expr, $default:expr, $label:expr, $help:expr) => {
                $ui.add(
                    Parameter::new(&mut self.$field, $range, $default)
                        .key(stringify!($field))
                        .text($label),
                )
                .help_text($help);
            };
        }
        settings_panel(ui, "Geometry", |ui| {
            settings_group(ui, "Surface motion", |ui| {
                crate::dropdown::ComboBox::from_id_salt("sphere-motion")
                    .selected_text(match self.motion {
                        Motion::WrappedSpectrum => "Wrapped spectrum",
                        Motion::PoleRipples => ripple_label,
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.motion,
                            Motion::WrappedSpectrum,
                            "Wrapped spectrum",
                        );
                        ui.selectable_value(&mut self.motion, Motion::PoleRipples, ripple_label);
                    });
                if self.motion == Motion::PoleRipples {
                    ui.small(if cylinder {
                        "Audio launches rings along the cylinder."
                    } else {
                        "Audio launches circular waves from pole to pole."
                    });
                    p!(
                        ui,
                        ripple_speed,
                        0.25..=4.0,
                        1.0,
                        "Travel speed",
                        "Speed multiplier for end-to-end travel. History s sets the travel time at 1×."
                    );
                    p!(
                        ui,
                        ripple_rate,
                        0.5..=8.0,
                        3.0,
                        "Ripple rate Hz",
                        "Number of emitted wave crests per second. Sound controls their strength; silence produces no audio ripples."
                    );
                    p!(
                        ui,
                        ripple_damping,
                        0.0..=4.0,
                        1.0,
                        "Damping",
                        "How quickly ripples weaken as they travel toward the opposite end."
                    );
                    p!(
                        ui,
                        ripple_impulse,
                        0.0..=1.0,
                        0.65,
                        "Hit emphasis",
                        "Blend sustained sound energy with sharp changes in energy. Raise this for distinct rings on drums and impacts."
                    );
                    ui.small(format!("Travel time: {:.2} s", self.ripple_travel_time()));
                    if ui.button("Launch ripple").help_text("Send one manual ripple from the source end. Audio-driven waves continue alongside it.").clicked() {
                        self.ripple_trigger = Some(self.last_draw.unwrap_or_else(Instant::now));
                    }
                }
            });
            settings_group(ui, "Shape & displacement", |ui| {
                p!(
                    ui,
                    radius,
                    0.2..=2.0,
                    1.0,
                    "Radius",
                    "The resting radius. Independent of audio level and camera zoom."
                );
                p!(
                    ui,
                    displacement,
                    0.0..=2.0,
                    0.65,
                    "Displacement",
                    "Depth of the audio-driven surface and traveling ripples. Zero produces a smooth surface."
                );
                p!(
                    ui,
                    stretch_x,
                    0.2..=3.0,
                    1.0,
                    "Width X",
                    "Stretch the surface along its X axis."
                );
                p!(
                    ui,
                    stretch_y,
                    0.2..=3.0,
                    1.0,
                    "Vertical stretch",
                    "Stretch the surface vertically."
                );
                p!(
                    ui,
                    stretch_z,
                    0.2..=3.0,
                    1.0,
                    "Depth Y",
                    "Stretch the surface front to back."
                );
                if cylinder {
                    for (key, label, value, range, default, help) in [
                        (
                            "cylinder.height",
                            "Height",
                            &mut self.cylinder.height,
                            0.2..=6.0,
                            2.4,
                            "Length of the cylinder along its axis.",
                        ),
                        (
                            "cylinder.top_radius",
                            "Top radius ×",
                            &mut self.cylinder.top_radius,
                            0.0..=3.0,
                            1.0,
                            "Scale the top radius. Zero tapers the top to a point, including while audio displaces the surface.",
                        ),
                        (
                            "cylinder.bottom_radius",
                            "Bottom radius ×",
                            &mut self.cylinder.bottom_radius,
                            0.0..=3.0,
                            1.0,
                            "Scale the bottom radius independently. Zero tapers the bottom to a point.",
                        ),
                        (
                            "cylinder.edge_falloff",
                            "Edge falloff",
                            &mut self.cylinder.edge_falloff,
                            0.0..=1.0,
                            0.15,
                            "Fade displacement near both rims. Zero lets audio move the complete surface.",
                        ),
                    ] {
                        ui.add(Parameter::new(value, range, default).key(key).text(label))
                            .help_text(help);
                    }
                    ui.checkbox(&mut self.cylinder.caps, "Show end caps").help_text(
                        "Turn off to remove both end faces and their radial grid lines, leaving an open tube. Pointed ends have no cap geometry.",
                    );
                } else {
                    p!(
                        ui,
                        pole_taper,
                        0.1..=4.0,
                        1.0,
                        "Pole taper",
                        "Smoothly reduce displacement at both poles to keep the surface closed. Higher values concentrate peaks around the equator."
                    );
                }
            });
            if self.motion == Motion::WrappedSpectrum {
                settings_group(ui, "Frequency wrap", |ui| {
                    p!(
                        ui,
                        repeats,
                        1..=4,
                        1,
                        "Spectrum repeats",
                        "Repeat a mirrored low-to-high-to-low spectrum around the circumference. Mirroring avoids a seam between bass and treble."
                    );
                    p!(
                        ui,
                        twist,
                        -360.0..=360.0,
                        35.0,
                        "Twist °",
                        "Twist the frequency grid progressively along the surface."
                    );
                    p!(
                        ui,
                        offset,
                        -180.0..=180.0,
                        0.0,
                        "Wrap offset °",
                        "Rotate the frequency map around the surface without moving the camera."
                    );
                });
            }
            settings_group(ui, "History & resolution", |ui| {
                ui.add(Parameter::new(&mut self.seconds, 0.1..=30.0, 3.0).key("seconds").logarithmic(true).text("History s"))
                    .help_text("End-to-end history time. For ripples, divide this duration by Travel speed to get the wave travel time.");
                p!(
                    ui,
                    latitudes,
                    12..=128,
                    64,
                    "History rings",
                    "Number of time slices along the surface. Higher resolution uses more rendering work."
                );
                p!(
                    ui,
                    longitudes,
                    24..=256,
                    128,
                    if cylinder {
                        "Around cylinder"
                    } else {
                        "Around sphere"
                    },
                    "Number of samples around each ring. Display interpolation does not increase FFT resolution."
                );
                ui.checkbox(&mut self.reverse_flow, "Reverse history flow")
                    .help_text("Start history and ripples at the bottom instead of the top.");
            });
        });
        settings_panel(ui, "Frequency Range", |ui| {
            self.history.data.settings.range_controls(ui)
        });
        settings_panel(ui, "Signal Response", |ui| {
            self.history.data.settings.response_controls(ui)
        });
        settings_panel(ui, "Appearance", |ui| {
            self.history.data.settings.label_controls(ui);
            settings_group(ui, "Render style", |ui| {
                super::property_row(ui, "Style", |ui| {
                    crate::dropdown::ComboBox::from_id_salt("sphere-style")
                        .selected_text(self.style.label())
                        .width(ui.available_width())
                        .show_ui(ui, |ui| {
                            for style in Style::ALL {
                                ui.selectable_value(&mut self.style, style, style.label());
                            }
                        });
                });
                match self.style {
                    Style::Points => {
                        p!(
                            ui,
                            dot_size,
                            0.5..=8.0,
                            2.0,
                            "Point size px",
                            "Screen-space particle diameter."
                        );
                        p!(
                            ui,
                            dot_spacing,
                            1..=8,
                            2,
                            "Point spacing",
                            "Display every Nth ring and meridian point."
                        );
                    }
                    Style::Spikes => {
                        p!(
                            ui,
                            stem_width,
                            0.5..=4.0,
                            1.0,
                            "Spike width px",
                            "Thickness of radial spikes from the resting shape to the audio surface."
                        );
                        p!(
                            ui,
                            stem_spacing,
                            1..=8,
                            4,
                            "Spike spacing",
                            "Display every Nth radial spike."
                        );
                    }
                    _ => {
                        if self.style == Style::Surface {
                            ui.checkbox(&mut self.surface_mesh, "Surface grid");
                        }
                        p!(
                            ui,
                            line_width,
                            0.3..=3.0,
                            0.85,
                            "Line width px",
                            "Thickness of the frequency and history grid."
                        );
                        p!(
                            ui,
                            ring_spacing,
                            1..=8,
                            1,
                            "Ring spacing",
                            "Display every Nth history ring. Does not change captured history."
                        );
                        p!(
                            ui,
                            meridian_spacing,
                            1..=8,
                            2,
                            "Meridian spacing",
                            "Display every Nth column around the surface."
                        );
                    }
                }
            });
            settings_group(ui, "Color & light", |ui| {
                self.palette.controls(ui);
                crate::dropdown::ComboBox::from_id_salt("sphere-color")
                    .selected_text(match self.color_by {
                        ColorBy::Level => "Color by level",
                        ColorBy::Frequency => "Color by frequency",
                        ColorBy::History => "Color by history",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.color_by, ColorBy::Level, "Level");
                        ui.selectable_value(&mut self.color_by, ColorBy::Frequency, "Frequency");
                        ui.selectable_value(&mut self.color_by, ColorBy::History, "History");
                    });
                p!(
                    ui,
                    brightness,
                    0.0..=2.0,
                    1.6,
                    "Brightness",
                    "Overall light level. Audio also brightens each active frequency."
                );
                p!(
                    ui,
                    contrast,
                    0.25..=4.0,
                    1.0,
                    "Color contrast",
                    "Shape the gradient response."
                );
                p!(
                    ui,
                    opacity,
                    0.0..=1.0,
                    0.95,
                    "Opacity",
                    "Transparency of the whole surface."
                );
                p!(
                    ui,
                    rear_opacity,
                    0.0..=1.0,
                    0.18,
                    "Rear visibility",
                    "Dim the back surface for readable depth, or raise it for a transparent wireframe."
                );
                p!(
                    ui,
                    idle_brightness,
                    0.0..=1.0,
                    0.18,
                    "Resting grid",
                    "Visibility of the unexcited surface. Set to zero to show only audio activity."
                );
                self.history.data.settings.level_controls(ui);
            });
            settings_group(ui, "Viewport guides", |ui| {
                ui.checkbox(&mut self.show_gizmo, "Navigation gizmo");
            });
        });
        settings_panel(ui, "Camera", |ui| {
            settings_group(ui, "View presets", |ui| {
                ui.horizontal_wrapped(|ui| {
                    for (name, axis, positive) in [
                        ("Front", Axis::Y, false),
                        ("Right", Axis::X, true),
                        ("Top", Axis::Z, true),
                        ("Back", Axis::Y, true),
                        ("Left", Axis::X, false),
                        ("Bottom", Axis::Z, false),
                    ] {
                        if ui.button(name).clicked() {
                            (self.yaw, self.elevation) = axis.angles(positive);
                            self.roll = 0.0;
                            self.auto_orbit = false;
                        }
                    }
                });
            });
            settings_group(ui, "Transform", |ui| {
                for (key, name, angle, default) in [
                    ("yaw", "Rotation °", &mut self.yaw, -0.35_f32),
                    ("elevation", "Elevation °", &mut self.elevation, 0.35),
                    ("roll", "Roll °", &mut self.roll, 0.0),
                ] {
                    let mut degrees = angle.to_degrees();
                    if ui
                        .add(
                            Parameter::new(&mut degrees, -180.0..=180.0, default.to_degrees())
                                .key(key)
                                .raw_scale(std::f64::consts::PI / 180.0)
                                .text(name),
                        )
                        .changed()
                    {
                        *angle = wrap(degrees.to_radians());
                        self.auto_orbit = false;
                    }
                }
                p!(
                    ui,
                    zoom,
                    0.25..=8.0,
                    1.0,
                    "Zoom",
                    "Magnify the view; scroll over the surface to adjust."
                );
                ui.horizontal(|ui| {
                    if ui.button("Center view").clicked() {
                        self.pan = Vec2::ZERO;
                    }
                    if ui.button("Reset camera").clicked() {
                        self.reset_camera();
                    }
                });
            });
            settings_group(ui, "Auto-orbit", |ui| {
                ui.checkbox(&mut self.auto_orbit, "Enable auto-orbit");
                p!(
                    ui,
                    orbit_speed,
                    0.0..=60.0,
                    8.0,
                    "Orbit °/s",
                    "Camera rotation speed, independent of audio history flow. Pausing the panel pauses the orbit."
                );
                ui.checkbox(&mut self.orbit_reverse, "Reverse orbit");
            });
        });
    }

    // Interpolate capture rows in time, but never bridge dropouts. Sampling a
    // retained row twice must not invent new audio or advance the capture clock.
    fn levels_at(&self, now: Instant, age: f32) -> Option<[f32; BANDS]> {
        let target = now.checked_sub(Duration::from_secs_f32(age.max(0.0)))?;
        let index = self.history.rows.partition_point(|row| row.time <= target);
        let a = self.history.rows.get(index.saturating_sub(1))?;
        let distance = if a.time > target {
            a.time.duration_since(target)
        } else {
            target.duration_since(a.time)
        };
        if distance.as_secs_f32() >= 0.12 {
            return None;
        }
        if let Some(b) = self
            .history
            .rows
            .get(index)
            .filter(|b| b.time > a.time && b.time > target)
        {
            let span = b.time.duration_since(a.time).as_secs_f32();
            if span <= 0.12 && target >= a.time {
                let t = target.duration_since(a.time).as_secs_f32() / span;
                return Some(std::array::from_fn(|i| {
                    a.levels[i] + (b.levels[i] - a.levels[i]) * t
                }));
            }
        }
        Some(a.levels)
    }
    fn frequency_fraction(&self, longitude: f32) -> f32 {
        // A triangular wrap closes the seam and preserves the full selected
        // frequency range on both hemispheres instead of jumping high -> low.
        let phase = (longitude * self.repeats.clamp(1, 4) as f32).rem_euclid(1.0);
        1.0 - (phase * 2.0 - 1.0).abs()
    }
    #[cfg(test)]
    fn point(&self, longitude: f32, latitude: f32, level: f32) -> [f32; 3] {
        let phi = latitude * PI;
        let theta =
            longitude * TAU + self.offset.to_radians() + self.twist.to_radians() * (latitude - 0.5);
        let taper = phi.sin().max(0.0).powf(self.pole_taper.clamp(0.1, 4.0));
        let r = self.radius.clamp(0.05, 4.0)
            + level.clamp(
                if self.motion == Motion::PoleRipples {
                    -1.0
                } else {
                    0.0
                },
                1.0,
            ) * self.displacement.clamp(0.0, 4.0)
                * taper;
        let r = r.max(0.05);
        [
            r * phi.sin() * theta.cos() * self.stretch_x.clamp(0.1, 4.0),
            r * phi.cos() * self.stretch_y.clamp(0.1, 4.0),
            r * phi.sin() * theta.sin() * self.stretch_z.clamp(0.1, 4.0),
        ]
    }
    fn ripple_travel_time(&self) -> f32 {
        (self.seconds.clamp(0.1, 30.0) / self.ripple_speed.clamp(0.25, 4.0)).clamp(0.1, 30.0)
    }
    fn ripple_energy(&self, levels: Option<&[f32; BANDS]>) -> (f32, f32) {
        let Some(levels) = levels else {
            return (0.0, 0.0);
        };
        // A complete latitude shares one pulse; frequencies shape the pulse's
        // strength/color rather than assigning different longitude displacements.
        let weights: [f32; BANDS] =
            std::array::from_fn(|i| self.history.data.settings.intensity(levels[i]).powi(2));
        let sum = weights.iter().sum::<f32>();
        let energy = (sum / BANDS as f32).sqrt();
        let tone = if sum > 1e-8 {
            weights
                .iter()
                .enumerate()
                .map(|(i, w)| *w * i as f32 / (BANDS - 1) as f32)
                .sum::<f32>()
                / sum
        } else {
            0.0
        };
        (energy, tone)
    }
    fn ripple_at(&self, now: Instant, latitude: f32) -> (f32, f32) {
        let age = latitude * self.ripple_travel_time();
        let levels = self.levels_at(now, age);
        let (energy, tone) = self.ripple_energy(levels.as_ref());
        let (prior, _) = self.ripple_energy(self.levels_at(now, age + 0.05).as_ref());
        let hit = ((energy - prior).max(0.0) * 8.0).min(1.0);
        let impulse = self.ripple_impulse.clamp(0.0, 1.0);
        let strength = energy * (1.0 - impulse) + hit * impulse;
        let rate = self.ripple_rate.clamp(0.5, 8.0);
        let origin = self
            .ripple_origin
            .or_else(|| self.history.rows.front().map(|r| r.time))
            .unwrap_or(now);
        let emitted = now.saturating_duration_since(origin).as_secs_f32() - age;
        let audio_wave = strength * (TAU * rate * emitted).cos();
        let manual_wave = self
            .ripple_trigger
            .and_then(|trigger| now.checked_duration_since(trigger))
            .map_or(0.0, |elapsed| {
                let arrival = elapsed.as_secs_f32() - age;
                let width = (0.75 / rate)
                    .clamp(0.03, 0.6)
                    .min(self.ripple_travel_time() * 0.15);
                (-0.5 * (arrival / width).powi(2)).exp() * (TAU * rate * arrival).cos()
            });
        let damping = (-latitude * self.ripple_damping.clamp(0.0, 4.0)).exp();
        ((audio_wave + manual_wave).clamp(-1.0, 1.0) * damping, tone)
    }
    // Frozen pre-optimization renderer for numerical and timing comparisons.
    #[cfg(test)]
    fn build_mesh_reference(&self, rect: Rect, now: Instant, theme: &AppTheme) -> egui::Mesh {
        let rows = self.latitudes.clamp(12, 128);
        let cols = self.longitudes.clamp(24, 256);
        // Fixed framing prevents audio peaks from changing the camera scale.
        let scale = rect.width().min(rect.height()).max(0.0) * 0.27 * self.zoom.clamp(0.25, 8.0);
        let center = rect.center() + self.pan * rect.size();
        let project = |p| {
            let [x, y, depth] = self.view_vector(p);
            (center + Vec2::new(x, y) * scale, depth)
        };
        let mut vertices = Vec::with_capacity((rows + 1) * (cols + 1));
        for row in 0..=rows {
            let v = row as f32 / rows as f32;
            let age = if self.reverse_flow { 1.0 - v } else { v };
            let levels = self.levels_at(now, age * self.seconds.clamp(0.1, 30.0));
            let ripple = (self.motion == Motion::PoleRipples).then(|| self.ripple_at(now, age));
            for col in 0..=cols {
                // Use exactly the first longitude for the seam, avoiding float drift.
                let u = if col == cols {
                    0.0
                } else {
                    col as f32 / cols as f32
                };
                let f = self.frequency_fraction(u);
                let band = f * (BANDS - 1) as f32;
                let lo = (band as usize).min(BANDS - 1);
                let hi = (lo + 1).min(BANDS - 1);
                let level = ripple.map_or_else(
                    || {
                        levels.map_or(0.0, |a| {
                            self.history
                                .data
                                .settings
                                .intensity(a[lo] + (a[hi] - a[lo]) * band.fract())
                        })
                    },
                    |(level, _)| level,
                );
                let p = self.point(u, v, level);
                let (pos, depth) = project(p);
                let (_, facing) = camera_gizmo::project_direction(
                    self.yaw,
                    self.elevation,
                    self.point(u, v, 0.0),
                );
                let color_t = match self.color_by {
                    ColorBy::Level => level.abs(),
                    ColorBy::Frequency => ripple.map_or(f, |(_, tone)| tone),
                    ColorBy::History => age,
                };
                let activity = if self.motion == Motion::PoleRipples {
                    level.abs().sqrt()
                } else {
                    level.abs()
                };
                let light = (self.idle_brightness + activity * (1.0 - self.idle_brightness))
                    .clamp(0.0, 1.0)
                    * self.brightness.clamp(0.0, 2.0);
                let color = self
                    .palette
                    .color_with_contrast(color_t, self.contrast, theme);
                let alpha = self.opacity.clamp(0.0, 1.0)
                    * if facing < 0.0 {
                        self.rear_opacity.clamp(0.0, 1.0)
                    } else {
                        1.0
                    };
                let c = |v: u8| (v as f32 * light).clamp(0.0, 255.0) as u8;
                vertices.push(Vertex {
                    pos,
                    depth,
                    base: project(self.point(u, v, 0.0)).0,
                    color: Color32::from_rgba_unmultiplied(
                        c(color.r()),
                        c(color.g()),
                        c(color.b()),
                        (alpha * 255.0) as u8,
                    ),
                });
            }
        }
        let stride = cols + 1;
        let mut cells: Vec<_> = (0..rows)
            .flat_map(|r| (0..cols).map(move |c| (r, c)))
            .collect();
        cells.sort_unstable_by(|&(ar, ac), &(br, bc)| {
            let depth = |r: usize, c: usize| {
                [
                    r * stride + c,
                    r * stride + c + 1,
                    (r + 1) * stride + c,
                    (r + 1) * stride + c + 1,
                ]
                .iter()
                .map(|&i| vertices[i].depth)
                .sum::<f32>()
            };
            depth(ar, ac).total_cmp(&depth(br, bc))
        });
        let mut mesh = egui::Mesh::default();
        for (row, col) in cells {
            let a = vertices[row * stride + col];
            let b = vertices[row * stride + col + 1];
            let c = vertices[(row + 1) * stride + col];
            let d = vertices[(row + 1) * stride + col + 1];
            let line = |mesh: &mut egui::Mesh, a: Vertex, b: Vertex, width: f32| {
                mesh_segment(mesh, a.pos, b.pos, a.color, b.color, width)
            };
            match self.style {
                Style::Points
                    if row.is_multiple_of(self.dot_spacing.clamp(1, 8))
                        && col.is_multiple_of(self.dot_spacing.clamp(1, 8)) =>
                {
                    mesh_dot(
                        &mut mesh,
                        a.pos,
                        a.color,
                        self.dot_size.clamp(0.5, 8.0) * 0.5,
                    )
                }
                Style::Spikes
                    if row.is_multiple_of(self.stem_spacing.clamp(1, 8))
                        && col.is_multiple_of(self.stem_spacing.clamp(1, 8)) =>
                {
                    mesh_segment(
                        &mut mesh,
                        a.base,
                        a.pos,
                        a.color.gamma_multiply(0.3),
                        a.color,
                        self.stem_width.clamp(0.5, 4.0),
                    )
                }
                Style::Surface => {
                    let start = mesh.vertices.len() as u32;
                    for v in [a, b, c, d] {
                        mesh.colored_vertex(v.pos, v.color.gamma_multiply(0.7));
                    }
                    mesh.add_triangle(start, start + 1, start + 2);
                    mesh.add_triangle(start + 1, start + 3, start + 2);
                }
                _ => {}
            }
            let grid = self.style == Style::Wireframe
                || (self.style == Style::Surface && self.surface_mesh);
            if (grid || self.style == Style::Rings)
                && row.is_multiple_of(self.ring_spacing.clamp(1, 8))
            {
                line(&mut mesh, a, b, self.line_width.clamp(0.3, 3.0));
            }
            if (grid || self.style == Style::Meridians)
                && col.is_multiple_of(self.meridian_spacing.clamp(1, 8))
            {
                line(&mut mesh, a, c, self.line_width.clamp(0.3, 3.0));
            }
        }
        mesh
    }
    fn build_mesh(&self, rect: Rect, now: Instant, theme: &AppTheme) -> egui::Mesh {
        let rows = self.latitudes.clamp(12, 128);
        let cols = self.longitudes.clamp(24, 256);
        // Fixed framing prevents audio peaks from changing the camera scale.
        let scale = rect.width().min(rect.height()).max(0.0) * 0.27 * self.zoom.clamp(0.25, 8.0);
        let center = rect.center() + self.pan * rect.size();
        // Trigonometry is fixed across this mesh. Keep the same projection
        // arithmetic, but compute its six coefficients only once per frame.
        let (sy, cy) = self.yaw.sin_cos();
        let (se, ce) = self.elevation.sin_cos();
        let (sr, cr) = self.roll.sin_cos();
        let project = |[x, y, z]: [f32; 3]| {
            let horizontal = cy * x + sy * z;
            let forward = -sy * x + cy * z;
            let vertical = se * forward - ce * y;
            (
                center
                    + Vec2::new(
                        horizontal * cr - vertical * sr,
                        horizontal * sr + vertical * cr,
                    ) * scale,
                se * y + ce * forward,
            )
        };
        let radius = self.radius.clamp(0.05, 4.0);
        let displacement = self.displacement.clamp(0.0, 4.0);
        let stretch = [self.stretch_x, self.stretch_y, self.stretch_z].map(|v| v.clamp(0.1, 4.0));
        let columns: Vec<_> = (0..=cols)
            .map(|col| {
                let u = if col == cols {
                    0.0
                } else {
                    col as f32 / cols as f32
                };
                let f = self.frequency_fraction(u);
                let band = f * (BANDS - 1) as f32;
                let lo = (band as usize).min(BANDS - 1);
                (u, f, lo, (lo + 1).min(BANDS - 1), band.fract())
            })
            .collect();
        let mut vertices = Vec::with_capacity((rows + 1) * (cols + 1));
        for row in 0..=rows {
            let v = row as f32 / rows as f32;
            let age = if self.reverse_flow { 1.0 - v } else { v };
            let levels = self.levels_at(now, age * self.seconds.clamp(0.1, 30.0));
            let ripple = (self.motion == Motion::PoleRipples).then(|| self.ripple_at(now, age));
            let (sin_phi, cos_phi) = (v * PI).sin_cos();
            let taper = if self.is_cylinder() {
                let edge = self.cylinder.edge_falloff.clamp(0.0, 1.0) * 0.5;
                if edge > 0.0 {
                    let t = (v.min(1.0 - v) / edge).clamp(0.0, 1.0);
                    t * t * (3.0 - 2.0 * t)
                } else {
                    1.0
                }
            } else {
                sin_phi.max(0.0).powf(self.pole_taper.clamp(0.1, 4.0))
            };
            let row_radius = if self.is_cylinder() {
                radius
                    * (self.cylinder.top_radius.clamp(0.0, 3.0) * (1.0 - v)
                        + self.cylinder.bottom_radius.clamp(0.0, 3.0) * v)
            } else {
                radius
            };
            for &(u, f, lo, hi, fraction) in &columns {
                let level = ripple.map_or_else(
                    || {
                        levels.map_or(0.0, |a| {
                            self.history
                                .data
                                .settings
                                .intensity(a[lo] + (a[hi] - a[lo]) * fraction)
                        })
                    },
                    |(level, _)| level,
                );
                let theta =
                    u * TAU + self.offset.to_radians() + self.twist.to_radians() * (v - 0.5);
                let (sin_theta, cos_theta) = theta.sin_cos();
                let position = |r: f32| {
                    if self.is_cylinder() {
                        [
                            r * cos_theta * stretch[0],
                            self.cylinder.height.clamp(0.2, 8.0) * (0.5 - v) * stretch[1],
                            r * sin_theta * stretch[2],
                        ]
                    } else {
                        [
                            r * sin_phi * cos_theta * stretch[0],
                            r * cos_phi * stretch[1],
                            r * sin_phi * sin_theta * stretch[2],
                        ]
                    }
                };
                let r = (row_radius
                    + level.clamp(
                        if self.motion == Motion::PoleRipples {
                            -1.0
                        } else {
                            0.0
                        },
                        1.0,
                    ) * displacement
                        * taper
                        // Narrow ends reduce displacement continuously, so a zero-radius
                        // tip stays closed even with edge falloff disabled and loud audio.
                        * if self.is_cylinder() {
                            (row_radius / radius).min(1.0)
                        } else {
                            1.0
                        })
                .max(if self.is_cylinder() { 0.0 } else { 0.05 });
                let (pos, depth) = project(position(r));
                let (base, facing) = project(position(row_radius));
                let facing = if self.is_cylinder() {
                    ce * (-sy * cos_theta / stretch[0] + cy * sin_theta / stretch[2])
                } else {
                    facing
                };
                let color_t = match self.color_by {
                    ColorBy::Level => level.abs(),
                    ColorBy::Frequency => ripple.map_or(f, |(_, tone)| tone),
                    ColorBy::History => age,
                };
                let activity = if self.motion == Motion::PoleRipples {
                    level.abs().sqrt()
                } else {
                    level.abs()
                };
                let light = (self.idle_brightness + activity * (1.0 - self.idle_brightness))
                    .clamp(0.0, 1.0)
                    * self.brightness.clamp(0.0, 2.0);
                let color = self
                    .palette
                    .color_with_contrast(color_t, self.contrast, theme);
                let alpha = self.opacity.clamp(0.0, 1.0)
                    * if facing < 0.0 {
                        self.rear_opacity.clamp(0.0, 1.0)
                    } else {
                        1.0
                    };
                let c = |v: u8| (v as f32 * light).clamp(0.0, 255.0) as u8;
                vertices.push(Vertex {
                    pos,
                    depth,
                    base,
                    color: Color32::from_rgba_unmultiplied(
                        c(color.r()),
                        c(color.g()),
                        c(color.b()),
                        (alpha * 255.0) as u8,
                    ),
                });
            }
        }
        let stride = cols + 1;
        // Compute each depth once rather than four vertex reads on every
        // comparison during sorting. Painter order remains identical.
        let mut cells: Vec<_> = (0..rows)
            .flat_map(|r| (0..cols).map(move |c| (r, c)))
            .map(|(r, c)| {
                let depth = [
                    r * stride + c,
                    r * stride + c + 1,
                    (r + 1) * stride + c,
                    (r + 1) * stride + c + 1,
                ]
                .iter()
                .map(|&i| vertices[i].depth)
                .sum::<f32>();
                (r, c, depth)
            })
            .collect();
        let cap_start = vertices.len();
        if self.is_cylinder() && self.cylinder.caps {
            for end in [0, rows] {
                let v = end as f32 / rows as f32;
                let (pos, depth) = project([
                    0.0,
                    self.cylinder.height.clamp(0.2, 8.0) * (0.5 - v) * stretch[1],
                    0.0,
                ]);
                vertices.push(Vertex {
                    pos,
                    base: pos,
                    depth,
                    color: vertices[end * stride].color,
                });
                let end_radius = if end == 0 {
                    self.cylinder.top_radius
                } else {
                    self.cylinder.bottom_radius
                };
                if end_radius <= 0.0 {
                    continue;
                }
                for col in 0..cols {
                    cells.push((
                        rows + 1 + usize::from(end > 0),
                        col,
                        depth * 2.0
                            + vertices[end * stride + col].depth
                            + vertices[end * stride + col + 1].depth,
                    ));
                }
            }
        }
        cells.sort_unstable_by(|a, b| a.2.total_cmp(&b.2));
        let mut mesh = egui::Mesh::default();
        for (row, col, _) in cells {
            if row > rows {
                let end = row - rows - 1;
                let center = vertices[cap_start + end];
                let a = vertices[end * rows * stride + col];
                let b = vertices[end * rows * stride + col + 1];
                if self.style == Style::Surface {
                    let start = mesh.vertices.len() as u32;
                    for v in [center, a, b] {
                        mesh.colored_vertex(v.pos, v.color.gamma_multiply(0.7));
                    }
                    mesh.add_triangle(start, start + 1, start + 2);
                }
                if (self.style == Style::Wireframe
                    || self.style == Style::Meridians
                    || (self.style == Style::Surface && self.surface_mesh))
                    && col.is_multiple_of(self.meridian_spacing.clamp(1, 8))
                {
                    mesh_segment(
                        &mut mesh,
                        center.pos,
                        a.pos,
                        a.color,
                        a.color,
                        self.line_width.clamp(0.3, 3.0),
                    );
                }
                continue;
            }
            let a = vertices[row * stride + col];
            let b = vertices[row * stride + col + 1];
            let c = vertices[(row + 1) * stride + col];
            let d = vertices[(row + 1) * stride + col + 1];
            let line = |mesh: &mut egui::Mesh, a: Vertex, b: Vertex, width: f32| {
                mesh_segment(mesh, a.pos, b.pos, a.color, b.color, width)
            };
            match self.style {
                Style::Points
                    if row.is_multiple_of(self.dot_spacing.clamp(1, 8))
                        && col.is_multiple_of(self.dot_spacing.clamp(1, 8)) =>
                {
                    mesh_dot(
                        &mut mesh,
                        a.pos,
                        a.color,
                        self.dot_size.clamp(0.5, 8.0) * 0.5,
                    )
                }
                Style::Spikes
                    if row.is_multiple_of(self.stem_spacing.clamp(1, 8))
                        && col.is_multiple_of(self.stem_spacing.clamp(1, 8)) =>
                {
                    mesh_segment(
                        &mut mesh,
                        a.base,
                        a.pos,
                        a.color.gamma_multiply(0.3),
                        a.color,
                        self.stem_width.clamp(0.5, 4.0),
                    )
                }
                Style::Surface => {
                    let start = mesh.vertices.len() as u32;
                    for v in [a, b, c, d] {
                        mesh.colored_vertex(v.pos, v.color.gamma_multiply(0.7));
                    }
                    mesh.add_triangle(start, start + 1, start + 2);
                    mesh.add_triangle(start + 1, start + 3, start + 2);
                }
                _ => {}
            }
            let grid = self.style == Style::Wireframe
                || (self.style == Style::Surface && self.surface_mesh);
            if (grid || self.style == Style::Rings)
                && row.is_multiple_of(self.ring_spacing.clamp(1, 8))
            {
                line(&mut mesh, a, b, self.line_width.clamp(0.3, 3.0));
            }
            if self.is_cylinder() && row + 1 == rows && (grid || self.style == Style::Rings) {
                line(&mut mesh, c, d, self.line_width.clamp(0.3, 3.0));
            }
            if (grid || self.style == Style::Meridians)
                && col.is_multiple_of(self.meridian_spacing.clamp(1, 8))
            {
                line(&mut mesh, a, c, self.line_width.clamp(0.3, 3.0));
            }
        }
        mesh
    }
    pub fn draw(
        &mut self,
        ui: &mut egui::Ui,
        rect: Rect,
        frame: &AnalysisFrame,
        theme: &AppTheme,
        now: Instant,
    ) {
        self.history.update(frame, now, false);
        self.advance_camera(now);
        let painter = ui.painter_at(rect);
        let scene = painter.add(egui::Shape::Noop);
        // Reserve the gizmo's input area before the canvas so drags never orbit twice.
        let gizmo =
            Rect::from_min_size(rect.right_top() + Vec2::new(-88.0, 6.0), Vec2::splat(80.0));
        if self.show_gizmo {
            let action =
                camera_gizmo::draw_with_roll(ui, gizmo, self.yaw, self.elevation, self.roll, true);
            if action.orbit != Vec2::ZERO
                && let Some(pointer) = ui.input(|i| i.pointer.interact_pos())
            {
                self.trackball_drag(
                    pointer - action.orbit,
                    pointer,
                    gizmo.center(),
                    gizmo.width() * 0.4,
                );
            }
            if let Some((axis, positive)) = action.snap {
                (self.yaw, self.elevation) = axis.snap(positive, self.yaw, self.elevation);
                self.roll = 0.0;
                self.auto_orbit = false;
            }
        }
        let response = ui.interact(rect, ui.id().with("sphere-camera"), Sense::click_and_drag())
            .help_text("Drag near the center to tilt freely; drag around the edge to roll. Right-drag or Shift-drag to pan. Scroll to zoom; double-click to reset.");
        let over_gizmo = self.show_gizmo
            && ui.input(|i| {
                i.pointer
                    .press_origin()
                    .filter(|_| i.pointer.any_down())
                    .or_else(|| i.pointer.interact_pos())
                    .is_some_and(|p| gizmo.contains(p))
            });
        if !over_gizmo {
            let shift = ui.input(|i| i.modifiers.shift);
            if response.dragged_by(egui::PointerButton::Secondary)
                || response.dragged_by(egui::PointerButton::Middle)
                || (response.dragged() && shift)
            {
                self.pan += response.drag_delta() / rect.size().max(Vec2::splat(1.0));
                self.auto_orbit = false;
            } else if response.dragged_by(egui::PointerButton::Primary)
                && let Some(pointer) = response.interact_pointer_pos()
            {
                let (center, radius) = self.trackball_bounds(rect);
                self.trackball_drag(pointer - response.drag_delta(), pointer, center, radius);
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            }
            if response.double_clicked() {
                self.reset_camera();
            }
            if response.hovered() {
                let scroll = ui.input(|i| i.smooth_scroll_delta.y);
                if scroll.abs() > 0.01 {
                    self.zoom = (self.zoom * (scroll * 0.002).exp()).clamp(0.25, 8.0);
                }
            }
        }
        painter.set(
            scene,
            egui::Shape::Mesh(self.cached_mesh(rect.shrink(24.0), now, theme)),
        );
    }
}
#[derive(Clone, Copy)]
struct Vertex {
    pos: Pos2,
    base: Pos2,
    depth: f32,
    color: Color32,
}
fn wrap(a: f32) -> f32 {
    (a + PI).rem_euclid(TAU) - PI
}

module_palette!(SpectralSurface, palette);

#[cfg(test)]
mod tests {
    use super::super::frequency::HistoryRow;
    use super::super::trackball::dot;
    use super::*;
    fn populated(now: Instant) -> SpectralSurface {
        let mut sphere = SpectralSurface {
            latitudes: 24,
            longitudes: 48,
            ..SpectralSurface::default()
        };
        for n in 0..=90 {
            sphere.history.rows.push_back(HistoryRow {
                time: now - Duration::from_secs_f32((90 - n) as f32 / 30.0),
                sequence: n + 1,
                levels: std::array::from_fn(|b| {
                    -55.0 + 25.0 * (b as f32 * 0.13 + n as f32 * 0.2).sin()
                }),
                magnitudes: vec![].into(),
            });
        }
        sphere
    }
    #[test]
    fn paused_surface_reuses_mesh_and_invalidates_for_every_visible_change() {
        use std::sync::Arc;
        let now = Instant::now();
        let mut surface = populated(now);
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let theme = AppTheme::default();
        let first = surface.cached_mesh(rect, now, &theme);
        assert!(Arc::ptr_eq(&first, &surface.cached_mesh(rect, now, &theme)));
        surface.yaw += 0.2;
        let moved = surface.cached_mesh(rect, now, &theme);
        assert!(!Arc::ptr_eq(&first, &moved));
        surface.palette = Palette::Ember;
        let colored = surface.cached_mesh(rect, now, &theme);
        assert!(!Arc::ptr_eq(&moved, &colored));
        assert!(!Arc::ptr_eq(
            &colored,
            &surface.cached_mesh(rect, now + Duration::from_millis(8), &theme)
        ));
        surface.clear();
        assert!(surface.mesh_cache.is_none());
    }
    #[test]
    #[ignore = "manual CPU timing for paused sphere/cylinder mesh reuse"]
    fn benchmark_paused_spectral_surface() {
        let now = Instant::now();
        let mut surface = populated(now);
        surface.latitudes = 128;
        surface.longitudes = 256;
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 720.0));
        let theme = AppTheme::default();
        let start = Instant::now();
        for _ in 0..100 {
            std::hint::black_box(surface.build_mesh(rect, now, &theme));
        }
        let uncached = start.elapsed();
        surface.cached_mesh(rect, now, &theme);
        let start = Instant::now();
        for _ in 0..100 {
            std::hint::black_box(surface.cached_mesh(rect, now, &theme));
        }
        eprintln!(
            "Paused spectral surface, 100 dense mesh requests: {:?} uncached → {:?} cached",
            uncached,
            start.elapsed()
        );
    }
    #[test]
    fn cylinder_dimensions_taper_and_caps_define_a_cylinder_not_a_sphere() {
        let now = Instant::now();
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let theme = AppTheme::default();
        let mut cylinder = SpectralSurface::cylinder();
        cylinder.latitudes = 24;
        cylinder.longitudes = 48;
        cylinder.style = Style::Surface;
        cylinder.surface_mesh = false;
        cylinder.displacement = 0.0;
        cylinder.twist = 0.0;
        cylinder.yaw = 0.0;
        cylinder.elevation = 0.0;
        let mesh = cylinder.build_mesh(rect, now, &theme);
        assert_eq!(mesh.indices.len(), 24 * 48 * 6 + 2 * 48 * 3);
        let bounds = mesh.vertices.iter().fold(Rect::NOTHING, |mut bounds, v| {
            bounds.extend_with(v.pos);
            bounds
        });
        let scale = 600.0 * 0.27;
        assert!((bounds.width() - 2.0 * cylinder.radius * scale).abs() < 0.001);
        assert!((bounds.height() - cylinder.cylinder.height * scale).abs() < 0.001);
        cylinder.cylinder.caps = false;
        cylinder.cylinder.top_radius = 0.5;
        cylinder.cylinder.bottom_radius = 1.5;
        let mesh = cylinder.build_mesh(rect, now, &theme);
        assert_eq!(mesh.indices.len(), 24 * 48 * 6);
        for (y, radius) in [(bounds.top(), 0.5), (bounds.bottom(), 1.5)] {
            let width = mesh
                .vertices
                .iter()
                .filter(|v| (v.pos.y - y).abs() < 0.001)
                .map(|v| (v.pos.x - rect.center().x).abs())
                .fold(0.0_f32, f32::max);
            assert!((width - radius * scale).abs() < 0.001);
        }
    }
    #[test]
    fn cylinder_tips_stay_pointed_with_audio_and_caps_skip_zero_radius_ends() {
        let now = Instant::now();
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let theme = AppTheme::default();
        let mut cylinder = populated(now);
        cylinder.shape = Shape::Cylinder;
        cylinder.latitudes = 24;
        cylinder.longitudes = 48;
        cylinder.style = Style::Surface;
        cylinder.surface_mesh = false;
        cylinder.cylinder.edge_falloff = 0.0;
        cylinder.displacement = 4.0;
        cylinder.yaw = 0.0;
        cylinder.elevation = 0.0;
        cylinder.roll = 0.0;
        for motion in [Motion::WrappedSpectrum, Motion::PoleRipples] {
            cylinder.motion = motion;
            for (top, bottom) in [(0.0, 1.0), (1.0, 0.0), (0.0, 0.0)] {
                cylinder.cylinder.top_radius = top;
                cylinder.cylinder.bottom_radius = bottom;
                for caps in [false, true] {
                    cylinder.cylinder.caps = caps;
                    let mesh = cylinder.build_mesh(rect, now, &theme);
                    assert!(mesh.is_valid());
                    assert!(
                        mesh.vertices
                            .iter()
                            .all(|v| v.pos.x.is_finite() && v.pos.y.is_finite())
                    );
                    let cap_count = if caps {
                        usize::from(top > 0.0) + usize::from(bottom > 0.0)
                    } else {
                        0
                    };
                    assert_eq!(mesh.indices.len(), 24 * 48 * 6 + cap_count * 48 * 3);
                    for (radius, direction) in [(top, -1.0), (bottom, 1.0)] {
                        if radius != 0.0 {
                            continue;
                        }
                        let tip_y = rect.center().y
                            + direction * cylinder.cylinder.height * 0.5 * 600.0 * 0.27;
                        let tip: Vec<_> = mesh
                            .vertices
                            .iter()
                            .filter(|v| (v.pos.y - tip_y).abs() < 0.001)
                            .collect();
                        assert!(!tip.is_empty());
                        assert!(
                            tip.iter()
                                .all(|v| (v.pos.x - rect.center().x).abs() < 0.001)
                        );
                    }
                }
            }
        }
        let saved = cylinder.settings_snapshot();
        let decoded = serde_json::from_str(&serde_json::to_string(&saved).unwrap()).unwrap();
        let mut restored = SpectralSurface::cylinder();
        restored.apply_settings(&decoded);
        assert_eq!(saved, restored.settings_snapshot());
    }

    #[test]
    fn cylinder_audio_styles_and_ripples_are_finite_and_keep_history_through_reset() {
        let now = Instant::now();
        let mut cylinder = populated(now);
        cylinder.shape = Shape::Cylinder;
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let theme = AppTheme::default();
        let audio = cylinder.build_mesh(rect, now, &theme);
        let silent = cylinder.build_mesh(rect, now + Duration::from_secs(35), &theme);
        assert_ne!(audio.vertices, silent.vertices);
        for motion in [Motion::WrappedSpectrum, Motion::PoleRipples] {
            cylinder.motion = motion;
            for style in Style::ALL {
                cylinder.style = style;
                for caps in [false, true] {
                    cylinder.cylinder.caps = caps;
                    cylinder.cylinder.edge_falloff = 0.0;
                    cylinder.cylinder.top_radius = 0.1;
                    cylinder.cylinder.bottom_radius = 3.0;
                    cylinder.displacement = 4.0;
                    let mesh = cylinder.build_mesh(rect, now, &theme);
                    assert!(mesh.is_valid() && !mesh.vertices.is_empty());
                    assert!(mesh.vertices.iter().all(|v| v.pos.is_finite()));
                }
            }
        }
        cylinder.trackball_drag(
            Pos2::new(400.0, 200.0),
            Pos2::new(350.0, 300.0),
            rect.center(),
            160.0,
        );
        let orientation = cylinder.orientation_axes();
        let settings = cylinder.settings_snapshot();
        let decoded: SurfaceSettings =
            serde_json::from_slice(&serde_json::to_vec(&settings).unwrap()).unwrap();
        let mut restored = SpectralSurface::cylinder();
        restored.apply_settings(&decoded);
        assert_eq!(restored.orientation_axes(), orientation);
        assert_eq!(restored.settings_snapshot(), settings);
        cylinder.reset_settings();
        assert!(cylinder.is_cylinder());
        assert_eq!(cylinder.history.rows.len(), 91);
        assert_eq!(
            cylinder.settings_snapshot(),
            SpectralSurface::cylinder().settings_snapshot()
        );
    }
    #[test]
    #[ignore = "manual sphere CPU mesh timing, not a frame-rate test"]
    fn benchmark_sphere_mesh() {
        let now = Instant::now();
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 900.0));
        let theme = AppTheme::default();
        for (rows, cols) in [(64, 128), (128, 256)] {
            let mut sphere = populated(now);
            sphere.latitudes = rows;
            sphere.longitudes = cols;
            for motion in [Motion::WrappedSpectrum, Motion::PoleRipples] {
                sphere.motion = motion;
                for optimized in [false, true] {
                    let mesh = || {
                        if optimized {
                            sphere.build_mesh(rect, now, &theme)
                        } else {
                            sphere.build_mesh_reference(rect, now, &theme)
                        }
                    };
                    std::hint::black_box(mesh());
                    let start = Instant::now();
                    for _ in 0..100 {
                        std::hint::black_box(mesh());
                    }
                    eprintln!(
                        "SpectralSurface {rows}x{cols} {motion:?} {}: {:.3} ms/mesh",
                        if optimized { "optimized" } else { "reference" },
                        start.elapsed().as_secs_f64() * 10.0
                    );
                }
            }
        }
    }
    #[test]
    fn optimized_mesh_preserves_geometry_colors_and_painter_order() {
        let now = Instant::now();
        let rect = Rect::from_min_size(Pos2::new(40.0, 20.0), Vec2::new(800.0, 600.0));
        let theme = AppTheme::default();
        let mut sphere = populated(now);
        sphere.twist = 137.0;
        sphere.offset = 19.0;
        sphere.pan = Vec2::new(0.2, -0.1);
        sphere.roll = 0.8;
        sphere.stretch_x = 1.3;
        sphere.stretch_z = 0.7;
        for motion in [Motion::WrappedSpectrum, Motion::PoleRipples] {
            sphere.motion = motion;
            for style in [
                Style::Wireframe,
                Style::Surface,
                Style::Rings,
                Style::Meridians,
                Style::Points,
                Style::Spikes,
            ] {
                sphere.style = style;
                for elevation in [-PI * 0.5, 0.35, PI * 0.5] {
                    sphere.elevation = elevation;
                    let reference = sphere.build_mesh_reference(rect, now, &theme);
                    let actual = sphere.build_mesh(rect, now, &theme);
                    assert_eq!(actual.indices, reference.indices);
                    assert_eq!(actual.vertices.len(), reference.vertices.len());
                    for (a, b) in actual.vertices.iter().zip(&reference.vertices) {
                        assert!(
                            (a.pos - b.pos).length() < 0.001,
                            "{motion:?} {style:?}: {:?} != {:?}",
                            a.pos,
                            b.pos
                        );
                        assert_eq!(a.color, b.color);
                        assert_eq!(a.uv, b.uv);
                    }
                }
            }
        }
    }
    #[test]
    fn trackball_composes_free_rotation_at_poles_and_rolls_and_reverses_cleanly() {
        let center = Pos2::new(300.0, 200.0);
        let basis = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        for elevation in [-PI * 0.5, -PI * 0.4999, 0.0, PI * 0.4999, PI * 0.5] {
            for roll in [-2.0, 0.0, 1.2] {
                let mut sphere = SpectralSurface {
                    yaw: 0.8,
                    elevation,
                    roll,
                    ..SpectralSurface::default()
                };
                let before = basis.map(|v| sphere.view_vector(v));
                let from = center + Vec2::new(70.0, 10.0);
                let to = center + Vec2::new(-30.0, 80.0);
                sphere.trackball_drag(from, to, center, 100.0);
                let after = basis.map(|v| sphere.view_vector(v));
                assert_ne!(before, after);
                for v in after {
                    assert!((dot(v.map(f64::from), v.map(f64::from)) - 1.0).abs() < 1e-5);
                }
                assert!(dot(after[0].map(f64::from), after[1].map(f64::from)).abs() < 1e-5);
                sphere.trackball_drag(to, from, center, 100.0);
                for (a, b) in before.into_iter().zip(basis.map(|v| sphere.view_vector(v))) {
                    for (a, b) in a.into_iter().zip(b) {
                        assert!((a - b).abs() < 1e-5, "{elevation} {roll}: {a} != {b}");
                    }
                }
            }
        }
        let mut sphere = SpectralSurface {
            yaw: 0.0,
            elevation: 0.0,
            roll: 0.0,
            ..SpectralSurface::default()
        };
        sphere.trackball_drag(
            center + Vec2::new(100.0, 0.0),
            center + Vec2::new(0.0, 100.0),
            center,
            100.0,
        );
        assert!(sphere.roll.abs() > 0.3);
    }
    #[test]
    fn trackball_pivot_follows_pan_and_repeated_rotations_keep_orientation_and_history() {
        let now = Instant::now();
        let mut a = populated(now);
        let mut b = populated(now);
        b.pan = Vec2::new(0.3, -0.2);
        let original_pan = b.pan;
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        for n in 0..1000 {
            let (ac, ar) = a.trackball_bounds(rect);
            let (bc, br) = b.trackball_bounds(rect);
            assert_eq!(ar, br);
            assert_ne!(ac, bc);
            let from = Vec2::angled(n as f32 * 0.13) * ar * 0.8;
            let to = Vec2::angled(n as f32 * 0.13 + 0.03) * ar * 0.8;
            a.trackball_drag(ac + from, ac + to, ac, ar);
            b.trackball_drag(bc + from, bc + to, bc, br);
        }
        for point in [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]] {
            let va = a.view_vector(point);
            let vb = b.view_vector(point);
            for (a, b) in va.into_iter().zip(vb) {
                assert!((a - b).abs() < 0.001);
            }
            assert!((dot(va.map(f64::from), va.map(f64::from)) - 1.0).abs() < 1e-5);
        }
        assert_eq!(b.pan, original_pan);
        assert_eq!(a.history.rows.len(), 91);
        let settings = b.settings_snapshot();
        let mut restored = SpectralSurface::default();
        restored.apply_settings(&settings);
        assert_eq!(
            restored.view_vector([1.0, 2.0, 3.0]),
            b.view_vector([1.0, 2.0, 3.0])
        );
    }
    #[test]
    fn canvas_drag_rotates_freely_and_pan_gestures_preserve_all_rotation_angles() {
        for (button, modifiers) in [
            (egui::PointerButton::Primary, egui::Modifiers::NONE),
            (egui::PointerButton::Secondary, egui::Modifiers::NONE),
            (egui::PointerButton::Primary, egui::Modifiers::SHIFT),
        ] {
            let ctx = egui::Context::default();
            let now = Instant::now();
            let mut sphere = SpectralSurface {
                show_gizmo: false,
                pan: Vec2::new(0.15, -0.1),
                ..SpectralSurface::default()
            };
            let before = sphere.settings_snapshot();
            let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(600.0, 400.0));
            let (center, radius) = sphere.trackball_bounds(rect);
            let from = center + Vec2::new(radius * 0.9, 0.0);
            let to = from + Vec2::new(-20.0, 50.0);
            let mut render = |mut events: Vec<egui::Event>| {
                events.insert(0, egui::Event::ModifiersChanged(modifiers));
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))),
                        events,
                        ..Default::default()
                    },
                    |ui| {
                        sphere.draw(
                            ui,
                            rect,
                            &AnalysisFrame::default(),
                            &AppTheme::default(),
                            now,
                        );
                    },
                );
                output.textures_delta.clear();
            };
            render(vec![]);
            render(vec![egui::Event::PointerMoved(from)]);
            render(vec![egui::Event::PointerButton {
                pos: from,
                button,
                pressed: true,
                modifiers,
            }]);
            render(vec![egui::Event::PointerMoved(to)]);
            render(vec![egui::Event::PointerButton {
                pos: to,
                button,
                pressed: false,
                modifiers,
            }]);
            render(vec![]);
            if button == egui::PointerButton::Primary && modifiers == egui::Modifiers::NONE {
                assert_ne!(sphere.roll, before.roll);
                assert_eq!(sphere.pan, before.pan);
            } else {
                assert_eq!(
                    (sphere.yaw, sphere.elevation, sphere.roll),
                    (before.yaw, before.elevation, before.roll)
                );
                assert_ne!(sphere.pan, before.pan);
            }
            assert_eq!(sphere.zoom, before.zoom);
        }
    }
    #[test]
    fn manual_ripple_travels_between_poles_at_selected_speed_and_damps() {
        let now = Instant::now();
        let mut sphere = SpectralSurface {
            motion: Motion::PoleRipples,
            ripple_trigger: Some(now),
            ..SpectralSurface::default()
        };
        let quarter = now + Duration::from_secs_f32(sphere.ripple_travel_time() * 0.25);
        let (crest, _) = sphere.ripple_at(quarter, 0.25);
        assert!((crest - (-0.25_f32).exp()).abs() < 1e-5);
        assert!(sphere.ripple_at(quarter, 0.7).0.abs() < 1e-5);
        let middle = now + Duration::from_secs_f32(sphere.ripple_travel_time() * 0.5);
        assert!((sphere.ripple_at(middle, 0.5).0 - (-0.5_f32).exp()).abs() < 1e-5);
        sphere.ripple_speed = 2.0;
        let faster = now + Duration::from_secs_f32(sphere.ripple_travel_time() * 0.5);
        assert!((sphere.ripple_at(faster, 0.5).0 - (-0.5_f32).exp()).abs() < 1e-5);
        sphere.ripple_damping = 0.0;
        assert!((sphere.ripple_at(faster, 0.5).0 - 1.0).abs() < 1e-5);
        assert_eq!(sphere.ripple_at(now + Duration::from_secs(35), 0.5).0, 0.0);
        sphere.clear();
        assert_eq!(sphere.ripple_at(now, 0.5).0, 0.0);
    }
    #[test]
    fn audio_ripples_are_circular_and_disappear_without_audio_or_manual_trigger() {
        let now = Instant::now();
        let mut sphere = populated(now);
        sphere.motion = Motion::PoleRipples;
        sphere.ripple_impulse = 0.0;
        let (level, _) = sphere.ripple_at(now, 0.43);
        assert!(level.abs() > 0.001);
        let radius = |p: [f32; 3]| p.iter().map(|v| v * v).sum::<f32>().sqrt();
        let r = radius(sphere.point(0.0, 0.43, level));
        for u in [0.125, 0.25, 0.5, 0.875] {
            assert!((radius(sphere.point(u, 0.43, level)) - r).abs() < 1e-5);
        }
        assert_eq!(sphere.ripple_at(now + Duration::from_secs(35), 0.43).0, 0.0);
        let before = sphere.history.rows.len();
        sphere.motion = Motion::WrappedSpectrum;
        sphere.motion = Motion::PoleRipples;
        assert_eq!(sphere.history.rows.len(), before);
        // A trough contracts the sphere rather than being clipped away.
        assert!(radius(sphere.point(0.1, 0.5, -0.5)) < sphere.radius);
        sphere.displacement = 4.0;
        assert!(sphere.point(0.1, 0.5, -1.0).iter().all(|v| v.is_finite()));
    }
    #[test]
    fn ripple_settings_roundtrip_and_older_spheres_keep_wrapped_spectrum() {
        let sphere = SpectralSurface {
            motion: Motion::PoleRipples,
            ripple_speed: 2.3,
            ripple_damping: 0.7,
            ..SpectralSurface::default()
        };
        let settings = sphere.settings_snapshot();
        let decoded: SurfaceSettings =
            serde_json::from_value(serde_json::to_value(&settings).unwrap()).unwrap();
        assert_eq!(settings, decoded);
        let mut old = serde_json::to_value(SpectralSurface::default().settings_snapshot()).unwrap();
        for key in [
            "cylinder",
            "motion",
            "ripple_speed",
            "ripple_rate",
            "ripple_damping",
            "ripple_impulse",
        ] {
            old.as_object_mut().unwrap().remove(key);
        }
        let decoded: SurfaceSettings = serde_json::from_value(old).unwrap();
        assert_eq!(decoded, SpectralSurface::default().settings_snapshot());
    }
    #[test]
    fn spherical_mapping_closes_seam_and_poles_and_moves_only_excited_frequencies() {
        let mut sphere = SpectralSurface::default();
        for repeats in 1..=4 {
            sphere.repeats = repeats;
            for v in [0.0, 0.1, 0.5, 0.9, 1.0] {
                for (a, b) in sphere
                    .point(0.0, v, 0.8)
                    .into_iter()
                    .zip(sphere.point(1.0, v, 0.8))
                {
                    assert!((a - b).abs() < 1.0e-5);
                }
            }
            assert_eq!(sphere.frequency_fraction(0.0), 0.0);
            assert_eq!(sphere.frequency_fraction(0.5 / repeats as f32), 1.0);
            assert_eq!(sphere.frequency_fraction(1.0 / repeats as f32), 0.0);
        }
        for pole in [0.0, 1.0] {
            for u in [0.0, 0.25, 0.5, 0.75] {
                let p = sphere.point(u, pole, 1.0);
                assert!(p[0].abs() < 1.0e-6 && p[2].abs() < 1.0e-6);
                assert!((p[1].abs() - sphere.radius).abs() < 1.0e-6);
            }
        }
        let radius = |p: [f32; 3]| p.iter().map(|v| v * v).sum::<f32>().sqrt();
        assert!(
            (radius(sphere.point(0.25, 0.5, 1.0)) - sphere.radius - sphere.displacement).abs()
                < 1.0e-5
        );
        assert!((radius(sphere.point(0.75, 0.5, 0.0)) - sphere.radius).abs() < 1.0e-5);
    }
    #[test]
    fn history_interpolates_real_samples_and_leaves_dropouts_unexcited() {
        let now = Instant::now();
        let mut sphere = SpectralSurface::default();
        for (age, db, sequence) in [(0.1, -80.0, 1), (0.0, -20.0, 2)] {
            sphere.history.rows.push_back(HistoryRow {
                time: now - Duration::from_secs_f32(age),
                levels: [db; BANDS],
                sequence,
                magnitudes: vec![].into(),
            });
        }
        assert!((sphere.levels_at(now, 0.05).unwrap()[0] + 50.0).abs() < 0.001);
        assert!(
            sphere
                .levels_at(now + Duration::from_secs(1), 0.0)
                .is_none()
        );
        assert!(sphere.levels_at(now, 1.0).is_none());
        // A long break between captures must not be interpolated.
        sphere.history.rows.front_mut().unwrap().time = now - Duration::from_secs(2);
        assert!(sphere.levels_at(now, 1.0).is_none());
    }
    #[test]
    fn every_style_renders_finite_bounded_audio_geometry_and_changes_with_history() {
        let now = Instant::now();
        let mut sphere = populated(now);
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let theme = AppTheme::default();
        for (motion, style) in [Motion::WrappedSpectrum, Motion::PoleRipples]
            .into_iter()
            .flat_map(|motion| Style::ALL.map(|style| (motion, style)))
        {
            sphere.motion = motion;
            sphere.style = style;
            for (yaw, elevation) in [(0.0, 0.0), (1.1, PI * 0.5), (-2.0, -0.8)] {
                sphere.yaw = yaw;
                sphere.elevation = elevation;
                let mesh = sphere.build_mesh(rect, now, &theme);
                assert!(!mesh.indices.is_empty() && mesh.is_valid());
                assert!(mesh.vertices.len() < 50_000);
                assert!(mesh.vertices.iter().all(|v| v.pos.is_finite()));
                let silent = sphere.build_mesh(rect, now + Duration::from_secs(35), &theme);
                assert_ne!(mesh.vertices, silent.vertices);
            }
        }
        sphere.style = Style::Wireframe;
        let original = sphere.build_mesh(rect, now, &theme);
        sphere.reverse_flow = true;
        assert_ne!(
            original.vertices,
            sphere.build_mesh(rect, now, &theme).vertices
        );
        sphere.latitudes = usize::MAX;
        sphere.longitudes = usize::MAX;
        let dense = sphere.build_mesh(rect, now, &theme);
        assert!(dense.is_valid() && dense.vertices.len() <= 262_144);
    }
    #[test]
    fn orbit_is_time_based_freezes_with_the_capture_clock_and_reset_keeps_history() {
        let now = Instant::now();
        let mut sphere = populated(now);
        sphere.auto_orbit = true;
        sphere.advance_camera(now);
        let start = sphere.yaw;
        for n in 1..=60 {
            sphere.advance_camera(now + Duration::from_secs_f32(n as f32 / 60.0));
        }
        assert!((sphere.yaw - start - sphere.orbit_speed.to_radians()).abs() < 1.0e-5);
        let end = sphere.yaw;
        sphere.advance_camera(now + Duration::from_secs(1));
        assert_eq!(sphere.yaw, end);
        sphere.trackball_drag(Pos2::ZERO, Pos2::new(5.0, 2.0), Pos2::ZERO, 100.0);
        assert!(!sphere.auto_orbit);
        sphere.reset_settings();
        assert_eq!(sphere.history.rows.len(), 91);
        assert_eq!(
            sphere.settings_snapshot(),
            SpectralSurface::default().settings_snapshot()
        );
    }
}

#[cfg(test)]
mod preview {
    use super::*;
    #[test]
    #[ignore = "manual visual review: writes the actual renderer mesh to /tmp"]
    fn export_sphere_mesh() {
        let now = Instant::now();
        let cylinder = std::env::var_os("SYMPHOS_PREVIEW_CYLINDER").is_some();
        let mut sphere = if cylinder {
            SpectralSurface::cylinder()
        } else {
            SpectralSurface::default()
        };
        if std::env::var_os("SYMPHOS_PREVIEW_RIPPLES").is_some() {
            sphere.motion = Motion::PoleRipples;
        }
        for n in 0..=90 {
            sphere
                .history
                .rows
                .push_back(super::super::frequency::HistoryRow {
                    time: now - Duration::from_secs_f32((90 - n) as f32 / 30.0),
                    sequence: n + 1,
                    levels: std::array::from_fn(|b| {
                        let x = b as f32 / (BANDS - 1) as f32;
                        let phase = n as f32 * 0.12;
                        let bass = (-((x - 0.12 - 0.04 * phase.sin()) / 0.09).powi(2)).exp();
                        let mids = (-((x - 0.5 - 0.1 * phase.cos()) / 0.14).powi(2)).exp();
                        -85.0 + 60.0 * (bass + 0.7 * mids).min(1.0) + 8.0 * (x * 55.0 + phase).sin()
                    }),
                    magnitudes: vec![].into(),
                });
        }
        let mesh = sphere.build_mesh(
            Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 900.0)),
            now,
            &AppTheme::default(),
        );
        let value = serde_json::json!({
            "vertices": mesh.vertices.iter().map(|v| (v.pos.x,v.pos.y,v.color.to_srgba_unmultiplied())).collect::<Vec<_>>(),
            "indices": mesh.indices
        });
        std::fs::write(
            if cylinder {
                "/tmp/symphos-cylinder-mesh.json"
            } else {
                "/tmp/symphos-sphere-mesh.json"
            },
            serde_json::to_vec(&value).unwrap(),
        )
        .unwrap();
    }
}

impl super::camera::Controlled for SpectralSurface {
    fn camera(&mut self) -> super::camera::Camera<'_> {
        let s = self;
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
            default_speed: 8.0,
            speed_limit: 360.0,
            speed_key: "orbit_speed",
            reverse: Some(&mut s.orbit_reverse),
            motion: &mut s.camera_motion,
            gizmo: &mut s.show_gizmo,
        }
    }
}
