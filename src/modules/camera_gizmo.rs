use std::f32::consts::{FRAC_PI_2, PI};

use eframe::egui::{self, Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
    Z,
}

impl Axis {
    fn label(self) -> &'static str {
        match self {
            Self::X => "X",
            Self::Y => "Y",
            Self::Z => "Z",
        }
    }

    fn description(self) -> &'static str {
        match self {
            Self::X => "Frequency",
            Self::Y => "Time",
            Self::Z => "Level",
        }
    }

    fn color(self) -> Color32 {
        match self {
            Self::X => Color32::from_rgb(223, 105, 111),
            Self::Y => Color32::from_rgb(123, 192, 110),
            Self::Z => Color32::from_rgb(104, 160, 236),
        }
    }

    fn direction(self, positive: bool) -> [f32; 3] {
        let sign = if positive { 1.0 } else { -1.0 };
        // Expose familiar Z-up axes while the renderer uses Y for height.
        match self {
            Self::X => [sign, 0.0, 0.0],
            Self::Y => [0.0, 0.0, -sign],
            Self::Z => [0.0, sign, 0.0],
        }
    }

    fn angles(self, positive: bool) -> (f32, f32) {
        match (self, positive) {
            (Self::X, true) => (-FRAC_PI_2, 0.0),
            (Self::X, false) => (FRAC_PI_2, 0.0),
            (Self::Y, true) => (PI, 0.0),
            (Self::Y, false) => (0.0, 0.0),
            (Self::Z, true) => (0.0, FRAC_PI_2),
            (Self::Z, false) => (0.0, -FRAC_PI_2),
        }
    }

    pub fn snap(self, positive: bool, yaw: f32, elevation: f32) -> (f32, f32) {
        let (_, depth) = project_direction(yaw, elevation, self.direction(positive));
        // Selecting the axis already facing the viewer flips to its opposite.
        self.angles(if depth > 0.9999 { !positive } else { positive })
    }
}

pub fn project_direction(yaw: f32, elevation: f32, [x, y, z]: [f32; 3]) -> (Vec2, f32) {
    let (sin_yaw, cos_yaw) = yaw.sin_cos();
    let (sin_el, cos_el) = elevation.sin_cos();
    let horizontal = cos_yaw * x + sin_yaw * z;
    let forward = -sin_yaw * x + cos_yaw * z;
    (
        Vec2::new(horizontal, sin_el * forward - cos_el * y),
        sin_el * y + cos_el * forward,
    )
}

#[derive(Default)]
pub struct GizmoAction {
    pub orbit: Vec2,
    pub snap: Option<(Axis, bool)>,
}

struct Endpoint {
    axis: Axis,
    positive: bool,
    pos: Pos2,
    depth: f32,
}

fn endpoints(rect: Rect, yaw: f32, elevation: f32) -> Vec<Endpoint> {
    let arm = rect.width().min(rect.height()) * 0.31;
    let mut points = Vec::with_capacity(6);
    for axis in [Axis::X, Axis::Y, Axis::Z] {
        for positive in [false, true] {
            let (position, depth) = project_direction(yaw, elevation, axis.direction(positive));
            points.push(Endpoint {
                axis,
                positive,
                pos: rect.center() + position * arm,
                depth,
            });
        }
    }
    points.sort_by(|a, b| a.depth.total_cmp(&b.depth));
    points
}

/// One interaction region prevents axis clicks from competing with orbit drags.
pub fn draw(ui: &mut egui::Ui, rect: Rect, yaw: f32, elevation: f32, compact: bool) -> GizmoAction {
    let response = ui.interact(
        rect,
        ui.id().with("orientation-gizmo"),
        Sense::click_and_drag(),
    );
    let points = endpoints(rect, yaw, elevation);
    let radius = if compact { 8.0 } else { 12.0 };
    let hit = response.hover_pos().and_then(|pointer| {
        points
            .iter()
            .rev()
            .filter(|point| point.pos.distance(pointer) <= radius + 3.0)
            .min_by(|a, b| {
                a.pos
                    .distance_sq(pointer)
                    .total_cmp(&b.pos.distance_sq(pointer))
            })
    });
    let mut action = GizmoAction::default();
    if response.dragged_by(egui::PointerButton::Primary) {
        action.orbit = response.drag_delta();
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
    } else if response.clicked_by(egui::PointerButton::Primary) {
        action.snap = hit.map(|point| (point.axis, point.positive));
    }
    let painter = ui.painter_at(rect);
    let background = ui.visuals().extreme_bg_color;
    let foreground = ui.visuals().text_color();
    painter.circle_filled(
        rect.center(),
        rect.width().min(rect.height()) * 0.47,
        background,
    );
    painter.circle_stroke(
        rect.center(),
        rect.width().min(rect.height()) * 0.44,
        Stroke::new(0.7, ui.visuals().widgets.noninteractive.bg_stroke.color),
    );
    for point in &points {
        let color = point.axis.color();
        let hovered =
            hit.is_some_and(|hit| hit.axis == point.axis && hit.positive == point.positive);
        painter.line_segment(
            [rect.center(), point.pos],
            Stroke::new(if point.positive { 1.8 } else { 0.8 }, color),
        );
        painter.circle_filled(
            point.pos,
            radius,
            if point.positive { color } else { background },
        );
        painter.circle_stroke(
            point.pos,
            radius,
            Stroke::new(
                if hovered { 2.0 } else { 1.0 },
                if hovered { foreground } else { color },
            ),
        );
        painter.text(
            point.pos,
            egui::Align2::CENTER_CENTER,
            if point.positive {
                point.axis.label().to_owned()
            } else {
                format!("−{}", point.axis.label())
            },
            FontId::proportional(if compact { 9.0 } else { 11.0 }),
            if point.positive {
                Color32::from_rgb(18, 22, 28)
            } else {
                color
            },
        );
    }
    if let Some(point) = hit {
        response
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text(format!(
                "{}{} · {}\nClick to align; click again to flip. Drag to orbit.",
                if point.positive { "+" } else { "−" },
                point.axis.label(),
                point.axis.description()
            ));
    } else {
        response
            .on_hover_cursor(egui::CursorIcon::Grab)
            .on_hover_text("Drag to orbit. Click an axis to align the view.");
    }
    action
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_axis_aligns_and_repeated_selection_flips() {
        for axis in [Axis::X, Axis::Y, Axis::Z] {
            for sign in [false, true] {
                let (yaw, elevation) = axis.snap(sign, -0.35, 0.65);
                let (projected, depth) = project_direction(yaw, elevation, axis.direction(sign));
                assert!(projected.length() < 1.0e-5 && depth > 0.9999);
                let (yaw, elevation) = axis.snap(sign, yaw, elevation);
                assert!(project_direction(yaw, elevation, axis.direction(sign)).1 < -0.9999);
            }
        }
    }
}
