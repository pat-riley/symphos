use std::time::Instant;

use eframe::egui::{self, Pos2, Rect, Sense, Stroke, Vec2};

use super::{
    Palette,
    camera_gizmo::{self, Axis},
    frequency::{BANDS, History},
    frequency_label, label, mix, settings_panel,
};
use crate::help::HoverHelp;
use crate::{analysis::AnalysisFrame, theme::AppTheme};

const MAX_HEIGHT: f32 = 2.0;
const MIN_LENGTH: f32 = 0.25;
const MAX_LENGTH: f32 = 10.0;
const MIN_ZOOM: f32 = 0.5;
const MAX_ZOOM: f32 = 10.0;
const DEFAULT_HISTORY_SECONDS: f32 = 2.0;
const MIN_HISTORY_SECONDS: f32 = 0.1;
const MAX_HISTORY_SECONDS: f32 = 30.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum RenderMode {
    Surface,
    Lines,
    YLines,
    Dots,
    Wireframe,
}

pub struct Waterfall {
    history: History,
    seconds: f32,
    length_x: f32,
    length_y: f32,
    height: f32,
    detail: usize,
    yaw: f32,
    elevation: f32,
    zoom: f32,
    pan: Vec2,
    grid: bool,
    floor_grid: bool,
    mode: RenderMode,
    palette: Palette,
}

impl Default for Waterfall {
    fn default() -> Self {
        Self {
            history: History::default(),
            seconds: DEFAULT_HISTORY_SECONDS,
            length_x: 1.0,
            length_y: 1.0,
            height: 0.85,
            detail: 72,
            yaw: -0.35,
            elevation: 0.65,
            zoom: 1.0,
            pan: Vec2::ZERO,
            grid: true,
            floor_grid: true,
            mode: RenderMode::Surface,
            palette: Palette::default(),
        }
    }
}

impl Waterfall {
    pub fn clear(&mut self) {
        self.history.clear();
    }

    fn reset_camera(&mut self) {
        self.yaw = -0.35;
        self.elevation = 0.65;
        self.zoom = 1.0;
        self.pan = Vec2::ZERO;
    }

    fn orbit(&mut self, delta: Vec2) {
        self.yaw = wrap_angle(self.yaw + delta.x * 0.008);
        self.elevation = wrap_angle(self.elevation + delta.y * 0.006);
    }

    fn zoom_by_scroll(&mut self, scroll: f32) {
        self.zoom = (self.zoom * (scroll * 0.002).exp()).clamp(MIN_ZOOM, MAX_ZOOM);
    }

    fn camera(&self, plot: Rect) -> Camera {
        let mut camera = Camera::fit(
            plot,
            self.yaw,
            self.elevation,
            self.zoom,
            Vec2::new(self.length_x, self.length_y),
        );
        camera.center += self.pan * plot.size();
        camera
    }

    fn geometry_point(&self, [x, y, z]: [f32; 3]) -> [f32; 3] {
        // The user-facing Y (time) axis is Z in the renderer's Y-up coordinates.
        [x * self.length_x, y, z * self.length_y]
    }

    fn orientation_gizmo(&mut self, ui: &mut egui::Ui, rect: Rect, compact: bool) {
        let action = camera_gizmo::draw(ui, rect, self.yaw, self.elevation, compact);
        if action.orbit != Vec2::ZERO {
            self.orbit(action.orbit);
        }
        if let Some((axis, positive)) = action.snap {
            (self.yaw, self.elevation) = axis.snap(positive, self.yaw, self.elevation);
        }
    }

    pub fn controls(&mut self, ui: &mut egui::Ui) {
        settings_panel(ui, "Camera", true, |ui| {
            let (area, _) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), 128.0), Sense::hover());
            self.orientation_gizmo(
                ui,
                Rect::from_center_size(area.center(), Vec2::splat(128.0)),
                false,
            );
            ui.small("X frequency · Y time · Z level");
            ui.horizontal(|ui| {
                for (label, axis, positive) in [
                    ("Front", Axis::Y, false),
                    ("Side", Axis::X, true),
                    ("Top", Axis::Z, true),
                ] {
                    if ui
                        .small_button(label)
                        .help_text("Click again for the opposite view")
                        .clicked()
                    {
                        (self.yaw, self.elevation) = axis.snap(positive, self.yaw, self.elevation);
                    }
                }
            });
            egui::Grid::new("camera-angles")
                .num_columns(2)
                .show(ui, |ui| {
                    for (label, angle) in [
                        ("Rotation", &mut self.yaw),
                        ("Elevation", &mut self.elevation),
                    ] {
                        ui.label(label);
                        let mut degrees = angle.to_degrees();
                        if ui
                            .add(
                                egui::DragValue::new(&mut degrees)
                                    .speed(0.5)
                                    .range(-180.0..=180.0)
                                    .suffix("°"),
                            )
                            .help_text("Rotate the camera around the waterfall. Angles are in degrees; full rotation is supported.")
                            .changed()
                        {
                            *angle = degrees.to_radians();
                        }
                        ui.end_row();
                    }
                });
            ui.add(egui::Slider::new(&mut self.zoom, MIN_ZOOM..=MAX_ZOOM).logarithmic(true).text("Zoom"))
                .help_text("Magnify the view from 0.5× to 10× without changing history or geometry. Scroll over the waterfall to zoom; right-drag or Shift-drag to pan around a close-up.");
            ui.horizontal(|ui| {
                if ui
                    .small_button("Center view")
                    .help_text("Reset pan; keep rotation and zoom")
                    .clicked()
                {
                    self.pan = Vec2::ZERO;
                }
                if ui.small_button("Reset camera").help_text("Restore the default rotation, zoom, and pan. Audio history and geometry are unchanged.").clicked() {
                    self.reset_camera();
                }
            });
        });
        settings_panel(ui, "Time & History", true, |ui| {
            ui.add(
                egui::Slider::new(&mut self.seconds, MIN_HISTORY_SECONDS..=MAX_HISTORY_SECONDS)
                    .logarithmic(true)
                    .text("History s"),
            ).help_text("How many seconds of recent audio are displayed. Does not change the waterfall's physical length.");
        });
        settings_panel(ui, "Geometry", false, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.selectable_value(&mut self.mode, RenderMode::Surface, "Surface")
                    .help_text("A filled terrain surface whose height and color show signal level.");
                ui.selectable_value(&mut self.mode, RenderMode::Lines, "Lines")
                    .help_text("Separate frequency traces for each time slice, with no connecting surface.");
                ui.selectable_value(&mut self.mode, RenderMode::YLines, "Y Lines")
                    .help_text("Separate traces along the Y (time) axis, one per frequency band. No cross-frequency lines or filled faces.");
                ui.selectable_value(&mut self.mode, RenderMode::Wireframe, "Wireframe")
                    .help_text("An open mesh connecting frequency traces along time, without filled faces.");
                ui.selectable_value(&mut self.mode, RenderMode::Dots, "Dots")
                    .help_text("Individual colored dots at each frequency/time sample. Height and color show level, with no connecting lines or filled faces.");
            });
            ui.add(egui::Slider::new(&mut self.length_x, MIN_LENGTH..=MAX_LENGTH).text("Length X"))
                .help_text("Stretch or compress the frequency axis visually. 1 is the default size; frequency range and audio are unchanged.");
            ui.add(egui::Slider::new(&mut self.length_y, MIN_LENGTH..=MAX_LENGTH).text("Length Y"))
                .help_text("Stretch or compress the time axis visually. 1 is the default size; history duration and audio are unchanged.");
            ui.add(egui::Slider::new(&mut self.height, 0.0..=MAX_HEIGHT).text("Height"))
                .help_text("Scale signal peaks vertically above the fixed floor grid.");
            ui.add(egui::Slider::new(&mut self.detail, 24..=128).text("Time slices"))
                .help_text("Number of displayed time slices. More slices add detail and rendering work; history duration is unchanged.");
        });
        settings_panel(ui, "Frequency Range", false, |ui| {
            self.history.data.settings.range_controls(ui)
        });
        settings_panel(ui, "Signal Response", false, |ui| {
            self.history.data.settings.response_controls(ui)
        });
        settings_panel(ui, "Appearance", true, |ui| {
            self.palette.controls(ui);
            self.history.data.settings.level_controls(ui);
            if self.mode == RenderMode::Surface {
                ui.checkbox(&mut self.grid, "Surface grid")
                    .help_text("Show subtle mesh lines over the filled surface.");
            }
            ui.checkbox(&mut self.floor_grid, "Floor grid").help_text(
                "Show or hide the reference grid beneath the waterfall in any render mode.",
            );
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
        self.history.update(frame, now, live);
        let plot = rect.shrink2(Vec2::new(44.0, 32.0));
        let response = ui.interact(
            rect,
            ui.id().with("waterfall-camera"),
            Sense::click_and_drag(),
        );
        let shift = ui.input(|input| input.modifiers.shift);
        let panning = response.dragged_by(egui::PointerButton::Secondary)
            || response.dragged_by(egui::PointerButton::Middle)
            || (shift && response.dragged_by(egui::PointerButton::Primary));
        if panning {
            self.pan += response.drag_delta() / plot.size().max(Vec2::splat(1.0));
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        } else if response.dragged_by(egui::PointerButton::Primary) {
            self.orbit(response.drag_delta());
        }
        if response.hovered() && shift && !response.dragged() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        }
        if response.hovered() {
            let scroll = ui.input(|input| input.smooth_scroll_delta.y);
            self.zoom_by_scroll(scroll);
        }
        if response.double_clicked_by(egui::PointerButton::Primary) && !shift {
            self.reset_camera();
        }
        response.help_text("Drag to rotate. Right-drag, middle-drag, or Shift + left-drag to pan. Scroll to zoom. Double-click to reset the camera.");
        let painter = ui.painter_at(rect);
        let camera = self.camera(plot);
        let project = |x, y, z| camera.project(self.geometry_point([x, y, z])).0;
        let base_stroke = Stroke::new(0.6, mix(theme.background, theme.muted, 0.35));
        for step in 0..if self.floor_grid { 9 } else { 0 } {
            let t = step as f32 / 8.0;
            painter.line_segment(
                [
                    project(-1.4 + t * 2.8, 0.0, -1.0),
                    project(-1.4 + t * 2.8, 0.0, 1.0),
                ],
                base_stroke,
            );
            painter.line_segment(
                [
                    project(-1.4, 0.0, -1.0 + t * 2.0),
                    project(1.4, 0.0, -1.0 + t * 2.0),
                ],
                base_stroke,
            );
        }

        // CPU projection feeds a single GPU mesh. Cells and their subtle grid edges
        // are emitted back-to-front so rotating the surface preserves occlusion.
        let settings = &self.history.data.settings;
        let slices: Vec<_> = (0..self.detail)
            .map(|row| {
                let age = row as f32 / (self.detail - 1) as f32 * self.seconds;
                self.history.sample(now, age)
            })
            .collect();
        let mut vertices = Vec::with_capacity(self.detail * BANDS);
        for (row, levels) in slices.iter().enumerate() {
            let z = 1.0 - row as f32 / (self.detail - 1) as f32 * 2.0;
            for band in 0..BANDS {
                let intensity = levels.map_or(0.0, |values| settings.intensity(values[band]));
                let (pos, depth) = camera.project(self.geometry_point([
                    -1.4 + band as f32 / (BANDS - 1) as f32 * 2.8,
                    intensity * self.height,
                    z,
                ]));
                vertices.push((pos, depth, self.palette.color(intensity, theme)));
            }
        }
        let mut mesh = egui::Mesh::default();
        match self.mode {
            RenderMode::Dots => {
                let mut points: Vec<_> = vertices
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| slices[index / BANDS].is_some())
                    .map(|(_, vertex)| *vertex)
                    .collect();
                points.sort_unstable_by(|a, b| a.1.total_cmp(&b.1));
                for (position, _, color) in points {
                    mesh_dot(&mut mesh, position, color);
                }
            }
            RenderMode::Lines | RenderMode::YLines | RenderMode::Wireframe => {
                let mut segments = Vec::with_capacity(self.detail * BANDS * 2);
                for (row, levels) in slices.iter().enumerate() {
                    if levels.is_none() {
                        continue;
                    }
                    if self.mode != RenderMode::YLines {
                        for band in 0..BANDS - 1 {
                            let a = row * BANDS + band;
                            segments.push(((vertices[a].1 + vertices[a + 1].1) / 2.0, a, a + 1));
                        }
                    }
                    if self.mode != RenderMode::Lines
                        && row + 1 < self.detail
                        && slices[row + 1].is_some()
                    {
                        for band in 0..BANDS {
                            let a = row * BANDS + band;
                            let b = a + BANDS;
                            segments.push(((vertices[a].1 + vertices[b].1) / 2.0, a, b));
                        }
                    }
                }
                segments.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
                for (_, a, b) in segments {
                    mesh_segment(
                        &mut mesh,
                        vertices[a].0,
                        vertices[b].0,
                        vertices[a].2,
                        vertices[b].2,
                        1.1,
                    );
                }
            }
            RenderMode::Surface => {
                let floor_offset = vertices.len();
                for row in 0..self.detail {
                    let z = 1.0 - row as f32 / (self.detail - 1) as f32 * 2.0;
                    for band in 0..BANDS {
                        let (position, depth) = camera.project(self.geometry_point([
                            -1.4 + band as f32 / (BANDS - 1) as f32 * 2.8,
                            0.0,
                            z,
                        ]));
                        let color = mix(theme.background, vertices[row * BANDS + band].2, 0.6);
                        vertices.push((position, depth, color));
                    }
                }
                let mut faces = Vec::with_capacity((self.detail - 1) * (BANDS - 1));
                let mut add_face = |indices: [usize; 4], grid_edges: [bool; 2]| {
                    let depth = indices.iter().map(|i| vertices[*i].1).sum::<f32>() / 4.0;
                    faces.push((depth, indices, grid_edges));
                };
                for row in 0..self.detail - 1 {
                    if slices[row].is_none() || slices[row + 1].is_none() {
                        continue;
                    }
                    for band in 0..BANDS - 1 {
                        let a = row * BANDS + band;
                        add_face(
                            [a, a + 1, a + BANDS + 1, a + BANDS],
                            [self.grid && row % 3 == 0, self.grid && band % 4 == 0],
                        );
                        // Perimeter walls share the same y=0 plane as the floor.
                        // Height stretches the terrain from that fixed foundation.
                        if row == 0 || slices[row - 1].is_none() {
                            add_face(
                                [a, a + 1, floor_offset + a + 1, floor_offset + a],
                                [false; 2],
                            );
                        }
                        if row + 2 == self.detail || slices[row + 2].is_none() {
                            let b = a + BANDS;
                            add_face(
                                [b, b + 1, floor_offset + b + 1, floor_offset + b],
                                [false; 2],
                            );
                        }
                    }
                    let a = row * BANDS;
                    add_face(
                        [a, a + BANDS, floor_offset + a + BANDS, floor_offset + a],
                        [false; 2],
                    );
                    let b = a + BANDS - 1;
                    add_face(
                        [b, b + BANDS, floor_offset + b + BANDS, floor_offset + b],
                        [false; 2],
                    );
                }
                faces.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
                for (_, indices, grid_edges) in faces {
                    let offset = mesh.vertices.len() as u32;
                    for index in indices {
                        let (pos, _, color) = vertices[index];
                        mesh.colored_vertex(pos, color);
                    }
                    mesh.add_triangle(offset, offset + 1, offset + 2);
                    mesh.add_triangle(offset, offset + 2, offset + 3);
                    let color = mix(theme.background, theme.foreground, 0.22);
                    if grid_edges[0] {
                        mesh_line(
                            &mut mesh,
                            vertices[indices[0]].0,
                            vertices[indices[1]].0,
                            color,
                        );
                    }
                    if grid_edges[1] {
                        mesh_line(
                            &mut mesh,
                            vertices[indices[0]].0,
                            vertices[indices[3]].0,
                            color,
                        );
                    }
                }
            }
        }
        painter.add(egui::Shape::mesh(mesh));
        let axis_stroke = Stroke::new(1.0, theme.muted);
        painter.line_segment(
            [project(-1.4, 0.0, 1.0), project(1.4, 0.0, 1.0)],
            axis_stroke,
        );
        painter.line_segment(
            [project(1.4, 0.0, 1.0), project(1.4, 0.0, -1.0)],
            axis_stroke,
        );
        painter.line_segment(
            [project(-1.4, 0.0, 1.0), project(-1.4, self.height, 1.0)],
            axis_stroke,
        );
        for step in 0..=4 {
            let t = step as f32 / 4.0;
            label(
                &painter,
                project(-1.4 + t * 2.8, 0.0, 1.0) + Vec2::new(-8.0, 6.0),
                frequency_label(settings.frequency(t, frame.sample_rate)),
                theme.muted,
            );
        }
        label(
            &painter,
            project(1.4, 0.0, -1.0) + Vec2::new(5.0, 0.0),
            format!("−{} s", self.seconds),
            theme.muted,
        );
        label(
            &painter,
            project(1.4, 0.0, 1.0) + Vec2::new(12.0, -12.0),
            "now",
            theme.muted,
        );
        label(
            &painter,
            project(-1.4, self.height, 1.0) + Vec2::new(-8.0, -16.0),
            format!("{:.0} dBFS", settings.ceiling),
            theme.muted,
        );
        if rect.width() > 380.0 {
            label(
                &painter,
                rect.left_top() + Vec2::new(12.0, 10.0),
                "FREQUENCY × TIME · HEIGHT = LEVEL",
                theme.muted,
            );
        }
        label(
            &painter,
            rect.left_bottom() + Vec2::new(12.0, -18.0),
            "Drag: rotate · Right/Shift-drag: pan · Scroll: zoom",
            theme.muted,
        );
        let gizmo_rect =
            Rect::from_min_size(rect.right_top() + Vec2::new(-88.0, 6.0), Vec2::splat(80.0));
        self.orientation_gizmo(ui, gizmo_rect, true);
    }
}

struct Camera {
    yaw: f32,
    elevation: f32,
    scale: f32,
    center: Pos2,
}

impl Camera {
    fn fit(rect: Rect, yaw: f32, elevation: f32, zoom: f32, length: Vec2) -> Self {
        // A sphere enclosing the full height range fits at every orientation.
        // Its scale and orbit center stay fixed while either angle changes.
        let radius = ((1.4 * length.x.max(1.0)).powi(2)
            + length.y.max(1.0).powi(2)
            + (MAX_HEIGHT * 0.5).powi(2))
        .sqrt();
        Self {
            yaw,
            elevation,
            scale: rect.width().min(rect.height()).max(0.0) / (2.0 * radius) * zoom,
            center: rect.center(),
        }
    }

    fn project(&self, [x, y, z]: [f32; 3]) -> (Pos2, f32) {
        let y = y - MAX_HEIGHT * 0.5;
        let (position, depth) =
            camera_gizmo::project_direction(self.yaw, self.elevation, [x, y, z]);
        (self.center + position * self.scale, depth)
    }
}

fn wrap_angle(angle: f32) -> f32 {
    (angle + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}

fn mesh_line(mesh: &mut egui::Mesh, a: Pos2, b: Pos2, color: egui::Color32) {
    mesh_segment(mesh, a, b, color, color, 0.6);
}

fn mesh_dot(mesh: &mut egui::Mesh, center: Pos2, color: egui::Color32) {
    // Small screen-space discs stay legible at every zoom and share one GPU mesh.
    const SIDES: usize = 8;
    const RADIUS: f32 = 1.6;
    let offset = mesh.vertices.len() as u32;
    mesh.colored_vertex(center, color);
    for side in 0..SIDES {
        let angle = side as f32 * std::f32::consts::TAU / SIDES as f32;
        mesh.colored_vertex(center + Vec2::angled(angle) * RADIUS, color);
    }
    for side in 0..SIDES as u32 {
        mesh.add_triangle(
            offset,
            offset + 1 + side,
            offset + 1 + (side + 1) % SIDES as u32,
        );
    }
}

fn mesh_segment(
    mesh: &mut egui::Mesh,
    a: Pos2,
    b: Pos2,
    color_a: egui::Color32,
    color_b: egui::Color32,
    width: f32,
) {
    let delta = b - a;
    let normal = Vec2::new(-delta.y, delta.x).normalized() * width * 0.5;
    let offset = mesh.vertices.len() as u32;
    for (point, color) in [
        (a + normal, color_a),
        (b + normal, color_b),
        (b - normal, color_b),
        (a - normal, color_a),
    ] {
        mesh.colored_vertex(point, color);
    }
    mesh.add_triangle(offset, offset + 1, offset + 2);
    mesh.add_triangle(offset, offset + 2, offset + 3);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dots_are_separate_discs_and_skip_missing_history() {
        use super::super::frequency::HistoryRow;
        let now = Instant::now();
        let mut waterfall = Waterfall {
            mode: RenderMode::Dots,
            detail: 24,
            seconds: 1.0,
            ..Waterfall::default()
        };
        waterfall.history.rows.push_back(HistoryRow {
            time: now,
            sequence: 1,
            levels: [-45.0; BANDS],
            magnitudes: Vec::new(),
        });
        let context = egui::Context::default();
        let mut output = context.run_ui(egui::RawInput::default(), |ui| {
            waterfall.draw(
                ui,
                Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0)),
                &AnalysisFrame::default(),
                &AppTheme::default(),
                now,
                false,
            );
        });
        output.textures_delta.clear();
        let mesh = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Mesh(mesh) => Some(mesh),
                _ => None,
            })
            .expect("dots mesh");
        // Only three slices lie within the 120 ms freshness limit.
        assert_eq!(mesh.vertices.len(), 3 * BANDS * 9);
        assert_eq!(mesh.indices.len(), 3 * BANDS * 24);
        for triangle in mesh.indices.as_chunks::<3>().0 {
            assert_eq!(triangle[0] / 9, triangle[1] / 9);
            assert_eq!(triangle[0] / 9, triangle[2] / 9);
        }
        for dot in mesh.vertices.as_chunks::<9>().0 {
            assert!(
                dot.iter()
                    .all(|vertex| vertex.pos.distance(dot[0].pos) <= 1.601)
            );
        }
    }

    #[test]
    fn changing_history_duration_preserves_audio_and_camera() {
        use super::super::frequency::HistoryRow;
        let mut waterfall = Waterfall {
            zoom: 5.0,
            pan: Vec2::new(0.2, -0.1),
            ..Waterfall::default()
        };
        let now = Instant::now();
        waterfall.history.rows.push_back(HistoryRow {
            time: now,
            sequence: 42,
            levels: [-45.0; BANDS],
            magnitudes: vec![0.1],
        });
        for seconds in [
            MIN_HISTORY_SECONDS,
            DEFAULT_HISTORY_SECONDS,
            MAX_HISTORY_SECONDS,
        ] {
            waterfall.seconds = seconds;
            let mut output = egui::Context::default().run_ui(egui::RawInput::default(), |ui| {
                waterfall.draw(
                    ui,
                    Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0)),
                    &AnalysisFrame::default(),
                    &AppTheme::default(),
                    now,
                    false,
                );
            });
            output.textures_delta.clear();
            assert_eq!(waterfall.seconds, seconds);
            assert_eq!(waterfall.history.rows.len(), 1);
            assert_eq!(waterfall.history.rows[0].time, now);
            assert_eq!(waterfall.history.rows[0].magnitudes, vec![0.1]);
            assert_eq!(waterfall.zoom, 5.0);
            assert_eq!(waterfall.pan, Vec2::new(0.2, -0.1));
        }
    }

    #[test]
    fn y_lines_only_connect_time_samples_and_leave_stale_history_blank() {
        use super::super::frequency::HistoryRow;
        let now = Instant::now();
        let mut waterfall = Waterfall {
            mode: RenderMode::YLines,
            detail: 24,
            yaw: 0.0,
            height: 0.0,
            ..Waterfall::default()
        };
        waterfall.history.rows.push_back(HistoryRow {
            time: now,
            sequence: 1,
            levels: [-45.0; BANDS],
            magnitudes: Vec::new(),
        });
        let context = egui::Context::default();
        for (seconds, connections) in [(0.1, 23), (1.0, 2)] {
            waterfall.seconds = seconds;
            let mut output = context.run_ui(egui::RawInput::default(), |ui| {
                waterfall.draw(
                    ui,
                    Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0)),
                    &AnalysisFrame::default(),
                    &AppTheme::default(),
                    now,
                    false,
                );
            });
            output.textures_delta.clear();
            let mesh = output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Mesh(mesh) => Some(mesh),
                    _ => None,
                })
                .expect("waterfall mesh");
            assert_eq!(mesh.vertices.len(), connections * BANDS * 4);
            for segment in mesh.vertices.as_chunks::<4>().0 {
                let a = (segment[0].pos.to_vec2() + segment[3].pos.to_vec2()) * 0.5;
                let b = (segment[1].pos.to_vec2() + segment[2].pos.to_vec2()) * 0.5;
                assert!((a.x - b.x).abs() < 0.001, "no cross-frequency connection");
                assert!((a.y - b.y).abs() > 0.01, "trace advances along time");
            }
        }
    }

    #[test]
    fn scroll_zoom_reaches_extended_limit_without_changing_pan_or_geometry() {
        let mut waterfall = Waterfall {
            pan: Vec2::new(0.2, -0.1),
            ..Waterfall::default()
        };
        waterfall.zoom_by_scroll(500.0);
        assert!(waterfall.zoom > 2.0);
        waterfall.zoom_by_scroll(10_000.0);
        assert_eq!(waterfall.zoom, MAX_ZOOM);
        assert_eq!(waterfall.pan, Vec2::new(0.2, -0.1));
        assert_eq!(
            (waterfall.length_x, waterfall.length_y, waterfall.seconds),
            (1.0, 1.0, 2.0)
        );
        waterfall.zoom_by_scroll(-10_000.0);
        assert_eq!(waterfall.zoom, MIN_ZOOM);
        waterfall.reset_camera();
        assert_eq!(waterfall.zoom, 1.0);
    }

    #[test]
    fn geometry_lengths_scale_frequency_and_time_independently() {
        let mut waterfall = Waterfall::default();
        let point = [1.0, 0.5, -1.0];
        let settings = waterfall.history.data.settings.clone();
        assert_eq!(waterfall.geometry_point(point), point);
        waterfall.length_x = 2.0;
        assert_eq!(waterfall.geometry_point(point), [2.0, 0.5, -1.0]);
        waterfall.length_y = 0.25;
        assert_eq!(waterfall.geometry_point(point), [2.0, 0.5, -0.25]);
        waterfall.reset_camera();
        assert_eq!(waterfall.geometry_point(point), [2.0, 0.5, -0.25]);
        assert_eq!(waterfall.seconds, 2.0);
        assert_eq!(waterfall.history.data.settings, settings);
    }

    #[test]
    fn viewport_gizmo_snaps_and_drags_without_panning_or_double_orbiting() {
        let context = egui::Context::default();
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let gizmo =
            Rect::from_min_size(rect.right_top() + Vec2::new(-88.0, 6.0), Vec2::splat(80.0));
        let mut waterfall = Waterfall {
            pan: Vec2::new(0.2, -0.1),
            zoom: 1.5,
            ..Waterfall::default()
        };
        let render = |waterfall: &mut Waterfall, events| {
            let mut output = context.run_ui(
                egui::RawInput {
                    screen_rect: Some(rect),
                    events,
                    ..Default::default()
                },
                |ui| {
                    waterfall.draw(
                        ui,
                        rect,
                        &AnalysisFrame::default(),
                        &AppTheme::default(),
                        Instant::now(),
                        false,
                    );
                },
            );
            output.textures_delta.clear();
        };
        render(&mut waterfall, vec![]);
        let axis = gizmo.center()
            + camera_gizmo::project_direction(waterfall.yaw, waterfall.elevation, [1.0, 0.0, 0.0])
                .0
                * gizmo.width()
                * 0.31;
        for pressed in [true, false] {
            render(
                &mut waterfall,
                vec![
                    egui::Event::PointerMoved(axis),
                    egui::Event::PointerButton {
                        pos: axis,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
        }
        assert!((waterfall.yaw + std::f32::consts::FRAC_PI_2).abs() < 1.0e-5);
        assert_eq!(waterfall.elevation, 0.0);
        assert_eq!(waterfall.pan, Vec2::new(0.2, -0.1));
        assert_eq!(waterfall.zoom, 1.5);
        let before = (waterfall.yaw, waterfall.elevation);
        let start = gizmo.center();
        let delta = Vec2::new(12.0, 18.0);
        render(
            &mut waterfall,
            vec![
                egui::Event::PointerMoved(start),
                egui::Event::PointerButton {
                    pos: start,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        render(
            &mut waterfall,
            vec![egui::Event::PointerMoved(start + delta)],
        );
        assert!((waterfall.yaw - wrap_angle(before.0 + delta.x * 0.008)).abs() < 1.0e-5);
        assert!((waterfall.elevation - wrap_angle(before.1 + delta.y * 0.006)).abs() < 1.0e-5);
        assert_eq!(waterfall.pan, Vec2::new(0.2, -0.1));
    }

    #[test]
    fn pointer_gestures_pan_without_rotating_and_plain_left_drag_still_orbits() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let plot = rect.shrink2(Vec2::new(44.0, 32.0));
        for (button, shift, should_pan) in [
            (egui::PointerButton::Secondary, false, true),
            (egui::PointerButton::Middle, false, true),
            (egui::PointerButton::Primary, true, true),
            (egui::PointerButton::Primary, false, false),
        ] {
            let context = egui::Context::default();
            let mut waterfall = Waterfall::default();
            let initial_angles = (waterfall.yaw, waterfall.elevation);
            let initial_center = waterfall.camera(plot).center;
            let modifiers = egui::Modifiers {
                shift,
                ..Default::default()
            };
            let start = rect.center();
            let movement = Vec2::new(30.0, -20.0);
            for events in [
                vec![egui::Event::ModifiersChanged(modifiers)],
                vec![
                    egui::Event::PointerMoved(start),
                    egui::Event::PointerButton {
                        pos: start,
                        button,
                        pressed: true,
                        modifiers,
                    },
                ],
                vec![egui::Event::PointerMoved(start + movement)],
            ] {
                let mut output = context.run_ui(
                    egui::RawInput {
                        screen_rect: Some(rect),
                        events,
                        ..Default::default()
                    },
                    |ui| {
                        waterfall.draw(
                            ui,
                            rect,
                            &AnalysisFrame::default(),
                            &AppTheme::default(),
                            Instant::now(),
                            false,
                        )
                    },
                );
                output.textures_delta.clear();
            }
            if should_pan {
                assert_eq!((waterfall.yaw, waterfall.elevation), initial_angles);
                assert!(
                    (waterfall.camera(plot).center - initial_center - movement).length() < 0.01
                );
                waterfall.zoom = 2.0;
                waterfall.orbit(Vec2::new(100.0, 200.0));
                assert!(
                    (waterfall.camera(plot).center - initial_center - movement).length() < 0.01
                );
            } else {
                assert_eq!(waterfall.pan, Vec2::ZERO);
                assert_ne!((waterfall.yaw, waterfall.elevation), initial_angles);
            }
            waterfall.reset_camera();
            assert_eq!(waterfall.pan, Vec2::ZERO);
            assert_eq!(waterfall.zoom, 1.0);
            assert_eq!((waterfall.yaw, waterfall.elevation), initial_angles);
        }
    }

    #[test]
    fn camera_rotation_changes_depth_and_keeps_projection_finite() {
        let mut camera = Camera {
            yaw: 0.0,
            elevation: 0.65,
            scale: 100.0,
            center: Pos2::ZERO,
        };
        let (_, front) = camera.project([0.0, 0.0, 1.0]);
        camera.yaw = std::f32::consts::PI;
        let (position, back) = camera.project([0.0, 0.0, 1.0]);
        assert!(front > 0.0 && back < 0.0);
        assert!(position.x.is_finite() && position.y.is_finite());
    }

    #[test]
    fn full_height_range_fits_above_a_fixed_floor() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 500.0));
        for yaw in [-3.0, -1.5, 0.0, 1.5, 3.0] {
            for elevation in [0.15, 0.65, 1.45] {
                let camera = Camera::fit(rect, yaw, elevation, 1.0, Vec2::splat(1.0));
                for x in [-1.4, 0.0, 1.4] {
                    for z in [-1.0, 0.0, 1.0] {
                        let floor = camera.project([x, 0.0, z]).0;
                        for height in [0.0, 0.85, MAX_HEIGHT] {
                            let top = camera.project([x, height, z]).0;
                            assert!(rect.expand(0.01).contains(top));
                            assert!((top.x - floor.x).abs() < 0.01);
                            assert!(top.y <= floor.y + 0.01);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn dragging_orbits_through_poles_and_wraps_full_turns() {
        let mut waterfall = Waterfall::default();
        let initial = (waterfall.yaw, waterfall.elevation);
        waterfall.orbit(Vec2::new(0.0, 250.0));
        assert!(waterfall.elevation > std::f32::consts::FRAC_PI_2);
        waterfall.orbit(Vec2::new(0.0, -500.0));
        assert!(waterfall.elevation < 0.0);
        waterfall.reset_camera();
        waterfall.orbit(Vec2::new(
            std::f32::consts::TAU / 0.008,
            std::f32::consts::TAU / 0.006,
        ));
        assert!((waterfall.yaw - initial.0).abs() < 1.0e-5);
        assert!((waterfall.elevation - initial.1).abs() < 1.0e-5);
    }

    #[test]
    fn orbit_keeps_scale_and_center_stable_and_fits_all_orientations() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 500.0));
        for length_x in [MIN_LENGTH, 1.0, 3.0, MAX_LENGTH] {
            for length_y in [MIN_LENGTH, 1.0, 3.0, MAX_LENGTH] {
                let length = Vec2::new(length_x, length_y);
                let reference = Camera::fit(rect, 0.0, 0.0, 1.0, length);
                for yaw_step in -8..=8 {
                    for elevation_step in -8..=8 {
                        let camera = Camera::fit(
                            rect,
                            yaw_step as f32 * std::f32::consts::PI / 8.0,
                            elevation_step as f32 * std::f32::consts::PI / 8.0,
                            1.0,
                            length,
                        );
                        assert_eq!(camera.scale, reference.scale);
                        assert_eq!(
                            camera.project([0.0, MAX_HEIGHT * 0.5, 0.0]).0,
                            rect.center()
                        );
                        for x in [-1.4 * length.x, 1.4 * length.x] {
                            for z in [-length.y, length.y] {
                                for y in [0.0, MAX_HEIGHT] {
                                    let position = camera.project([x, y, z]).0;
                                    assert!(rect.expand(0.01).contains(position));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn full_history_renders_bounded_finite_mesh_at_multiple_angles_and_sizes() {
        use super::super::frequency::HistoryRow;
        use std::time::Duration;

        let mut waterfall = Waterfall {
            detail: 128,
            palette: Palette::Heatmap,
            ..Waterfall::default()
        };
        let start = Instant::now();
        for row in 0..=240 {
            let mut levels = [-120.0; BANDS];
            for (band, level) in levels.iter_mut().enumerate() {
                *level = -45.0 + (band as f32 * 0.2 + row as f32 * 0.1).sin() * 30.0;
            }
            waterfall.history.rows.push_back(HistoryRow {
                time: start + Duration::from_secs_f32(row as f32 / 30.0),
                levels,
                magnitudes: Vec::new(),
                sequence: row + 1,
            });
        }
        let now = start + Duration::from_secs(8);
        let retained: Vec<_> = waterfall
            .history
            .rows
            .iter()
            .map(|row| (row.time, row.sequence, row.levels))
            .collect();
        let context = egui::Context::default();
        let theme = AppTheme::default();
        let started = Instant::now();
        for (size, mode, seconds, floor_grid, length) in [
            (
                Vec2::new(180.0, 200.0),
                RenderMode::Surface,
                2.0,
                true,
                MIN_LENGTH,
            ),
            (
                Vec2::new(1100.0, 500.0),
                RenderMode::Surface,
                0.1,
                false,
                MAX_LENGTH,
            ),
            (
                Vec2::new(180.0, 200.0),
                RenderMode::Lines,
                0.1,
                true,
                MIN_LENGTH,
            ),
            (
                Vec2::new(1100.0, 500.0),
                RenderMode::Lines,
                2.0,
                false,
                MAX_LENGTH,
            ),
            (
                Vec2::new(180.0, 200.0),
                RenderMode::YLines,
                0.1,
                true,
                MIN_LENGTH,
            ),
            (
                Vec2::new(180.0, 200.0),
                RenderMode::Dots,
                0.1,
                true,
                MIN_LENGTH,
            ),
            (
                Vec2::new(1100.0, 500.0),
                RenderMode::Dots,
                2.0,
                false,
                MAX_LENGTH,
            ),
            (
                Vec2::new(1100.0, 500.0),
                RenderMode::YLines,
                2.0,
                false,
                MAX_LENGTH,
            ),
            (
                Vec2::new(180.0, 200.0),
                RenderMode::Wireframe,
                0.1,
                true,
                MIN_LENGTH,
            ),
            (
                Vec2::new(1100.0, 500.0),
                RenderMode::Wireframe,
                2.0,
                false,
                MAX_LENGTH,
            ),
        ] {
            waterfall.mode = mode;
            waterfall.seconds = seconds;
            waterfall.floor_grid = floor_grid;
            waterfall.length_y = length;
            waterfall.length_x = MIN_LENGTH + MAX_LENGTH - length;
            // Include close-ups beyond the former 2× limit in each render mode.
            waterfall.zoom = if floor_grid { 1.0 } else { MAX_ZOOM };
            for yaw in [-3.0, -1.5, 0.0, 1.5, 3.0] {
                waterfall.yaw = yaw;
                waterfall.elevation = yaw;
                let rect = Rect::from_min_size(Pos2::ZERO, size);
                let mut output = context.run_ui(egui::RawInput::default(), |ui| {
                    waterfall.draw(ui, rect, &AnalysisFrame::default(), &theme, now, false);
                });
                // This headless geometry check intentionally does not upload textures.
                output.textures_delta.clear();
                let meshes: Vec<_> = output
                    .shapes
                    .iter()
                    .filter_map(|shape| {
                        if let egui::Shape::Mesh(mesh) = &shape.shape {
                            Some(mesh)
                        } else {
                            None
                        }
                    })
                    .collect();
                assert_eq!(meshes.len(), 1);
                let floor_and_axes = output
                    .shapes
                    .iter()
                    .filter(|shape| matches!(shape.shape, egui::Shape::LineSegment { .. }))
                    .count();
                assert_eq!(floor_and_axes, if floor_grid { 27 } else { 9 });
                let mesh = meshes[0];
                assert!(mesh.vertices.len() > 40_000 && mesh.vertices.len() < 120_000);
                assert!(mesh.is_valid());
                assert!(
                    mesh.vertices
                        .iter()
                        .all(|vertex| vertex.pos.x.is_finite() && vertex.pos.y.is_finite())
                );
                assert!(waterfall.history.rows.len() <= 901);
                assert_eq!(
                    waterfall
                        .history
                        .rows
                        .iter()
                        .map(|row| (row.time, row.sequence, row.levels))
                        .collect::<Vec<_>>(),
                    retained
                );
                assert_eq!(waterfall.seconds, seconds);
            }
        }
        eprintln!(
            "50 maximum-detail waterfall frames: {:?}",
            started.elapsed()
        );
    }
}
