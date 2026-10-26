//! Camera animation is presentation state; only its recipe and starting view are saved.
use super::property_row;
fn wrap_angle(v: f32) -> f32 {
    (v + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}
use crate::{help::HoverHelp, parameter::Parameter};
use egui::{Ui, Vec2};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum Axis {
    Frequency,
    Time,
    #[default]
    Level,
    Custom,
}
impl Axis {
    fn label(self) -> &'static str {
        match self {
            Self::Frequency => "X",
            Self::Time => "Y",
            Self::Level => "Z",
            Self::Custom => "Custom axis",
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum Path {
    #[default]
    Orbit,
    Swing,
}
impl Path {
    fn label(self) -> &'static str {
        match self {
            Self::Orbit => "Continuous",
            Self::Swing => "Back and forth",
        }
    }
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub(crate) struct Settings {
    pub reverse: bool,
    pub pan: bool,
    pub pan_horizontal: f32,
    pub pan_vertical: f32,
    pub pan_hz: f32,
    pub axis: Axis,
    pub custom_x: f32,
    pub custom_y: f32,
    pub custom_z: f32,
    pub path: Path,
    pub swing_degrees: f32,
    pub wobble: bool,
    pub wobble_horizontal: f32,
    pub wobble_vertical: f32,
    pub wobble_hz: f32,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            reverse: false,
            pan: false,
            pan_horizontal: 0.1,
            pan_vertical: 0.05,
            pan_hz: 0.05,
            axis: Axis::Level,
            custom_x: 0.0,
            custom_y: 0.0,
            custom_z: 1.0,
            path: Path::Orbit,
            swing_degrees: 45.0,
            wobble: false,
            wobble_horizontal: 8.0,
            wobble_vertical: 5.0,
            wobble_hz: 0.15,
        }
    }
}
impl Settings {
    pub(crate) fn pan_controls(&mut self, ui: &mut Ui, prefix: &str) {
        ui.push_id((prefix, "pan-enabled"), |ui| {
            ui.checkbox(&mut self.pan, "Enable auto-pan").help_text("Move the framing smoothly around the saved center, independently of auto-orbit and wobble. This moves the view only; particle forces, audio and object positions stay unchanged.");
        });
        ui.add_enabled_ui(self.pan, |ui| {
            for (name, value, field, default) in [("Horizontal ±", &mut self.pan_horizontal, "pan_horizontal", 0.1), ("Vertical ±", &mut self.pan_vertical, "pan_vertical", 0.05)] {
                ui.add(Parameter::new(value, 0.0..=0.5, default).bounds(0.0..=1.0).key(format!("{prefix}.{field}")).text(name)).help_text("Maximum travel from the saved framing, as a fraction of the viewport. 0.1 is ten percent of the viewport size.");
            }
            ui.add(Parameter::new(&mut self.pan_hz, 0.01..=1.0, 0.05).bounds(0.0..=2.0).key(format!("{prefix}.pan_hz")).text("Pan rate Hz")).help_text("Framing cycles per second. 0.05 takes twenty seconds for a gentle looping pan. Zero holds the current phase.");
        });
    }
    pub(crate) fn y_up_default() -> Self {
        Self {
            axis: Axis::Time,
            ..Self::default()
        }
    }
    pub(crate) fn y_up(&self) -> Self {
        let mut s = self.clone();
        match self.axis {
            Axis::Frequency => {}
            Axis::Time => s.axis = Axis::Level,
            Axis::Level => {
                s.axis = Axis::Custom;
                s.custom_x = 0.0;
                s.custom_y = -1.0;
                s.custom_z = 0.0;
            }
            Axis::Custom => {
                s.custom_x = self.custom_x;
                s.custom_y = -self.custom_z;
                s.custom_z = self.custom_y;
            }
        }
        s
    }
    fn axis_vector(&self) -> [f32; 3] {
        let axis = match self.axis {
            Axis::Frequency => [1.0, 0.0, 0.0],
            Axis::Time => [0.0, 0.0, -1.0],
            Axis::Level => [0.0, 1.0, 0.0],
            // Public Z-up coordinates map to the renderer's Y-up convention.
            Axis::Custom => [self.custom_x, self.custom_z, -self.custom_y],
        };
        let length = axis.iter().map(|v| v * v).sum::<f32>().sqrt();
        if length > 1.0e-6 {
            axis.map(|v| v / length)
        } else {
            [0.0, 1.0, 0.0]
        }
    }

    pub(crate) fn orbit_controls(&mut self, ui: &mut Ui) {
        self.orbit_controls_keyed(ui, "motion");
    }
    pub(crate) fn orbit_controls_keyed(&mut self, ui: &mut Ui, prefix: &str) {
        property_row(ui, "Axis", |ui| {
            crate::dropdown::ComboBox::from_id_salt("orbit-axis").selected_text(self.axis.label())
                .width(ui.available_width()).show_ui(ui, |ui| {
                    for axis in [Axis::Frequency, Axis::Time, Axis::Level, Axis::Custom] {
                        ui.selectable_value(&mut self.axis, axis, axis.label());
                    }
                }).response.help_text("Choose a fixed world axis through the visualizer’s center. X runs across the view, Y follows depth, and Z is up. Custom combines the three axes for a diagonal orbit.");
        });
        if self.axis == Axis::Custom {
            for (name, value, key, default) in [
                ("Axis X", &mut self.custom_x, "motion.custom_x", 0.0),
                ("Axis Y", &mut self.custom_y, "motion.custom_y", 0.0),
                ("Axis Z", &mut self.custom_z, "motion.custom_z", 1.0),
            ] {
                ui.add(Parameter::new(value, -1.0..=1.0, default).key(key.replace("motion", prefix)).text(name))
                    .help_text("Direction of the custom rotation axis. These values are normalized together: X=1, Y=0, Z=1 gives a diagonal between frequency and level. Negative values reverse an axis. All zeros fall back to Z.");
            }
        }
        property_row(ui, "Motion", |ui| {
            crate::dropdown::ComboBox::from_id_salt("orbit-path").selected_text(self.path.label())
                .width(ui.available_width()).show_ui(ui, |ui| {
                    for path in [Path::Orbit, Path::Swing] {
                        ui.selectable_value(&mut self.path, path, path.label());
                    }
                }).response.help_text("Continuous keeps circling the chosen axis. Back and forth sweeps either side of the starting view and slows gently to a stop at each end before returning.");
        });
        if self.path == Path::Swing {
            ui.add(Parameter::new(&mut self.swing_degrees, 1.0..=180.0, 45.0)
                .key(format!("{prefix}.swing_degrees")).text("Sweep ±°"))
                .help_text("Maximum angle on either side of the starting view. 45° makes a 90° total sweep. Speed is fastest at the center and eases to zero at either end; a wider sweep takes longer at the same speed.");
        }
    }

    pub(crate) fn wobble_controls(&mut self, ui: &mut Ui) {
        self.wobble_controls_keyed(ui, "motion");
    }
    pub(crate) fn wobble_controls_keyed(&mut self, ui: &mut Ui, prefix: &str) {
        ui.push_id((prefix, "wobble-enabled"), |ui| {
            ui.checkbox(&mut self.wobble, "Enable wobble")
                .help_text("Add a smooth figure-eight camera drift. Runs independently with auto-orbit off, or combines with an enabled orbit. Wobble moves the view only; audio and geometry are untouched.");
        });
        ui.add_enabled_ui(self.wobble, |ui| {
            ui.add(Parameter::new(&mut self.wobble_horizontal, 0.0..=90.0, 8.0)
                .key(format!("{prefix}.wobble_horizontal")).text("Horizontal ±°"))
                .help_text("How far the camera sways left and right from its orbit. Zero removes horizontal sway. Large values create a more dramatic rocking view.");
            ui.add(Parameter::new(&mut self.wobble_vertical, 0.0..=90.0, 5.0)
                .key(format!("{prefix}.wobble_vertical")).text("Vertical ±°"))
                .help_text("How far the camera nods up and down from its orbit. The vertical motion runs twice per horizontal cycle to trace a smooth figure eight. Zero keeps the orbit's elevation.");
            ui.add(Parameter::new(&mut self.wobble_hz, 0.01..=2.0, 0.15)
                .key(format!("{prefix}.wobble_hz")).bounds(0.0..=5.0).text("Rate Hz"))
                .help_text("Horizontal wobble cycles per second. 0.15 Hz takes about 6.7 seconds for a cycle; 1 Hz takes one second. Vertical nodding runs at twice this rate. Zero holds the wobble at its current phase.");
        });
    }
}

#[derive(Default)]
pub(crate) struct Runtime {
    pub active: bool,
    pub angles: (f32, f32),
    pub pan: Vec2,
    orbit_angles: (f32, f32),
    phase: f64,
    wobble_phase: f64,
    pan_phase: f64,
}
impl Runtime {
    pub(crate) fn advance(
        &mut self,
        base: (f32, f32),
        settings: &Settings,
        speed: f32,
        reverse: bool,
        dt: f32,
        orbit: bool,
    ) {
        let dt = if self.active {
            f64::from(dt.clamp(0.0, 0.1))
        } else {
            self.active = true;
            self.orbit_angles = base;
            0.0
        };
        // Orbit off holds the last orbit pose. Editing its axis, sweep or speed
        // must not move a camera that is only wobbling or panning.
        if orbit {
            let amplitude = f64::from(settings.swing_degrees.clamp(1.0, 180.0).to_radians());
            let rate =
                f64::from(speed.clamp(0.0, 360.0)).to_radians() * if reverse { -1.0 } else { 1.0 };
            self.phase = (self.phase
                + dt * rate
                    / if settings.path == Path::Swing {
                        amplitude
                    } else {
                        1.0
                    })
            .rem_euclid(std::f64::consts::TAU);
            let angle = if settings.path == Path::Swing {
                amplitude * self.phase.sin()
            } else {
                self.phase
            } as f32;
            if settings.axis == Axis::Level {
                self.orbit_angles = (wrap_angle(base.0 + angle), base.1);
            } else {
                let direction = direction(base);
                let axis = settings.axis_vector();
                let (s, c) = (-angle).sin_cos();
                let dot = direction.iter().zip(axis).map(|(v, a)| v * a).sum::<f32>();
                let cross = [
                    axis[1] * direction[2] - axis[2] * direction[1],
                    axis[2] * direction[0] - axis[0] * direction[2],
                    axis[0] * direction[1] - axis[1] * direction[0],
                ];
                let rotated = std::array::from_fn(|i| {
                    direction[i] * c + cross[i] * s + axis[i] * dot * (1.0 - c)
                });
                self.orbit_angles = angles(rotated, self.orbit_angles);
            }
        }
        // Independent phases make speed/reverse edits continuous, and f64
        // accumulators avoid the drift that appears at high display rates.
        if settings.wobble {
            self.wobble_phase = (self.wobble_phase
                + dt * f64::from(settings.wobble_hz.clamp(0.0, 5.0)) * std::f64::consts::TAU)
                .rem_euclid(std::f64::consts::TAU);
        }
        let wobble = if settings.wobble {
            Vec2::new(
                settings.wobble_horizontal.clamp(0.0, 90.0).to_radians()
                    * self.wobble_phase.sin() as f32,
                settings.wobble_vertical.clamp(0.0, 90.0).to_radians()
                    * (2.0 * self.wobble_phase).sin() as f32,
            )
        } else {
            Vec2::ZERO
        };
        self.angles = (
            wrap_angle(self.orbit_angles.0 + wobble.x),
            wrap_angle(self.orbit_angles.1 + wobble.y),
        );
        if settings.pan {
            self.pan_phase = (self.pan_phase
                + dt * f64::from(settings.pan_hz.clamp(0.0, 2.0)) * std::f64::consts::TAU)
                .rem_euclid(std::f64::consts::TAU);
        }
        self.pan = if settings.pan {
            Vec2::new(
                settings.pan_horizontal.clamp(0.0, 1.0) * self.pan_phase.sin() as f32,
                settings.pan_vertical.clamp(0.0, 1.0) * (2.0 * self.pan_phase).sin() as f32,
            )
        } else {
            Vec2::ZERO
        };
    }
}
fn direction((yaw, elevation): (f32, f32)) -> [f32; 3] {
    [
        -yaw.sin() * elevation.cos(),
        elevation.sin(),
        yaw.cos() * elevation.cos(),
    ]
}
fn angles([x, y, z]: [f32; 3], previous: (f32, f32)) -> (f32, f32) {
    let elevation = y.clamp(-1.0, 1.0).asin();
    let yaw = if x.hypot(z) < 1.0e-5 {
        previous.0
    } else {
        (-x).atan2(z)
    };
    let a = (wrap_angle(yaw), elevation);
    let b = (
        wrap_angle(yaw + std::f32::consts::PI),
        wrap_angle(std::f32::consts::PI - elevation),
    );
    let distance =
        |v: (f32, f32)| wrap_angle(v.0 - previous.0).powi(2) + wrap_angle(v.1 - previous.1).powi(2);
    if distance(a) <= distance(b) { a } else { b }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_effect_clocks_do_not_follow_orbit_changes() {
        let mut runtime = Runtime::default();
        let mut settings = Settings {
            wobble: true,
            pan: true,
            ..Default::default()
        };
        runtime.advance((0.0, 0.3), &settings, 30.0, false, 0.0, true);
        runtime.advance((0.0, 0.3), &settings, 30.0, false, 0.1, true);
        let wobble = runtime.wobble_phase;
        let pan = runtime.pan_phase;
        let orbit = runtime.phase;
        settings.wobble = false;
        settings.pan = false;
        for _ in 0..10 {
            runtime.advance((0.0, 0.3), &settings, 90.0, true, 0.1, true);
        }
        assert_eq!(runtime.wobble_phase, wobble);
        assert_eq!(runtime.pan_phase, pan);
        assert_ne!(runtime.phase, orbit);
    }

    #[test]
    fn editing_disabled_orbit_settings_does_not_move_the_wobble_view() {
        let base = (-0.35, 0.65);
        let mut settings = Settings {
            wobble: true,
            ..Default::default()
        };
        let mut runtime = Runtime::default();
        runtime.advance(base, &settings, 30.0, false, 0.0, true);
        runtime.advance(base, &settings, 30.0, false, 0.1, true);
        let pose = runtime.angles;
        let phase = runtime.phase;
        settings.axis = Axis::Frequency;
        settings.path = Path::Swing;
        settings.swing_degrees = 120.0;
        runtime.advance(base, &settings, 200.0, true, 0.0, false);
        assert_eq!(runtime.angles, pose);
        assert_eq!(runtime.phase, phase);
        runtime.advance(base, &settings, 200.0, true, 0.1, false);
        assert_ne!(runtime.angles, pose, "wobble keeps moving");
        assert_eq!(runtime.phase, phase, "orbit remains paused");
    }

    #[test]
    fn every_axis_is_a_fixed_world_axis_and_motion_matches_at_all_frame_rates() {
        let base = (-0.35, 0.65);
        for axis in [Axis::Level, Axis::Frequency, Axis::Time, Axis::Custom] {
            let settings = Settings {
                axis,
                custom_x: 1.0,
                custom_y: -0.5,
                custom_z: 0.7,
                ..Default::default()
            };
            let axis = settings.axis_vector();
            let dot =
                |direction: [f32; 3]| direction.iter().zip(axis).map(|(v, a)| v * a).sum::<f32>();
            let initial = dot(direction(base));
            let mut poses = Vec::new();
            for fps in [15, 30, 60, 120] {
                let mut runtime = Runtime::default();
                runtime.advance(base, &settings, 60.0, false, 0.0, true);
                let mut previous = runtime.angles;
                for _ in 0..fps * 12 {
                    runtime.advance(base, &settings, 60.0, false, 1.0 / fps as f32, true);
                    assert!((dot(direction(runtime.angles)) - initial).abs() < 1.0e-4);
                    assert!(wrap_angle(runtime.angles.0 - previous.0).abs() < 0.6);
                    assert!(wrap_angle(runtime.angles.1 - previous.1).abs() < 0.6);
                    previous = runtime.angles;
                }
                poses.push(direction(runtime.angles));
            }
            for pose in &poses[1..] {
                assert!(
                    pose.iter()
                        .zip(poses[0])
                        .all(|(a, b)| (a - b).abs() < 1.0e-4)
                );
            }
        }
    }

    #[test]
    fn swing_is_bounded_eases_at_ends_and_reverse_retraces_the_same_path() {
        let settings = Settings {
            path: Path::Swing,
            swing_degrees: 30.0,
            ..Default::default()
        };
        let mut runtime = Runtime::default();
        let base = (0.0, 0.4);
        runtime.advance(base, &settings, 60.0, false, 0.0, true);
        for _ in 0..60 {
            runtime.advance(base, &settings, 60.0, false, 1.0 / 120.0, true);
        }
        let before = runtime.angles;
        let start_phase = runtime.phase;
        for _ in 0..60 {
            runtime.advance(base, &settings, 60.0, true, 1.0 / 120.0, true);
        }
        assert!(runtime.angles.0.abs() < 1.0e-5);
        assert!(before.0 > 0.4);
        runtime.phase = start_phase;
        let mut max = 0.0_f32;
        let mut min = 0.0_f32;
        for _ in 0..120 * 10 {
            runtime.advance(base, &settings, 60.0, false, 1.0 / 120.0, true);
            max = max.max(runtime.angles.0);
            min = min.min(runtime.angles.0);
        }
        assert!((max - 30.0_f32.to_radians()).abs() < 1.0e-4);
        assert!((min + 30.0_f32.to_radians()).abs() < 1.0e-4);
        runtime.phase = std::f64::consts::FRAC_PI_2;
        runtime.advance(base, &settings, 0.0, false, 0.0, true);
        let end = runtime.angles.0;
        runtime.advance(base, &settings, 60.0, false, 1.0 / 120.0, true);
        assert!(
            (runtime.angles.0 - end).abs() < 0.0001,
            "sweep eases at its end"
        );
    }

    #[test]
    fn wobble_works_without_rotation_is_bounded_and_speed_changes_do_not_jump() {
        let base = (-0.35, 0.65);
        let settings = Settings {
            wobble: true,
            wobble_hz: 0.25,
            ..Default::default()
        };
        let mut runtime = Runtime::default();
        runtime.advance(base, &settings, 0.0, false, 0.0, true);
        for _ in 0..120 {
            runtime.advance(base, &settings, 0.0, false, 1.0 / 120.0, true);
            assert!(
                wrap_angle(runtime.angles.0 - base.0).abs()
                    <= settings.wobble_horizontal.to_radians() + 1.0e-5
            );
            assert!(
                wrap_angle(runtime.angles.1 - base.1).abs()
                    <= settings.wobble_vertical.to_radians() + 1.0e-5
            );
        }
        assert!((wrap_angle(runtime.angles.0 - base.0) - 8.0_f32.to_radians()).abs() < 1.0e-4);
        let previous = runtime.angles;
        runtime.advance(base, &settings, 240.0, true, 0.0, true);
        assert_eq!(runtime.angles, previous, "a speed edit preserves phase");
        let zero = Settings {
            axis: Axis::Custom,
            custom_x: 0.0,
            custom_y: 0.0,
            custom_z: 0.0,
            ..Default::default()
        };
        runtime.advance(base, &zero, 1.0, false, 0.01, true);
        assert!(runtime.angles.0.is_finite() && runtime.angles.1.is_finite());
    }

    #[test]
    fn crossing_poles_retains_a_continuous_camera_basis() {
        let settings = Settings {
            axis: Axis::Frequency,
            ..Default::default()
        };
        let base = (0.0, 0.0);
        let mut runtime = Runtime::default();
        runtime.advance(base, &settings, 90.0, false, 0.0, true);
        for _ in 0..120 * 8 {
            let previous = runtime.angles;
            runtime.advance(base, &settings, 90.0, false, 1.0 / 120.0, true);
            assert!(wrap_angle(runtime.angles.0 - previous.0).abs() < 0.03);
            assert!(wrap_angle(runtime.angles.1 - previous.1).abs() < 0.03);
        }
    }
}
