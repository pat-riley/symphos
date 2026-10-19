use super::{OrbitalSettings, choice, property_row, trackball};
use crate::{help::HoverHelp, theme::AppTheme};
use egui::{Color32, Pos2, Rect, Sense, Stroke, Vec2};
use std::f32::consts::{FRAC_PI_2, PI};

pub(super) fn reset(s: &mut OrbitalSettings) {
    s.yaw = -0.3;
    s.elevation = 0.45;
    s.roll = 0.0;
    s.zoom = 1.0;
    s.pan_x = 0.0;
    s.pan_y = 0.0;
    s.camera_distance = 8.5;
    s.field_of_view = 53.1301;
    s.auto_orbit = false;
}
fn snap(s: &mut OrbitalSettings, axis: usize, positive: bool) {
    (s.yaw, s.elevation) = match (axis, positive) {
        (0, true) => (FRAC_PI_2, 0.0),
        (0, false) => (-FRAC_PI_2, 0.0),
        (1, true) => (0.0, -FRAC_PI_2),
        (1, false) => (0.0, FRAC_PI_2),
        (2, true) => (PI, 0.0),
        _ => (0.0, 0.0),
    };
    s.roll = 0.0;
    s.auto_orbit = false;
}
pub(super) fn basic_controls(ui: &mut egui::Ui, s: &mut OrbitalSettings) {
    choice(
        ui,
        "Projection",
        &mut s.projection,
        &["Perspective", "Orthographic"],
        "Perspective makes nearby objects larger and distant objects smaller. Orthographic keeps size constant with depth, making it easier to compare XYZ positions and trace paths.",
    );
    egui::Grid::new("orbital-views").num_columns(3).show(ui,|ui| {
            for (i,(name,axis,positive)) in [("Front",2,false),("Back",2,true),("Top",1,true),("Bottom",1,false),("Left",0,false),("Right",0,true)].into_iter().enumerate() {
                if ui.button(name).help_text(format!("Look straight along a world axis from the {name} side. Keeps zoom and framing, removes roll and stops automatic orbit. Use Orthographic for a flat technical view.")).clicked() {snap(s,axis,positive);}
                if i%3==2 {ui.end_row();}
            }
        });
    ui.horizontal(|ui| {
            if ui.button("Center view").help_text("Move the world origin back to the center of the viewport. Rotation, lens and zoom stay unchanged.").clicked() {s.pan_x=0.0;s.pan_y=0.0;}
            if ui.button("Reset camera").help_text("Restore the original camera angle, distance, lens, zoom and centered framing. Leaves particle behavior and appearance unchanged.").clicked() {reset(s);}
        });
    property_row(ui, "Navigation gestures", |ui| {
        ui.label("Drag: rotate · Shift-drag: pan").help_text("Drag the canvas to rotate freely. Right-drag, middle-drag or Shift-drag pans. Scroll zooms. Double-click restores the camera. The upper-right navigator supports axis snapping and free rotation.");
    });
}
pub(super) fn advanced_controls(ui: &mut egui::Ui, s: &mut OrbitalSettings) {
    ui.checkbox(&mut s.auto_orbit,"Auto-orbit").help_text("Rotate the camera around Y at Camera orbit speed. Dragging or choosing a view stops it; particle motion stays independent.");
}
pub(super) fn guide_controls(ui: &mut egui::Ui, s: &mut OrbitalSettings) {
    ui.checkbox(&mut s.show_grid,"Show reference grid").help_text("Draw a world-space ruler through the origin. Grid lines are one world unit apart. It follows the same camera as the particles and is only a guide, not a collision surface. Guides are overlaid and do not hide behind particles.");
    choice(
        ui,
        "Grid planes",
        &mut s.grid_plane,
        &[
            "XZ · horizontal",
            "XY · front",
            "YZ · side",
            "All three planes",
        ],
        "Choose which plane passes through the origin. Y is up; X and Z span the horizontal plane. All three planes makes the depth and orientation of the cloud easier to inspect.",
    );
    ui.checkbox(&mut s.show_axes,"Show XYZ axes").help_text("Mark world X in red, Y in green and Z in blue, with the origin at 0. The positive end of each axis is labeled. These coordinates also describe Vertical drift and Flatten to XZ.");
    ui.checkbox(&mut s.show_gizmo,"Show axis navigator").help_text("Show a small orientation compass at the upper right of this view. Click a positive or negative axis to look along it; drag the compass to rotate freely, including roll.");
}
fn camera_vector(s: &OrbitalSettings, p: [f32; 3]) -> [f32; 3] {
    let axes = trackball::orientation_axes(s.yaw, s.elevation, s.roll);
    std::array::from_fn(|r| {
        (0..3).map(|c| axes[c][r] as f32 * p[c]).sum::<f32>() * if r == 1 { -1.0 } else { 1.0 }
    })
}
pub(super) fn project(s: &OrbitalSettings, rect: Rect, p: [f32; 3]) -> Option<Pos2> {
    let p = camera_vector(s, p);
    let depth = s.camera_distance + p[2];
    if depth < 0.3 {
        return None;
    }
    let scale = if s.projection == 1 {
        2.0 / s.camera_distance
    } else {
        1.0 / (s.field_of_view.to_radians() * 0.5).tan() / depth
    };
    let k = s.zoom * rect.aspect_ratio().min(1.0) * scale;
    let ndc = Vec2::new(p[0] * k / rect.aspect_ratio() + s.pan_x, p[1] * k + s.pan_y);
    Some(rect.center() + ndc * rect.size() * Vec2::new(0.5, -0.5))
}
pub(super) fn draw_guides(ui: &egui::Ui, rect: Rect, s: &OrbitalSettings, theme: &AppTheme) {
    if !s.show_grid && !s.show_axes {
        return;
    }
    let painter = ui.painter_at(rect);
    let line = |a, b, color, width| {
        if let (Some(a), Some(b)) = (project(s, rect, a), project(s, rect, b)) {
            painter.line_segment([a, b], Stroke::new(width, color));
        }
    };
    let extent = s.grid_extent.floor();
    if s.show_grid {
        let color = theme.foreground.gamma_multiply(s.grid_opacity);
        for plane in 0..3 {
            if s.grid_plane != 3 && s.grid_plane != plane {
                continue;
            }
            let point = |a, b| match plane {
                0 => [a, 0.0, b],
                1 => [a, b, 0.0],
                _ => [0.0, a, b],
            };
            for i in -(extent as i32)..=extent as i32 {
                line(
                    point(i as f32, -extent),
                    point(i as f32, extent),
                    color,
                    0.7,
                );
                line(
                    point(-extent, i as f32),
                    point(extent, i as f32),
                    color,
                    0.7,
                );
            }
        }
    }
    if s.show_axes {
        for (axis, name, color) in [
            (0, "X", Color32::from_rgb(235, 100, 110)),
            (1, "Y", Color32::from_rgb(110, 210, 130)),
            (2, "Z", Color32::from_rgb(100, 165, 245)),
        ] {
            let mut a = [0.0; 3];
            let mut b = a;
            a[axis] = -extent;
            b[axis] = extent;
            line(a, b, color.gamma_multiply(0.7), 1.2);
            if let Some(p) = project(s, rect, b) {
                painter.text(
                    p,
                    egui::Align2::LEFT_BOTTOM,
                    name,
                    egui::FontId::proportional(12.0),
                    color,
                );
            }
        }
        if let Some(p) = project(s, rect, [0.0; 3]) {
            painter.text(
                p + Vec2::splat(4.0),
                egui::Align2::LEFT_TOP,
                "0",
                egui::FontId::proportional(10.0),
                theme.foreground,
            );
        }
    }
}
pub(super) fn gizmo_rect(rect: Rect) -> Rect {
    Rect::from_min_size(
        rect.right_top() + Vec2::new(-110.0, 10.0),
        Vec2::splat(100.0),
    )
}
pub(super) fn gizmo(ui: &mut egui::Ui, rect: Rect, s: &mut OrbitalSettings) {
    let painter = ui.painter_at(rect);
    painter.circle_filled(rect.center(), 46.0, Color32::from_black_alpha(140));
    let response=ui.interact(rect,ui.id().with("orbital-gizmo"),Sense::drag()).help_text("XYZ navigator\nRed X, green Y, blue Z. Click an axis label to look along it. Drag the circle to rotate freely. This changes only your view, not the flow of the particles.");
    if response.dragged()
        && let Some(p) = response.interact_pointer_pos()
    {
        let delta = ui.input(|i| i.pointer.delta());
        trackball::drag(
            &mut s.yaw,
            &mut s.elevation,
            &mut s.roll,
            p - delta,
            p,
            rect.center(),
            42.0,
        );
        s.auto_orbit = false;
    }
    let mut ends = Vec::new();
    for axis in 0..3 {
        for positive in [false, true] {
            let mut v = [0.0; 3];
            v[axis] = if positive { 1.0 } else { -1.0 };
            let p = camera_vector(s, v);
            ends.push((
                p[2],
                axis,
                positive,
                rect.center() + Vec2::new(p[0], -p[1]) * 32.0,
            ));
        }
    }
    // Far labels first; facing labels get the final, topmost hit target.
    ends.sort_by(|a, b| b.0.total_cmp(&a.0));
    for (_, axis, positive, p) in ends {
        let color = [
            Color32::from_rgb(235, 100, 110),
            Color32::from_rgb(110, 210, 130),
            Color32::from_rgb(100, 165, 245),
        ][axis];
        painter.line_segment(
            [rect.center(), p],
            Stroke::new(1.0, color.gamma_multiply(if positive { 1.0 } else { 0.4 })),
        );
        painter.circle_filled(p, 9.0, Color32::from_gray(28));
        let label = format!(
            "{}{}",
            if positive { "+" } else { "−" },
            ["X", "Y", "Z"][axis]
        );
        painter.text(
            p,
            egui::Align2::CENTER_CENTER,
            &label,
            egui::FontId::proportional(10.0),
            color,
        );
        if ui.interact(Rect::from_center_size(p,Vec2::splat(18.0)),ui.id().with(("orbital-axis",axis,positive)),Sense::click()).help_text(format!("View from {label}\nLook straight along this world axis. This removes roll and stops camera auto-orbit, while preserving zoom and pan.")).clicked() {snap(s,axis,positive);}
    }
}
