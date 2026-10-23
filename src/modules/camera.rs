//! Common camera motion. Renderers retain their lenses and coordinate units.
use super::{camera_gizmo, camera_motion, settings_group};
use crate::{help::HoverHelp, parameter::Parameter};
use egui::{Rect, Ui, Vec2};
use std::time::Instant;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Pose {
    pub yaw: f32,
    pub elevation: f32,
    pub roll: f32,
    pub zoom: f32,
    pub pan: Vec2,
}
impl Pose {
    fn close(self, other: Self) -> bool {
        (self.yaw - other.yaw).abs() < 1.0e-5
            && (self.elevation - other.elevation).abs() < 1.0e-5
            && (self.roll - other.roll).abs() < 1.0e-5
            && (self.zoom - other.zoom).abs() < 1.0e-5
            && (self.pan - other.pan).length() < 1.0e-5
    }
}

pub(super) struct Camera<'a> {
    pub yaw: &'a mut f32,
    pub elevation: &'a mut f32,
    pub roll: &'a mut f32,
    pub zoom: &'a mut f32,
    pub pan_x: &'a mut f32,
    pub pan_y: &'a mut f32,
    pub degrees: bool,
    pub y_up: bool,
    pub enabled: &'a mut bool,
    pub speed: &'a mut f32,
    pub speed_radians: bool,
    pub default_speed: f32,
    pub speed_limit: f32,
    pub speed_key: &'static str,
    pub reverse: Option<&'a mut bool>,
    pub motion: &'a mut camera_motion::Settings,
    pub gizmo: &'a mut bool,
}
impl Camera<'_> {
    fn raw(&self) -> [f32; 6] {
        [
            *self.yaw,
            *self.elevation,
            *self.roll,
            *self.zoom,
            *self.pan_x,
            *self.pan_y,
        ]
    }
    fn restore(&mut self, v: [f32; 6]) {
        *self.yaw = v[0];
        *self.elevation = v[1];
        *self.roll = v[2];
        *self.zoom = v[3];
        *self.pan_x = v[4];
        *self.pan_y = v[5];
    }
    fn pose(&self) -> Pose {
        let scale = if self.degrees {
            std::f32::consts::PI / 180.0
        } else {
            1.0
        };
        Pose {
            yaw: *self.yaw * scale,
            elevation: *self.elevation * scale,
            roll: *self.roll * scale,
            zoom: *self.zoom,
            pan: Vec2::new(*self.pan_x, *self.pan_y),
        }
    }
    fn apply(&mut self, pose: Pose) {
        let scale = if self.degrees {
            180.0 / std::f32::consts::PI
        } else {
            1.0
        };
        *self.yaw = pose.yaw * scale;
        *self.elevation = pose.elevation * scale;
        *self.roll = pose.roll * scale;
        *self.zoom = pose.zoom;
        *self.pan_x = pose.pan.x;
        *self.pan_y = pose.pan.y;
    }
}
pub(super) trait Controlled {
    fn camera(&mut self) -> Camera<'_>;
}

#[derive(Default)]
pub(super) struct Runtime {
    motion: camera_motion::Runtime,
    time: Option<Instant>,
    base: Option<Pose>,
}

pub(super) fn views(module: &mut (impl Controlled + ?Sized), ui: &mut Ui) {
    super::settings_panel(ui, "Camera", |ui| {
        let mut c = module.camera();
        settings_group(ui, "Views", |ui| {
            let width = ((ui.available_width() - 8.0) / 3.0).max(38.0);
            for row in [
                [
                    ("Front", camera_gizmo::Axis::Y, false),
                    ("Right", camera_gizmo::Axis::X, true),
                    ("Top", camera_gizmo::Axis::Z, true),
                ],
                [
                    ("Back", camera_gizmo::Axis::Y, true),
                    ("Left", camera_gizmo::Axis::X, false),
                    ("Bottom", camera_gizmo::Axis::Z, false),
                ],
            ] {
                ui.horizontal(|ui| {
                    for (name, mut axis, mut positive) in row {
                        if c.y_up {
                            (axis, positive) = match name {
                                "Front" => (camera_gizmo::Axis::Z, false),
                                "Back" => (camera_gizmo::Axis::Z, true),
                                "Top" => (camera_gizmo::Axis::Y, true),
                                "Bottom" => (camera_gizmo::Axis::Y, false),
                                _ => (axis, positive),
                            };
                        }
                        let angles = if c.y_up {
                            axis.angles_y_up(positive)
                        } else {
                            axis.angles(positive)
                        };
                        if camera_gizmo::view_button(
                            ui,
                            name,
                            axis,
                            positive,
                            width,
                            (c.pose().yaw - angles.0).abs() < 0.001
                                && (c.pose().elevation - angles.1).abs() < 0.001,
                        )
                        .clicked()
                        {
                            let mut pose = c.pose();
                            (pose.yaw, pose.elevation) = angles;
                            pose.roll = 0.0;
                            c.apply(pose);
                            *c.enabled = false;
                        }
                    }
                });
            }
        });
    });
}
pub(super) fn controls(module: &mut (impl Controlled + ?Sized), ui: &mut Ui) -> bool {
    let mut restart = false;
    super::settings_panel(ui, "Camera", |ui| controls_inner(module, ui, &mut restart));
    restart
}
fn controls_inner(module: &mut (impl Controlled + ?Sized), ui: &mut Ui, restart: &mut bool) {
    let c = module.camera();
    settings_group(ui, "Automatic motion", |ui| {
        ui.checkbox(c.enabled, "Enable auto-orbit").help_text("Animate the camera independently of the audio and simulation. Manual navigation stops at the visible view. Pausing this visualizer also pauses camera motion.");
        let mut degrees = if c.speed_radians {
            c.speed.to_degrees()
        } else {
            *c.speed
        };
        let default = if c.speed_radians {
            c.default_speed.to_degrees()
        } else {
            c.default_speed
        };
        let limit = if c.speed_radians {
            c.speed_limit.to_degrees()
        } else {
            c.speed_limit
        };
        if ui
            .add(
                Parameter::new(&mut degrees, -limit.min(90.0)..=limit.min(90.0), default)
                    .bounds(-f64::from(limit)..=f64::from(limit))
                    .key(c.speed_key)
                    .raw_scale(if c.speed_radians {
                        std::f64::consts::PI / 180.0
                    } else {
                        1.0
                    })
                    .text("Orbit °/s"),
            )
            .changed()
        {
            *c.speed = if c.speed_radians {
                degrees.to_radians()
            } else {
                degrees
            };
        }
        if let Some(reverse) = c.reverse {
            ui.checkbox(reverse, "Reverse orbit");
        } else {
            ui.checkbox(&mut c.motion.reverse, "Reverse orbit");
        }
        c.motion.orbit_controls_keyed(ui, "camera_motion");
        if ui.button("Restart motion").help_text("Restart the orbit, sweep, wobble and pan from the saved starting view. Keeps the recipe and simulation unchanged.").clicked() { *restart = true; }
    });
    settings_group(ui, "Wobble", |ui| {
        c.motion.wobble_controls_keyed(ui, "camera_motion")
    });
    settings_group(ui, "Auto-pan", |ui| {
        c.motion.pan_controls(ui, "camera_motion")
    });
    settings_group(ui, "Navigator", |ui| {
        ui.checkbox(c.gizmo, "Show axis navigator").help_text("Show the XYZ orientation compass. Click an axis to snap to that side; drag the compass to rotate, including roll.");
    });
}

/// Apply a transient camera pose for one draw, then restore the saved starting
/// view. User navigation is committed, and immediately cancels automatic motion.
pub(super) fn draw<M: Controlled + ?Sized>(
    module: &mut M,
    runtime: &mut Runtime,
    ui: &mut Ui,
    rect: Rect,
    now: Instant,
    shared_gizmo: bool,
    body: impl FnOnce(&mut M, &mut Ui),
) {
    let presentation = super::presenting(ui);
    let (base, raw, enabled, speed, reverse, recipe, gizmo, y_up) = {
        let c = module.camera();
        (
            c.pose(),
            c.raw(),
            *c.enabled,
            if c.speed_radians {
                c.speed.to_degrees()
            } else {
                *c.speed
            },
            c.reverse.as_deref().copied().unwrap_or(c.motion.reverse),
            if c.y_up {
                c.motion.y_up()
            } else {
                c.motion.clone()
            },
            *c.gizmo,
            c.y_up,
        )
    };
    let dt = runtime
        .time
        .map_or(0.0, |t| now.saturating_duration_since(t).as_secs_f32());
    runtime.time = Some(now);
    let mut shown = base;
    if enabled {
        runtime.motion.advance(
            (base.yaw, base.elevation),
            &recipe,
            speed.abs(),
            reverse ^ (speed < 0.0),
            dt,
        );
        (shown.yaw, shown.elevation) = runtime.motion.angles;
        shown.pan = base.pan + runtime.motion.pan;
    } else if runtime.motion.active {
        if runtime.base == Some(base) {
            (shown.yaw, shown.elevation) = runtime.motion.angles;
            shown.pan = base.pan + runtime.motion.pan;
        }
        runtime.motion = Default::default();
    }
    {
        let mut c = module.camera();
        c.apply(shown);
        if shared_gizmo || presentation {
            *c.gizmo = false;
        }
        *c.enabled = false; // prevent the renderer's legacy yaw-only animator
    }
    let navigator = gizmo_rect(rect);
    if shared_gizmo && gizmo && !presentation {
        ui.data_mut(|d| d.insert_temp(egui::Id::new("camera-navigator-bounds"), navigator));
    }
    body(module, ui);
    let mut after = module.camera().pose();
    let mut navigated = !after.close(shown);
    if shared_gizmo && gizmo && !presentation {
        let action = camera_gizmo::draw_in_space(
            ui,
            navigator,
            after.yaw,
            after.elevation,
            after.roll,
            true,
            y_up,
        );
        if action.orbit != Vec2::ZERO {
            let delta = ui.input(|i| i.pointer.delta());
            if let Some(p) = ui.input(|i| i.pointer.interact_pos()) {
                super::trackball::drag(
                    &mut after.yaw,
                    &mut after.elevation,
                    &mut after.roll,
                    p - delta,
                    p,
                    navigator.center(),
                    navigator.width() * 0.42,
                );
                navigated = true;
            }
        }
        if let Some((axis, positive)) = action.snap {
            (after.yaw, after.elevation) = if y_up {
                axis.snap_y_up(positive, after.yaw, after.elevation)
            } else {
                axis.snap(positive, after.yaw, after.elevation)
            };
            after.roll = 0.0;
            navigated = true;
        }
    }
    if shared_gizmo && gizmo && !presentation {
        ui.data_mut(|d| d.remove::<Rect>(egui::Id::new("camera-navigator-bounds")));
    }
    let mut c = module.camera();
    if shared_gizmo || presentation {
        *c.gizmo = gizmo;
    }
    if navigated || !enabled {
        if navigated || !shown.close(base) {
            c.apply(after);
        } else {
            c.restore(raw);
        }
        *c.enabled = false;
        runtime.motion = Default::default();
        runtime.base = Some(after);
    } else {
        c.restore(raw);
        *c.enabled = true;
        runtime.base = Some(base);
    }
}

pub(super) fn over_navigator(ui: &Ui) -> bool {
    ui.data(|d| d.get_temp::<Rect>(egui::Id::new("camera-navigator-bounds")))
        .is_some_and(|rect| ui.input(|i| i.pointer.hover_pos().is_some_and(|p| rect.contains(p))))
}
fn gizmo_rect(rect: Rect) -> Rect {
    Rect::from_min_size(rect.right_top() + Vec2::new(-74.0, 8.0), Vec2::splat(66.0))
}

pub(super) fn grid(ui: &Ui, rect: Rect, project: impl Fn([f32; 3]) -> egui::Pos2) {
    let painter = ui.painter_at(rect);
    let color = ui.visuals().text_color().gamma_multiply(0.14);
    for i in -8..=8 {
        let v = i as f32 * 0.25;
        for (a, b) in [
            ([-2.0, 0.0, v], [2.0, 0.0, v]),
            ([v, 0.0, -2.0], [v, 0.0, 2.0]),
        ] {
            painter.line_segment([project(a), project(b)], egui::Stroke::new(0.7, color));
        }
    }
    for (axis, label, color) in [
        (0, "X", egui::Color32::from_rgb(223, 105, 111)),
        (2, "Y", egui::Color32::from_rgb(123, 192, 110)),
        (1, "Z", egui::Color32::from_rgb(104, 160, 236)),
    ] {
        let mut end = [0.0; 3];
        end[axis] = if axis == 2 { -2.0 } else { 2.0 };
        painter.line_segment(
            [project([0.0; 3]), project(end)],
            egui::Stroke::new(1.0, color),
        );
        painter.text(
            project(end),
            egui::Align2::LEFT_BOTTOM,
            label,
            egui::FontId::proportional(10.0),
            color,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::{ModuleKind, ModulePane};
    use std::time::Duration;

    #[test]
    #[ignore = "manual camera coordination timing, excluding rendering"]
    fn camera_coordination_timing() {
        let ctx = egui::Context::default();
        let mut pane = ModulePane::new(ModuleKind::Flux);
        let module = pane.controlled_camera().unwrap();
        *module.camera().enabled = true;
        module.camera().motion.wobble = true;
        module.camera().motion.pan = true;
        let mut runtime = Runtime::default();
        let now = Instant::now();
        let count = 50000;
        let mut out = ctx.run_ui(Default::default(), |ui| {
            let rect = ui.max_rect();
            let start = Instant::now();
            for _ in 0..count { std::hint::black_box(module.camera().pose()); }
            let baseline = start.elapsed().as_secs_f64()/f64::from(count);
            let start = Instant::now();
            for i in 0..count {
                draw(module,&mut runtime,ui,rect,now+Duration::from_secs_f64(f64::from(i)/120.0),false,|m,_| { std::hint::black_box(m.camera().pose()); });
            }
            let animated = start.elapsed().as_secs_f64()/f64::from(count);
            eprintln!("Camera pose baseline {:.3} µs; animated coordination {:.3} µs; added {:.3} µs per visible view (rendering excluded)",baseline*1e6,animated*1e6,(animated-baseline)*1e6);
        });
        out.textures_delta.clear();
    }

    #[test]
    fn every_camera_preserves_saved_parameters_and_motion_is_independent_of_frame_rate() {
        for kind in ModuleKind::ALL
            .into_iter()
            .filter(|k| !matches!(k, ModuleKind::Waterfall))
        {
            let mut results = Vec::new();
            for fps in [30, 60, 120] {
                let mut pane = ModulePane::new(kind);
                let Some(module) = pane.controlled_camera() else {
                    continue;
                };
                {
                    let c = module.camera();
                    *c.enabled = true;
                    *c.speed = if c.speed_radians { 0.5 } else { 20.0 };
                    c.motion.axis = camera_motion::Axis::Frequency;
                    c.motion.wobble = true;
                    c.motion.pan = true;
                }
                let raw = module.camera().raw();
                let mut runtime = Runtime::default();
                let ctx = egui::Context::default();
                let now = Instant::now();
                let mut shown = module.camera().pose();
                for step in 0..=fps {
                    let mut out = ctx.run_ui(Default::default(), |ui| {
                        draw(
                            module,
                            &mut runtime,
                            ui,
                            Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(600.0, 400.0)),
                            now + Duration::from_secs_f64(step as f64 / fps as f64),
                            false,
                            |m, _| shown = m.camera().pose(),
                        );
                    });
                    out.textures_delta.clear();
                    assert_eq!(
                        module.camera().raw(),
                        raw,
                        "{kind:?}: animation must not edit parameters"
                    );
                    assert!(*module.camera().enabled);
                }
                assert!(
                    shown.pan != Vec2::new(raw[4], raw[5]),
                    "{kind:?}: auto-pan must affect the rendered framing"
                );
                results.push(shown);
                // Repeating a frozen timestamp holds all motion, including pan.
                let paused = shown;
                let mut out = ctx.run_ui(Default::default(), |ui| {
                    draw(
                        module,
                        &mut runtime,
                        ui,
                        ui.max_rect(),
                        now + Duration::from_secs(1),
                        false,
                        |m, _| shown = m.camera().pose(),
                    );
                });
                out.textures_delta.clear();
                assert!(shown.close(paused), "{kind:?}: pause must freeze the view");
                // Turning animation off retains the last displayed view.
                *module.camera().enabled = false;
                let mut out = ctx.run_ui(Default::default(), |ui| {
                    draw(
                        module,
                        &mut runtime,
                        ui,
                        ui.max_rect(),
                        now + Duration::from_secs(1),
                        false,
                        |_, _| {},
                    );
                });
                out.textures_delta.clear();
                assert!(
                    module.camera().pose().close(paused),
                    "{kind:?}: stopping must not jump"
                );
                // A manual drag commits its new view and cancels animation.
                *module.camera().enabled = true;
                let mut expected = module.camera().pose();
                let mut out = ctx.run_ui(Default::default(), |ui| {
                    draw(
                        module,
                        &mut runtime,
                        ui,
                        ui.max_rect(),
                        now + Duration::from_secs(2),
                        false,
                        |m, _| {
                            let mut c = m.camera();
                            expected = c.pose();
                            expected.yaw += 0.1;
                            expected.pan.x += 0.2;
                            c.apply(expected);
                        },
                    );
                });
                out.textures_delta.clear();
                assert!(!*module.camera().enabled);
                assert!(module.camera().pose().close(expected));
            }
            for result in results.iter().skip(1) {
                assert!(
                    result.close(results[0]),
                    "{kind:?}: FPS must not change motion"
                );
            }
        }
    }
}
