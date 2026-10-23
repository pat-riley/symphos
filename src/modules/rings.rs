//! Live time-domain waveforms carried by configurable 3D orbital rings.
use super::{
    Palette,
    camera_gizmo::{self, Axis},
    label, settings_group, settings_panel, trackball,
    waterfall::mesh_segment,
    waveform::{WaveformChannel, capture_window},
};
use crate::{analysis::AnalysisFrame, help::HoverHelp, parameter::Parameter, theme::AppTheme};
use egui::{self, Color32, Pos2, Rect, Sense, Vec2};
use std::{f32::consts::TAU, time::Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Arrangement {
    Orbital,
    Concentric,
    Stacked,
}
impl Arrangement {
    fn label(self) -> &'static str {
        match self {
            Self::Orbital => "Orbital",
            Self::Concentric => "Concentric",
            Self::Stacked => "Stacked",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum ColorBy {
    Ring,
    Amplitude,
    Position,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub(crate) struct RingsSettings {
    camera_motion: super::camera_motion::Settings,
    auto_orbit: bool,
    orbit_speed: f32,
    arrangement: Arrangement,
    rings: usize,
    radius: f32,
    spacing: f32,
    inclination: f32,
    fan: f32,
    ellipticity: f32,
    stack_height: f32,
    resolution: usize,
    channel: WaveformChannel,
    stereo: bool,
    window_ms: f32,
    trigger: bool,
    rising: bool,
    threshold: f32,
    gain: f32,
    auto_scale: bool,
    amplitude: f32,
    sideways: f32,
    smoothing_ms: f32,
    line_smoothing: f32,
    seam: f32,
    repeats: usize,
    phase_offset: f32,
    motion_speed: f32,
    spin_speed: f32,
    alternate: bool,
    palette: Palette,
    color_by: ColorBy,
    brightness: f32,
    contrast: f32,
    opacity: f32,
    rear_visibility: f32,
    audio_light: f32,
    thickness: f32,
    glow: f32,
    guides: bool,
    show_gizmo: bool,
    baselines: bool,
    yaw: f32,
    elevation: f32,
    roll: f32,
    zoom: f32,
    pan: Vec2,
}
impl Default for RingsSettings {
    fn default() -> Self {
        Self {
            camera_motion: Default::default(),
            auto_orbit: false,
            orbit_speed: 10.0,
            arrangement: Arrangement::Orbital,
            rings: 4,
            radius: 1.0,
            spacing: 0.1,
            inclination: 65.0,
            fan: 180.0,
            ellipticity: 0.0,
            stack_height: 1.5,
            resolution: 768,
            channel: WaveformChannel::Mid,
            stereo: true,
            window_ms: 80.0,
            trigger: true,
            rising: true,
            threshold: 0.0,
            gain: 1.0,
            auto_scale: false,
            amplitude: 0.65,
            sideways: 0.12,
            smoothing_ms: 25.0,
            line_smoothing: 0.2,
            seam: 0.04,
            repeats: 1,
            phase_offset: 0.1,
            motion_speed: 8.0,
            spin_speed: 12.0,
            alternate: true,
            palette: Palette::Spectrum,
            color_by: ColorBy::Ring,
            brightness: 1.2,
            contrast: 1.0,
            opacity: 0.95,
            rear_visibility: 0.35,
            audio_light: 0.5,
            thickness: 1.4,
            glow: 0.2,
            guides: true,
            show_gizmo: true,
            baselines: false,
            yaw: -0.35,
            elevation: 0.35,
            roll: 0.0,
            zoom: 1.0,
            pan: Vec2::ZERO,
        }
    }
}
#[derive(Default)]
pub struct Rings {
    settings: RingsSettings,
    waves: [Vec<f32>; 4],
    last_draw: Option<Instant>,
    epoch: Option<u64>,
    orbit_phase: f32,
    spin_phase: f32,
}
#[derive(Clone, Copy)]
struct Vertex {
    position: Pos2,
    depth: f32,
    color: Color32,
}
impl Rings {
    pub fn settings_snapshot(&self) -> RingsSettings {
        self.settings.clone()
    }
    pub fn apply_settings(&mut self, settings: &RingsSettings) {
        self.settings = settings.clone();
    }
    pub fn reset_settings(&mut self) {
        *self = Self::default();
    }
    pub fn clear(&mut self) {
        self.waves = Default::default();
        self.last_draw = None;
        self.epoch = None;
        self.orbit_phase = 0.0;
        self.spin_phase = 0.0;
    }
    pub fn controls(&mut self, ui: &mut egui::Ui) {
        let s = &mut self.settings;
        macro_rules! p {
            ($field:ident, $range:expr, $default:expr, $name:expr, $help:expr, $ui:ident) => {
                $ui.add(
                    Parameter::new(&mut s.$field, $range, $default)
                        .key(stringify!($field))
                        .text($name),
                )
                .help_text($help);
            };
        }
        settings_panel(ui, "Geometry", |ui| {
            settings_group(ui, "Ring arrangement", |ui| {
                super::properties_combo("rings-arrangement", 180.0, 3)
                    .selected_text(s.arrangement.label())
                    .show_ui(ui, |ui| {
                        for mode in [
                            Arrangement::Orbital,
                            Arrangement::Concentric,
                            Arrangement::Stacked,
                        ] {
                            ui.selectable_value(&mut s.arrangement, mode, mode.label());
                        }
                    });
                p!(
                    rings,
                    1..=12,
                    4,
                    "Ring count",
                    "Number of independent waveform rings.",
                    ui
                );
                p!(
                    radius,
                    0.2..=2.5,
                    1.0,
                    "Radius",
                    "Resting radius of the innermost ring.",
                    ui
                );
                p!(
                    spacing,
                    0.0..=0.4,
                    0.1,
                    "Ring spacing",
                    "Additional radius for each successive ring.",
                    ui
                );
                p!(
                    ellipticity,
                    0.0..=0.85,
                    0.0,
                    "Ellipticity",
                    "Flatten circles into elliptical orbits.",
                    ui
                );
                if s.arrangement == Arrangement::Orbital {
                    p!(
                        inclination,
                        0.0..=90.0,
                        65.0,
                        "Inclination °",
                        "Tilt each orbital plane away from horizontal.",
                        ui
                    );
                    p!(
                        fan,
                        0.0..=360.0,
                        180.0,
                        "Orbit spread °",
                        "Distribute the tilted planes around the center.",
                        ui
                    );
                }
                if s.arrangement == Arrangement::Stacked {
                    p!(
                        stack_height,
                        0.0..=4.0,
                        1.5,
                        "Stack height",
                        "Distance from the bottom ring to the top ring.",
                        ui
                    );
                }
                p!(
                    resolution,
                    128..=2048,
                    768,
                    "Samples / ring",
                    "Trace resolution. Lower values reduce rendering work; peak sampling retains short transients.",
                    ui
                );
            });
            settings_group(ui, "Movement", |ui| {
                p!(
                    motion_speed,
                    -90.0..=90.0,
                    8.0,
                    "Orbit °/s",
                    "Precession of the orbital planes. Zero holds their arrangement still.",
                    ui
                );
                p!(
                    spin_speed,
                    -180.0..=180.0,
                    12.0,
                    "Trace spin °/s",
                    "Travel of the waveform around each ring, independent of audio capture.",
                    ui
                );
                ui.checkbox(&mut s.alternate, "Alternate rotation direction");
                p!(
                    phase_offset,
                    0.0..=1.0,
                    0.1,
                    "Ring phase",
                    "Offset the waveform position on successive rings.",
                    ui
                );
            });
        });
        settings_panel(ui, "Channels", |ui| {
            settings_group(ui, "Waveform input", |ui| {
                ui.checkbox(&mut s.stereo, "Alternate Left / Right rings").help_text("Odd rings show Left and even rings show Right, using the same time window and gain.");
                ui.add_enabled_ui(!s.stereo, |ui| {
                    super::properties_combo("rings-channel", 160.0, 4)
                        .selected_text(s.channel.label())
                        .show_ui(ui, |ui| {
                            for channel in WaveformChannel::ALL {
                                ui.selectable_value(&mut s.channel, channel, channel.label());
                            }
                        });
                });
                ui.small("Uses the same live audio samples as Waveform.");
            });
        });
        settings_panel(ui, "Time Window", |ui| {
            settings_group(ui, "Capture", |ui| {
                ui.add(
                    Parameter::new(&mut s.window_ms, 10.0..=500.0, 80.0)
                        .key("window_ms")
                        .logarithmic(true)
                        .text("Window ms"),
                )
                .help_text("Audio duration around one ring, independent of the FFT size.");
                p!(
                    repeats,
                    1..=8,
                    1,
                    "Waveform repeats",
                    "Repeat the captured window around each ring.",
                    ui
                );
                p!(
                    seam,
                    0.0..=0.2,
                    0.04,
                    "Seam blend",
                    "Ease the start and end of each waveform window to the resting ring to avoid a jump at the join.",
                    ui
                );
            });
            settings_group(ui, "Trigger", |ui| {
                ui.checkbox(&mut s.trigger, "Stabilize repeating signals");
                ui.add_enabled_ui(s.trigger, |ui| {
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut s.rising, true, "Rising");
                        ui.selectable_value(&mut s.rising, false, "Falling");
                    });
                    p!(
                        threshold,
                        -1.0..=1.0,
                        0.0,
                        "Threshold",
                        "Crossing level in original sample amplitude; stereo rings remain aligned.",
                        ui
                    );
                });
            });
        });
        settings_panel(ui, "Signal Response", |ui| {
            settings_group(ui, "Amplitude", |ui| {
                ui.checkbox(&mut s.auto_scale, "Auto scale").help_text("Fit the visible waveform using a shared stereo gain. Off preserves relative audio levels.");
                ui.add(
                    Parameter::new(&mut s.gain, 0.1..=10.0, 1.0)
                        .key("gain")
                        .logarithmic(true)
                        .text("Input gain ×"),
                );
                p!(
                    amplitude,
                    0.0..=2.0,
                    0.65,
                    "Radial movement",
                    "Signed waveform amplitude pushes each trace inward and outward.",
                    ui
                );
                p!(
                    sideways,
                    0.0..=2.0,
                    0.12,
                    "Sideways movement",
                    "Move the waveform perpendicular to its orbital plane.",
                    ui
                );
            });
            settings_group(ui, "Smoothing", |ui| {
                p!(
                    smoothing_ms,
                    0.0..=500.0,
                    25.0,
                    "Response ms",
                    "Time-based smoothing of successive waveforms. Zero follows samples directly.",
                    ui
                );
                p!(
                    line_smoothing,
                    0.0..=1.0,
                    0.2,
                    "Line smoothing",
                    "Blend neighboring trace samples to soften jagged detail.",
                    ui
                );
            });
        });
        settings_panel(ui, "Appearance", |ui| {
            settings_group(ui, "Trace & glow", |ui| {
                p!(
                    thickness,
                    0.5..=5.0,
                    1.4,
                    "Line width px",
                    "Width of the waveform trace.",
                    ui
                );
                p!(
                    glow,
                    0.0..=1.0,
                    0.2,
                    "Glow",
                    "Soft, wider line around each trace.",
                    ui
                );
                p!(
                    brightness,
                    0.0..=3.0,
                    1.2,
                    "Brightness",
                    "Overall trace brightness.",
                    ui
                );
                p!(
                    audio_light,
                    0.0..=1.0,
                    0.5,
                    "Audio brightness",
                    "Local peaks brighten the line; zero keeps brightness constant.",
                    ui
                );
                p!(
                    opacity,
                    0.0..=1.0,
                    0.95,
                    "Opacity",
                    "Transparency of the traces.",
                    ui
                );
                p!(
                    rear_visibility,
                    0.0..=1.0,
                    0.35,
                    "Rear visibility",
                    "Dim the rear portions for clearer depth.",
                    ui
                );
                ui.checkbox(&mut s.baselines, "Resting orbit guides");
            });
            settings_group(ui, "Color", |ui| {
                s.palette.controls(ui);
                super::properties_combo("rings-color", 180.0, 3)
                    .selected_text(match s.color_by {
                        ColorBy::Ring => "Color by ring",
                        ColorBy::Amplitude => "Color by amplitude",
                        ColorBy::Position => "Color around ring",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut s.color_by, ColorBy::Ring, "Ring");
                        ui.selectable_value(&mut s.color_by, ColorBy::Amplitude, "Amplitude");
                        ui.selectable_value(&mut s.color_by, ColorBy::Position, "Position");
                    });
                p!(
                    contrast,
                    0.25..=4.0,
                    1.0,
                    "Color contrast",
                    "Shape the palette gradient.",
                    ui
                );
            });
            settings_group(ui, "Guides", |ui| {
                ui.checkbox(&mut s.guides, "Labels & navigation hints");
                ui.checkbox(&mut s.show_gizmo, "Navigation gizmo");
            });
        });
        settings_panel(ui, "Camera", |ui| {
            settings_group(ui, "View presets", |ui| {
                ui.horizontal_wrapped(|ui| {
                    for (name, axis, sign) in [
                        ("Front", Axis::Y, false),
                        ("Right", Axis::X, true),
                        ("Top", Axis::Z, true),
                        ("Back", Axis::Y, true),
                    ] {
                        if ui.button(name).clicked() {
                            (s.yaw, s.elevation) = axis.angles(sign);
                            s.roll = 0.0;
                        }
                    }
                });
            });
            settings_group(ui, "Transform", |ui| {
                for (key, name, angle, default) in [
                    ("yaw", "Rotation °", &mut s.yaw, -0.35_f32),
                    ("elevation", "Elevation °", &mut s.elevation, 0.35),
                    ("roll", "Roll °", &mut s.roll, 0.0),
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
                        *angle = degrees.to_radians();
                    }
                }
                p!(
                    zoom,
                    0.25..=8.0,
                    1.0,
                    "Zoom",
                    "Scroll over the scene to zoom.",
                    ui
                );
                ui.horizontal(|ui| {
                    if ui.button("Center view").clicked() {
                        s.pan = Vec2::ZERO;
                    }
                    if ui.button("Reset camera").clicked() {
                        s.auto_orbit = false;
                        s.yaw = -0.35;
                        s.elevation = 0.35;
                        s.roll = 0.0;
                        s.zoom = 1.0;
                        s.pan = Vec2::ZERO;
                    }
                });
            });
        });
    }
    fn update(&mut self, frame: &AnalysisFrame, now: Instant, live: bool) {
        let dt = self.last_draw.map_or(0.0, |last| {
            now.saturating_duration_since(last).as_secs_f32().min(0.1)
        });
        self.last_draw = Some(now);
        if self.epoch != Some(frame.capture_epoch) {
            self.waves = Default::default();
            self.epoch = Some(frame.capture_epoch);
        }
        let s = &self.settings;
        self.orbit_phase = (self.orbit_phase + dt * s.motion_speed.clamp(-90.0, 90.0).to_radians())
            .rem_euclid(TAU);
        self.spin_phase =
            (self.spin_phase + dt * s.spin_speed.clamp(-180.0, 180.0).to_radians()).rem_euclid(TAU);
        let channel = if s.stereo {
            WaveformChannel::Left
        } else {
            s.channel
        };
        let (window, _) = capture_window(
            frame,
            s.window_ms.clamp(10.0, 500.0),
            channel,
            s.trigger,
            s.rising,
            s.threshold,
        );
        let count = s.resolution.clamp(128, 2048);
        let alpha = if s.smoothing_ms <= 0.0 {
            1.0
        } else {
            -(-dt * 1000.0 / s.smoothing_ms.clamp(1.0, 500.0)).exp_m1()
        };
        for (lane, channel) in WaveformChannel::ALL.into_iter().enumerate() {
            let initial = self.waves[lane].len() != count;
            self.waves[lane].resize(count, 0.0);
            for i in 0..count {
                let a = window.start + i * window.len() / count;
                let b = (window.start + (i + 1) * window.len() / count)
                    .max(a + 1)
                    .min(window.end);
                let mut peak = 0.0_f32;
                if live {
                    for index in a..b {
                        let value = channel.sample(frame, index);
                        if value.is_finite() && value.abs() > peak.abs() {
                            peak = value;
                        }
                    }
                }
                let out = &mut self.waves[lane][i];
                *out += (peak - *out) * if initial { 1.0 } else { alpha };
            }
        }
    }
    fn wave(&self, ring: usize, fraction: f32, gain: f32) -> f32 {
        let s = &self.settings;
        let lane = if s.stereo {
            ring % 2
        } else {
            match s.channel {
                WaveformChannel::Left => 0,
                WaveformChannel::Right => 1,
                WaveformChannel::Mid => 2,
                WaveformChannel::Side => 3,
            }
        };
        let wave = &self.waves[lane];
        if wave.is_empty() {
            return 0.0;
        }
        let phase = (fraction * s.repeats.clamp(1, 8) as f32 + ring as f32 * s.phase_offset)
            .rem_euclid(1.0);
        let x = phase * (wave.len() - 1) as f32;
        let a = x as usize;
        let b = (a + 1).min(wave.len() - 1);
        let center = wave[a] + (wave[b] - wave[a]) * x.fract();
        let softened =
            (wave[a.saturating_sub(1)] + 2.0 * center + wave[(b + 1).min(wave.len() - 1)]) * 0.25;
        let value = center + (softened - center) * s.line_smoothing.clamp(0.0, 1.0);
        let seam = s.seam.clamp(0.0, 0.2);
        let blend = if seam > 0.0 {
            let t = (phase.min(1.0 - phase) / seam).clamp(0.0, 1.0);
            t * t * (3.0 - 2.0 * t)
        } else {
            1.0
        };
        (value * gain * blend).clamp(-1.0, 1.0)
    }
    fn point(&self, ring: usize, fraction: f32, value: f32) -> [f32; 3] {
        let s = &self.settings;
        let rings = s.rings.clamp(1, 12);
        let radial = (s.radius.clamp(0.2, 2.5)
            + ring as f32 * s.spacing.clamp(0.0, 0.4)
            + value * s.amplitude.clamp(0.0, 2.0))
        .max(0.05);
        let sign = if s.alternate && ring % 2 == 1 {
            -1.0
        } else {
            1.0
        };
        let theta = fraction * TAU + self.spin_phase * sign;
        let (sin, cos) = theta.sin_cos();
        let x = radial * cos;
        let z = radial * sin * (1.0 - s.ellipticity.clamp(0.0, 0.85));
        let mut y = value * s.sideways.clamp(0.0, 2.0);
        let mut z = z;
        if s.arrangement == Arrangement::Orbital {
            let (si, ci) = s.inclination.clamp(0.0, 90.0).to_radians().sin_cos();
            (y, z) = (y * ci - z * si, y * si + z * ci);
        } else if s.arrangement == Arrangement::Stacked && rings > 1 {
            y += (ring as f32 / (rings - 1) as f32 - 0.5) * s.stack_height.clamp(0.0, 4.0);
        }
        let azimuth = self.orbit_phase * sign
            + if s.arrangement == Arrangement::Orbital {
                ring as f32 / rings as f32 * s.fan.clamp(0.0, 360.0).to_radians()
            } else {
                0.0
            };
        let (sa, ca) = azimuth.sin_cos();
        [ca * x + sa * z, y, -sa * x + ca * z]
    }
    fn build_mesh(&self, rect: Rect, theme: &AppTheme) -> egui::Mesh {
        let s = &self.settings;
        let rings = s.rings.clamp(1, 12);
        let count = s.resolution.clamp(128, 2048);
        let extent = (s.radius.clamp(0.2, 2.5)
            + (rings - 1) as f32 * s.spacing.clamp(0.0, 0.4)
            + 0.65)
            .max(if s.arrangement == Arrangement::Stacked {
                s.stack_height.clamp(0.0, 4.0) * 0.5 + 0.5
            } else {
                0.0
            });
        let scale =
            rect.width().min(rect.height()).max(0.0) * 0.43 * s.zoom.clamp(0.25, 8.0) / extent;
        let center = rect.center() + s.pan * rect.size();
        let axes =
            trackball::orientation_axes(s.yaw, s.elevation, s.roll).map(|v| v.map(|x| x as f32));
        let project = |p: [f32; 3]| {
            let v: [f32; 3] =
                std::array::from_fn(|i| axes[0][i] * p[0] + axes[1][i] * p[1] + axes[2][i] * p[2]);
            (center + Vec2::new(v[0], v[1]) * scale, v[2])
        };
        let peak = self
            .waves
            .iter()
            .enumerate()
            .filter(|(i, _)| {
                if s.stereo {
                    *i < 2
                } else {
                    *i == s.channel as usize
                }
            })
            .flat_map(|(_, wave)| wave)
            .map(|v| v.abs())
            .fold(0.0_f32, f32::max);
        let gain = s.gain.clamp(0.1, 10.0)
            * if s.auto_scale {
                (0.95 / peak.max(0.001)).clamp(0.1, 100.0)
            } else {
                1.0
            };
        let mut segments = Vec::with_capacity(rings * count * if s.baselines { 2 } else { 1 });
        for ring in 0..rings {
            let mut vertices = Vec::with_capacity(count + 1);
            for i in 0..count {
                let u = i as f32 / count as f32;
                let value = self.wave(ring, u, gain);
                let (position, depth) = project(self.point(ring, u, value));
                let t = match s.color_by {
                    ColorBy::Ring => ring as f32 / rings.max(2) as f32,
                    ColorBy::Amplitude => value.abs(),
                    ColorBy::Position => u,
                };
                let color = s.palette.color_with_contrast(t, s.contrast, theme);
                let light = (1.0 - s.audio_light.clamp(0.0, 1.0)
                    + s.audio_light.clamp(0.0, 1.0) * value.abs().sqrt())
                    * s.brightness.clamp(0.0, 3.0);
                let alpha = s.opacity.clamp(0.0, 1.0)
                    * if depth < 0.0 {
                        s.rear_visibility.clamp(0.0, 1.0)
                    } else {
                        1.0
                    };
                let rgb = |v: u8| (v as f32 * light).clamp(0.0, 255.0) as u8;
                vertices.push(Vertex {
                    position,
                    depth,
                    color: Color32::from_rgba_unmultiplied(
                        rgb(color.r()),
                        rgb(color.g()),
                        rgb(color.b()),
                        (alpha * 255.0) as u8,
                    ),
                });
            }
            vertices.push(vertices[0]); // Exact closure, independent of phase and waveform discontinuity.
            for pair in vertices.windows(2) {
                segments.push((pair[0], pair[1], false));
            }
            if s.baselines {
                for i in 0..count {
                    let (a, ad) = project(self.point(ring, i as f32 / count as f32, 0.0));
                    let (b, bd) =
                        project(self.point(ring, ((i + 1) % count) as f32 / count as f32, 0.0));
                    let c = theme.muted.gamma_multiply(0.2);
                    segments.push((
                        Vertex {
                            position: a,
                            depth: ad,
                            color: c,
                        },
                        Vertex {
                            position: b,
                            depth: bd,
                            color: c,
                        },
                        true,
                    ));
                }
            }
        }
        segments.sort_unstable_by(|(a, b, _), (c, d, _)| {
            (a.depth + b.depth).total_cmp(&(c.depth + d.depth))
        });
        let mut mesh = egui::Mesh::default();
        for (a, b, guide) in segments {
            let width = if guide {
                0.5
            } else {
                s.thickness.clamp(0.5, 5.0)
            };
            if !guide && s.glow > 0.0 {
                mesh_segment(
                    &mut mesh,
                    a.position,
                    b.position,
                    a.color.gamma_multiply(s.glow.clamp(0.0, 1.0) * 0.18),
                    b.color.gamma_multiply(s.glow.clamp(0.0, 1.0) * 0.18),
                    width + 5.0 * s.glow,
                );
            }
            mesh_segment(&mut mesh, a.position, b.position, a.color, b.color, width);
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
        live: bool,
    ) {
        if !rect.is_positive() {
            return;
        }
        self.update(frame, now, live);
        let painter = ui.painter_at(rect);
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
        let response=ui.interact(rect,ui.id().with("rings-camera"),Sense::click_and_drag()).help_text("Drag to rotate freely; edge drag rolls. Right/Shift-drag pans, scroll zooms, double-click resets.");
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
                s.yaw = -0.35;
                s.elevation = 0.35;
                s.roll = 0.0;
                s.zoom = 1.0;
                s.pan = Vec2::ZERO;
            }
        }
        painter.set(scene, egui::Shape::mesh(self.build_mesh(plot, theme)));
        if self.settings.guides && rect.width() > 450.0 {
            label(
                &painter,
                rect.left_top() + Vec2::new(12.0, 10.0),
                "ORBITAL WAVEFORMS · SIGNED AUDIO AMPLITUDE",
                theme.muted,
            );
            label(
                &painter,
                rect.left_bottom() + Vec2::new(12.0, -18.0),
                "Drag: free rotate · Edge: roll · Right/Shift-drag: pan · Scroll: zoom",
                theme.muted,
            );
        }
    }
}

impl super::camera::Controlled for Rings {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn audio() -> AnalysisFrame {
        AnalysisFrame {
            sample_rate: 48_000,
            waveform_left: (0..4800)
                .map(|i| {
                    let t = i as f32 / 48_000.0;
                    0.42 * (TAU * 110.0 * t).sin() + 0.12 * (TAU * 330.0 * t).sin()
                })
                .collect(),
            waveform_right: (0..4800)
                .map(|i| {
                    let t = i as f32 / 48_000.0;
                    0.32 * (TAU * 165.0 * t + 0.8).sin()
                })
                .collect(),
            ..Default::default()
        }
    }
    fn rect() -> Rect {
        Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 900.0))
    }
    fn direct() -> Rings {
        let mut rings = Rings::default();
        rings.settings.trigger = false;
        rings.settings.smoothing_ms = 0.0;
        rings.settings.seam = 0.0;
        rings.settings.phase_offset = 0.0;
        rings
    }

    #[test]
    fn signed_stereo_mid_and_side_match_waveform_channels() {
        let frame = AnalysisFrame {
            waveform_left: vec![0.6; 4800].into(),
            waveform_right: vec![-0.2; 4800].into(),
            ..Default::default()
        };
        let mut rings = direct();
        rings.update(&frame, Instant::now(), true);
        assert!((rings.wave(0, 0.5, 1.0) - 0.6).abs() < 1e-6);
        assert!((rings.wave(1, 0.5, 1.0) + 0.2).abs() < 1e-6);
        rings.settings.stereo = false;
        for (channel, expected) in [
            (WaveformChannel::Left, 0.6),
            (WaveformChannel::Right, -0.2),
            (WaveformChannel::Mid, 0.2),
            (WaveformChannel::Side, 0.4),
        ] {
            rings.settings.channel = channel;
            assert!((rings.wave(0, 0.5, 1.0) - expected).abs() < 1e-6);
        }
        let outward = rings.point(0, 0.0, 0.6);
        let inward = rings.point(0, 0.0, -0.6);
        let length = |p: [f32; 3]| p.into_iter().map(|v| v * v).sum::<f32>().sqrt();
        assert!(length(outward) > length(inward));
    }

    #[test]
    fn resampling_keeps_short_signed_transients_and_rejects_invalid_audio() {
        let mut frame = AnalysisFrame {
            waveform_left: vec![0.0; 4800].into(),
            waveform_right: vec![0.0; 4800].into(),
            ..Default::default()
        };
        std::sync::Arc::make_mut(&mut frame.waveform_left)[2000] = -0.9;
        std::sync::Arc::make_mut(&mut frame.waveform_left)[2001] = f32::NAN;
        std::sync::Arc::make_mut(&mut frame.waveform_right)[2500] = 0.8;
        std::sync::Arc::make_mut(&mut frame.waveform_right)[2501] = f32::INFINITY;
        let mut rings = direct();
        rings.settings.resolution = 128;
        rings.update(&frame, Instant::now(), true);
        assert!(rings.waves[0].contains(&-0.9));
        assert!(rings.waves[1].contains(&0.8));
        assert!(rings.waves.iter().flatten().all(|v| v.is_finite()));
        frame.waveform_left = frame.waveform_left[..3].into();
        rings.update(&frame, Instant::now(), true);
        frame.waveform_right = Default::default();
        rings.update(&frame, Instant::now(), true);
        assert!(rings.waves.iter().flatten().all(|v| *v == 0.0));
    }

    #[test]
    fn response_is_time_based_freezes_and_clears_between_capture_epochs() {
        let now = Instant::now();
        let mut rings = direct();
        rings.settings.smoothing_ms = 100.0;
        let mut frame = AnalysisFrame::default();
        rings.update(&frame, now, true);
        std::sync::Arc::make_mut(&mut frame.waveform_left).fill(1.0);
        std::sync::Arc::make_mut(&mut frame.waveform_right).fill(1.0);
        rings.update(&frame, now + Duration::from_millis(100), true);
        assert!((rings.waves[0][32] - (1.0 - (-1.0_f32).exp())).abs() < 1e-6);
        let frozen = rings.waves.clone();
        let phase = (rings.orbit_phase, rings.spin_phase);
        rings.update(&frame, now + Duration::from_millis(100), true);
        assert_eq!(frozen, rings.waves);
        assert_eq!(phase, (rings.orbit_phase, rings.spin_phase));
        rings.update(&frame, now + Duration::from_millis(200), false);
        assert!(rings.waves[0][32] < frozen[0][32]);
        frame.capture_epoch += 1;
        rings.update(&frame, now + Duration::from_millis(210), false);
        assert!(rings.waves.iter().flatten().all(|v| *v == 0.0));
    }

    #[test]
    fn silence_has_circular_baselines_and_arrangements_have_distinct_geometry() {
        let mut rings = direct();
        rings.settings.inclination = 0.0;
        rings.settings.sideways = 0.0;
        for i in 0..128 {
            let p = rings.point(2, i as f32 / 128.0, 0.0);
            assert!((p[0].hypot(p[2]) - 1.2).abs() < 1e-5);
            assert_eq!(p[1], 0.0);
        }
        rings.settings.inclination = 65.0;
        let orbital = rings.point(1, 0.25, 0.0);
        rings.settings.arrangement = Arrangement::Concentric;
        let concentric = rings.point(1, 0.25, 0.0);
        assert_ne!(orbital, concentric);
        assert_eq!(concentric[1], 0.0);
        rings.settings.arrangement = Arrangement::Stacked;
        let bottom = rings.point(0, 0.0, 0.0);
        let top = rings.point(3, 0.0, 0.0);
        assert!((top[1] - bottom[1] - rings.settings.stack_height).abs() < 1e-6);
        rings.settings.rings = 1;
        assert_eq!(rings.point(0, 0.0, 0.0)[1], 0.0);
    }

    #[test]
    fn seam_blend_closes_each_repeated_waveform_smoothly() {
        let mut rings = direct();
        rings.waves = std::array::from_fn(|_| vec![1.0; 128]);
        rings.settings.seam = 0.04;
        rings.settings.repeats = 3;
        for u in [0.0, 1.0 / 3.0, 2.0 / 3.0, 1.0] {
            assert!(rings.wave(0, u, 1.0).abs() < 1e-5);
        }
        assert!(rings.wave(0, 0.05, 1.0) > 0.9);
        assert!((rings.wave(0, 0.0, 1.0) - rings.wave(0, 1.0, 1.0)).abs() < 1e-6);
    }

    #[test]
    fn every_arrangement_renders_live_audio_with_finite_mesh_at_maximum_detail() {
        let mut rings = direct();
        rings.settings.rings = 12;
        rings.settings.resolution = 2048;
        rings.settings.baselines = true;
        rings.settings.glow = 1.0;
        rings.settings.roll = 1.7;
        rings.update(&audio(), Instant::now(), true);
        let theme = AppTheme::default();
        for mode in [
            Arrangement::Orbital,
            Arrangement::Concentric,
            Arrangement::Stacked,
        ] {
            rings.settings.arrangement = mode;
            let mesh = rings.build_mesh(rect(), &theme);
            assert!(mesh.is_valid());
            assert!(!mesh.vertices.is_empty());
            assert!(
                mesh.vertices
                    .iter()
                    .all(|v| v.pos.x.is_finite() && v.pos.y.is_finite())
            );
        }
        rings.settings = RingsSettings::default();
        let live = rings.build_mesh(rect(), &theme);
        rings.clear();
        let silent = rings.build_mesh(rect(), &theme);
        assert_ne!(live.vertices, silent.vertices);
    }

    #[test]
    fn settings_roundtrip_keeps_arrangement_but_excludes_captured_audio() {
        let mut rings = direct();
        rings.settings.arrangement = Arrangement::Stacked;
        rings.settings.rings = 7;
        rings.settings.sideways = 1.2;
        rings.settings.roll = 1.6;
        rings.update(&audio(), Instant::now(), true);
        let encoded = serde_json::to_string(&rings.settings_snapshot()).unwrap();
        assert!(!encoded.contains("waves"));
        let settings: RingsSettings = serde_json::from_str(&encoded).unwrap();
        let mut restored = Rings::default();
        restored.apply_settings(&settings);
        assert_eq!(rings.settings_snapshot(), restored.settings_snapshot());
        assert!(restored.waves.iter().all(Vec::is_empty));
        restored.reset_settings();
        assert_eq!(restored.settings, RingsSettings::default());
    }

    #[test]
    #[ignore = "manual visual review: writes the actual renderer mesh to /tmp"]
    fn export_rings_mesh() {
        let mut rings = Rings::default();
        rings.update(&audio(), Instant::now(), true);
        let mesh = rings.build_mesh(rect(), &AppTheme::default());
        let value = serde_json::json!({
            "vertices": mesh.vertices.iter().map(|v| (v.pos.x,v.pos.y,v.color.to_srgba_unmultiplied())).collect::<Vec<_>>(),
            "indices": mesh.indices
        });
        std::fs::write(
            "/tmp/symphos-rings-mesh.json",
            serde_json::to_vec(&value).unwrap(),
        )
        .unwrap();
    }
}
