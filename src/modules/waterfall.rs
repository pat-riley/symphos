use std::time::Instant;

use eframe::egui::{self, Pos2, Rect, Sense, Stroke, Vec2};

use super::{
    Palette,
    camera_gizmo::{self, Axis},
    frequency::{BANDS, History},
    label, mix, settings_panel,
};
use crate::help::HoverHelp;
use crate::icons::{self, Icon};
use crate::{analysis::AnalysisFrame, theme::AppTheme};

const MAX_HEIGHT: f32 = 2.0;
const MIN_LENGTH: f32 = 0.25;
const MAX_LENGTH: f32 = 10.0;
const MIN_ZOOM: f32 = 0.5;
const MAX_ZOOM: f32 = 10.0;
const DEFAULT_HISTORY_SECONDS: f32 = 2.0;
const MIN_HISTORY_SECONDS: f32 = 0.1;
const MAX_HISTORY_SECONDS: f32 = 30.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RenderMode {
    Surface,
    Lines,
    YLines,
    Dots,
    Wireframe,
    Stems,
}

impl RenderMode {
    const ALL: [Self; 6] = [
        Self::Surface,
        Self::Lines,
        Self::YLines,
        Self::Wireframe,
        Self::Dots,
        Self::Stems,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Surface => "Surface",
            Self::Lines => "Lines",
            Self::YLines => "Y Lines",
            Self::Wireframe => "Wireframe",
            Self::Dots => "Dots",
            Self::Stems => "Stems",
        }
    }

    fn icon(self) -> Icon {
        match self {
            Self::Surface => Icon::Surface,
            Self::Lines => Icon::Lines,
            Self::YLines => Icon::YLines,
            Self::Wireframe => Icon::Wireframe,
            Self::Dots => Icon::Dots,
            Self::Stems => Icon::Stems,
        }
    }

    fn description(self) -> &'static str {
        match self {
            Self::Surface => "Filled terrain with optional mesh lines and walls down to the floor.",
            Self::Lines => "Frequency traces across X, one per time slice.",
            Self::YLines => "Time traces along Y, one per frequency band.",
            Self::Wireframe => "An open mesh of frequency and time traces.",
            Self::Dots => "A point cloud of individual frequency/time samples.",
            Self::Stems => "A forest of colored pins rising from the floor to each signal level.",
        }
    }
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
    guides: bool,
    axes: bool,
    show_gizmo: bool,
    surface_walls: bool,
    line_width: f32,
    line_spacing: usize,
    y_line_width: f32,
    y_line_spacing: usize,
    wire_width: f32,
    wire_spacing: usize,
    dot_size: f32,
    dot_spacing: usize,
    stem_width: f32,
    stem_spacing: usize,
    contrast: f32,
    auto_orbit: bool,
    orbit_speed: f32,
    orbit_reverse: bool,
    last_draw: Option<Instant>,
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
            guides: true,
            axes: true,
            show_gizmo: true,
            surface_walls: true,
            line_width: 1.1,
            line_spacing: 1,
            y_line_width: 1.1,
            y_line_spacing: 1,
            wire_width: 1.1,
            wire_spacing: 1,
            dot_size: 3.2,
            dot_spacing: 1,
            stem_width: 1.2,
            stem_spacing: 4,
            contrast: 1.0,
            auto_orbit: false,
            orbit_speed: 10.0,
            orbit_reverse: false,
            last_draw: None,
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
        self.auto_orbit = false;
        self.yaw = -0.35;
        self.elevation = 0.65;
        self.zoom = 1.0;
        self.pan = Vec2::ZERO;
    }

    fn orbit(&mut self, delta: Vec2) {
        self.auto_orbit = false;
        self.yaw = wrap_angle(self.yaw + delta.x * 0.008);
        self.elevation = wrap_angle(self.elevation + delta.y * 0.006);
    }

    fn zoom_by_scroll(&mut self, scroll: f32) {
        if scroll != 0.0 {
            self.auto_orbit = false;
        }
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

    fn snap_view(&mut self, angles: (f32, f32)) {
        self.auto_orbit = false;
        (self.yaw, self.elevation) = angles;
    }

    fn isometric_angles(corner: usize, above: bool) -> (f32, f32) {
        let yaw = -std::f32::consts::FRAC_PI_4 + corner as f32 * std::f32::consts::FRAC_PI_2;
        (
            wrap_angle(yaw),
            (1.0 / 3.0_f32.sqrt()).asin() * if above { 1.0 } else { -1.0 },
        )
    }

    fn advance_camera(&mut self, now: Instant) {
        let dt = self
            .last_draw
            .map_or(0.0, |last| {
                now.saturating_duration_since(last).as_secs_f32()
            })
            .min(0.1);
        self.last_draw = Some(now);
        if self.auto_orbit {
            let direction = if self.orbit_reverse { -1.0 } else { 1.0 };
            self.yaw = wrap_angle(self.yaw + self.orbit_speed.to_radians() * direction * dt);
        }
    }

    fn orientation_gizmo(&mut self, ui: &mut egui::Ui, rect: Rect, compact: bool) {
        let action = camera_gizmo::draw(ui, rect, self.yaw, self.elevation, compact);
        if action.orbit != Vec2::ZERO {
            self.orbit(action.orbit);
        }
        if let Some((axis, positive)) = action.snap {
            self.snap_view(axis.snap(positive, self.yaw, self.elevation));
        }
    }

    pub fn controls(&mut self, ui: &mut egui::Ui) {
        settings_panel(ui, "Camera", |ui| {
            let (area, _) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), 128.0), Sense::hover());
            self.orientation_gizmo(
                ui,
                Rect::from_center_size(area.center(), Vec2::splat(128.0)),
                false,
            );
            ui.small("X frequency · Y time · Z level");
            ui.small("Orthographic views");
            egui::Grid::new("camera-presets").num_columns(3).show(ui, |ui| {
                for (i, (name, axis, positive)) in [
                    ("Front", Axis::Y, false), ("Right", Axis::X, true), ("Top", Axis::Z, true),
                    ("Back", Axis::Y, true), ("Left", Axis::X, false), ("Bottom", Axis::Z, false),
                ].into_iter().enumerate() {
                    if ui.add_sized([58.0, 24.0], egui::Button::new(name))
                        .help_text(format!("Snap to the {name} orthographic view. Keeps zoom and pan; stops auto-orbit."))
                        .clicked() {
                        self.snap_view(axis.angles(positive));
                    }
                    if i % 3 == 2 { ui.end_row(); }
                }
            });
            egui::ComboBox::from_id_salt("isometric-views")
                .selected_text("Snap isometric…").width(190.0).show_ui(ui, |ui| {
                    for above in [true, false] {
                        ui.strong(if above { "From above" } else { "From below" });
                        for (corner, name) in ["Front right", "Front left", "Back left", "Back right"].into_iter().enumerate() {
                            let angles = Self::isometric_angles(corner, above);
                            let selected = (wrap_angle(self.yaw - angles.0)).abs() < 0.001
                                && (self.elevation - angles.1).abs() < 0.001;
                            if ui.selectable_label(selected, name)
                                .help_text("True isometric view: equal foreshortening on all three axes. Keeps zoom and pan; stops auto-orbit.")
                                .clicked() {
                                self.snap_view(angles);
                            }
                        }
                    }
                }).response.help_text("Choose one of eight isometric corner views, above or below the waterfall.");
            ui.separator();
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
                            self.auto_orbit = false;
                        }
                        ui.end_row();
                    }
                });
            if ui.add(egui::Slider::new(&mut self.zoom, MIN_ZOOM..=MAX_ZOOM).logarithmic(true).text("Zoom"))
                .help_text("Magnify the view from 0.5× to 10× without changing history or geometry. Scroll over the waterfall to zoom; right-drag or Shift-drag to pan around a close-up.").changed() {
                self.auto_orbit = false;
            }
            ui.horizontal(|ui| {
                if ui
                    .small_button("Center view")
                    .help_text("Reset pan; keep rotation and zoom")
                    .clicked()
                {
                    self.pan = Vec2::ZERO;
                    self.auto_orbit = false;
                }
                if ui.small_button("Reset camera").help_text("Restore the default rotation, zoom, and pan. Audio history and geometry are unchanged.").clicked() {
                    self.reset_camera();
                }
            });
            ui.separator();
            ui.strong("Auto-orbit");
            ui.checkbox(&mut self.auto_orbit, "Enable auto-orbit").help_text("Slowly orbit around the vertical level axis while keeping elevation, zoom, and pan. Manual camera movement or choosing a view stops the orbit. Independent of history duration and analysis rate.");
            ui.add_enabled_ui(self.auto_orbit, |ui| {
                ui.add(egui::Slider::new(&mut self.orbit_speed, 1.0..=30.0).text("Speed °/s")).help_text("Camera rotation in degrees per second. Does not affect waterfall scrolling or audio.");
                ui.checkbox(&mut self.orbit_reverse, "Reverse direction").help_text("Orbit in the opposite direction at the same speed.");
            });
        });
        settings_panel(ui, "Time & History", |ui| {
            ui.add(
                egui::Slider::new(&mut self.seconds, MIN_HISTORY_SECONDS..=MAX_HISTORY_SECONDS)
                    .logarithmic(true)
                    .text("History s"),
            ).help_text("How many seconds of recent audio are displayed. Does not change the waterfall's physical length.");
        });
        settings_panel(ui, "Geometry", |ui| {
            ui.add(egui::Slider::new(&mut self.length_x, MIN_LENGTH..=MAX_LENGTH).text("Length X"))
                .help_text("Stretch or compress the frequency axis visually. 1 is the default size; frequency range and audio are unchanged.");
            ui.add(egui::Slider::new(&mut self.length_y, MIN_LENGTH..=MAX_LENGTH).text("Length Y"))
                .help_text("Stretch or compress the time axis visually. 1 is the default size; history duration and audio are unchanged.");
            ui.add(egui::Slider::new(&mut self.height, 0.0..=MAX_HEIGHT).text("Height"))
                .help_text("Scale signal peaks vertically above the fixed floor grid.");
            ui.add(egui::Slider::new(&mut self.detail, 24..=128).text("Time slices"))
                .help_text("Number of displayed time slices. More slices add detail and rendering work; history duration is unchanged.");
        });
        settings_panel(ui, "Frequency Range", |ui| {
            self.history.data.settings.range_controls(ui)
        });
        settings_panel(ui, "Signal Response", |ui| {
            self.history.data.settings.response_controls(ui)
        });
        settings_panel(ui, "Appearance", |ui| {
            ui.spacing_mut().slider_width = 60.0;
            ui.group(|ui| {
                ui.strong("Render style");
                let combo = egui::ComboBox::from_id_salt("render-mode")
                    .selected_text(format!("      {}", self.mode.label())).width(186.0)
                    .show_ui(ui, |ui| {
                        for mode in RenderMode::ALL {
                            let response = ui.add_sized([186.0, 28.0],
                                egui::Button::selectable(self.mode == mode, format!("      {}", mode.label())));
                            let icon_rect = Rect::from_center_size(response.rect.left_center() + Vec2::new(14.0, 0.0), Vec2::splat(22.0));
                            icons::paint(ui, icon_rect, mode.icon(), ui.style().interact(&response).fg_stroke.color);
                            if response.help_text(mode.description()).clicked() {
                                self.mode = mode;
                                ui.close();
                            }
                        }
                    });
                let icon_rect = Rect::from_center_size(combo.response.rect.left_center() + Vec2::new(14.0, 0.0), Vec2::splat(22.0));
                icons::paint(ui, icon_rect, self.mode.icon(), ui.visuals().text_color());
                combo.response.help_text(self.mode.description());
                ui.add_space(4.0);
                match self.mode {
                    RenderMode::Surface => {
                        ui.checkbox(&mut self.grid, "Surface mesh").help_text("Draw subtle grid lines over the terrain. Also hidden when Show guides is off.");
                        ui.checkbox(&mut self.surface_walls, "Base walls").help_text("Close the terrain's perimeter down to the fixed floor. Turn off for a floating sheet.");
                    }
                    RenderMode::Lines => {
                        width_control(ui, &mut self.line_width);
                        spacing_control(ui, &mut self.line_spacing, "Trace spacing", "Display every Nth time slice. Does not change history duration or audio sampling.");
                    }
                    RenderMode::YLines => {
                        width_control(ui, &mut self.y_line_width);
                        spacing_control(ui, &mut self.y_line_spacing, "Band spacing", "Display every Nth frequency trace. Does not change FFT resolution.");
                    }
                    RenderMode::Wireframe => {
                        width_control(ui, &mut self.wire_width);
                        spacing_control(ui, &mut self.wire_spacing, "Mesh spacing", "Display every Nth frequency and time grid line; keeps the outer edges.");
                    }
                    RenderMode::Dots => {
                        ui.add(egui::Slider::new(&mut self.dot_size, 1.0..=12.0).text("Dot size px")).help_text("Screen-space diameter of each dot, independent of camera zoom.");
                        spacing_control(ui, &mut self.dot_spacing, "Point spacing", "Display every Nth band and time slice for a more open point cloud.");
                    }
                    RenderMode::Stems => {
                        width_control(ui, &mut self.stem_width);
                        spacing_control(ui, &mut self.stem_spacing, "Stem spacing", "Display every Nth band and time slice. Wider spacing makes individual pins easier to see.");
                    }
                }
            });
            ui.add_space(6.0);
            ui.group(|ui| {
                ui.strong("Color & level");
                self.palette.controls(ui);
                self.palette.contrast_controls(ui, &mut self.contrast);
                self.history.data.settings.level_controls(ui);
            });
            ui.add_space(6.0);
            ui.group(|ui| {
                ui.strong("Viewport guides");
                ui.checkbox(&mut self.guides, "Show guides").help_text("Master switch: hide floor and surface grids, all axes and labels, navigation hints, and the viewport gizmo. Your individual guide settings are remembered.");
                ui.add_enabled_ui(self.guides, |ui| {
                    ui.checkbox(&mut self.floor_grid, "Floor grid").help_text("Reference grid beneath the waterfall.");
                    ui.checkbox(&mut self.axes, "Axes & labels").help_text("Frequency/time/level axis lines, tick labels, and navigation text. Note labels are configured under Frequency Range.");
                    ui.checkbox(&mut self.show_gizmo, "Navigation gizmo").help_text("Show the small clickable axis navigator in the viewport. The sidebar camera controls remain available when hidden.");
                });
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
        self.history.update(frame, now, live);
        self.advance_camera(now);
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
            self.auto_orbit = false;
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
        for step in 0..if self.guides && self.floor_grid { 9 } else { 0 } {
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
                vertices.push((
                    pos,
                    depth,
                    self.palette
                        .color_with_contrast(intensity, self.contrast, theme),
                ));
            }
        }
        let mut mesh = egui::Mesh::default();
        match self.mode {
            RenderMode::Dots | RenderMode::Stems => {
                let spacing = if self.mode == RenderMode::Dots {
                    self.dot_spacing
                } else {
                    self.stem_spacing
                };
                let mut points: Vec<_> = vertices
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| {
                        slices[index / BANDS].is_some()
                            && (index / BANDS).is_multiple_of(spacing)
                            && (index % BANDS).is_multiple_of(spacing)
                    })
                    .map(|(index, vertex)| (index, *vertex))
                    .collect();
                points.sort_unstable_by(|a, b| a.1.1.total_cmp(&b.1.1));
                for (index, (position, _, color)) in points {
                    if self.mode == RenderMode::Stems {
                        let base = project(
                            -1.4 + (index % BANDS) as f32 / (BANDS - 1) as f32 * 2.8,
                            0.0,
                            1.0 - (index / BANDS) as f32 / (self.detail - 1) as f32 * 2.0,
                        );
                        mesh_segment(
                            &mut mesh,
                            base,
                            position,
                            mix(theme.background, color, 0.35),
                            color,
                            self.stem_width,
                        );
                        mesh_dot(&mut mesh, position, color, self.stem_width + 1.0);
                    } else {
                        mesh_dot(&mut mesh, position, color, self.dot_size * 0.5);
                    }
                }
            }
            RenderMode::Lines | RenderMode::YLines | RenderMode::Wireframe => {
                let mut segments = Vec::with_capacity(self.detail * BANDS * 2);
                for (row, levels) in slices.iter().enumerate() {
                    if levels.is_none() {
                        continue;
                    }
                    let row_visible = match self.mode {
                        RenderMode::Lines => row % self.line_spacing == 0,
                        RenderMode::Wireframe => {
                            row % self.wire_spacing == 0 || row + 1 == self.detail
                        }
                        _ => false,
                    };
                    if row_visible {
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
                            let spacing = if self.mode == RenderMode::YLines {
                                self.y_line_spacing
                            } else {
                                self.wire_spacing
                            };
                            if band % spacing != 0
                                && !(self.mode == RenderMode::Wireframe && band + 1 == BANDS)
                            {
                                continue;
                            }
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
                        match self.mode {
                            RenderMode::Lines => self.line_width,
                            RenderMode::YLines => self.y_line_width,
                            _ => self.wire_width,
                        },
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
                            [
                                self.guides && self.grid && row % 3 == 0,
                                self.guides && self.grid && band % 4 == 0,
                            ],
                        );
                        // Perimeter walls share the same y=0 plane as the floor.
                        // Height stretches the terrain from that fixed foundation.
                        if self.surface_walls && (row == 0 || slices[row - 1].is_none()) {
                            add_face(
                                [a, a + 1, floor_offset + a + 1, floor_offset + a],
                                [false; 2],
                            );
                        }
                        if self.surface_walls
                            && (row + 2 == self.detail || slices[row + 2].is_none())
                        {
                            let b = a + BANDS;
                            add_face(
                                [b, b + 1, floor_offset + b + 1, floor_offset + b],
                                [false; 2],
                            );
                        }
                    }
                    if !self.surface_walls {
                        continue;
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
        if self.guides && self.axes {
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
                    settings.axis_label(t, frame.sample_rate),
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
        }
        if self.guides && self.show_gizmo {
            let gizmo_rect =
                Rect::from_min_size(rect.right_top() + Vec2::new(-88.0, 6.0), Vec2::splat(80.0));
            self.orientation_gizmo(ui, gizmo_rect, true);
        }
    }
}

fn width_control(ui: &mut egui::Ui, width: &mut f32) {
    ui.add(egui::Slider::new(width, 0.5..=5.0).text("Thickness px"))
        .help_text("Screen-space line thickness, independent of camera zoom. Remembered separately for each render style.");
}

fn spacing_control(ui: &mut egui::Ui, spacing: &mut usize, name: &str, description: &str) {
    ui.add(egui::Slider::new(spacing, 1..=8).text(name))
        .help_text(description);
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

fn mesh_dot(mesh: &mut egui::Mesh, center: Pos2, color: egui::Color32, radius: f32) {
    // Small screen-space discs stay legible at every zoom and share one GPU mesh.
    const SIDES: usize = 8;
    let offset = mesh.vertices.len() as u32;
    mesh.colored_vertex(center, color);
    for side in 0..SIDES {
        let angle = side as f32 * std::f32::consts::TAU / SIDES as f32;
        mesh.colored_vertex(center + Vec2::angled(angle) * radius, color);
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
    if delta.length_sq() < 1.0e-8 {
        // End-on traces still need a finite, visible cap in exact axis views.
        let offset = mesh.vertices.len() as u32;
        for corner in [
            Vec2::new(-1.0, -1.0),
            Vec2::new(1.0, -1.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(-1.0, 1.0),
        ] {
            mesh.colored_vertex(a + corner * width * 0.5, color_a);
        }
        mesh.add_triangle(offset, offset + 1, offset + 2);
        mesh.add_triangle(offset, offset + 2, offset + 3);
        return;
    }
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

    fn draw_frame(waterfall: &mut Waterfall, now: Instant) -> egui::FullOutput {
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
        output
    }

    fn populated_waterfall(now: Instant) -> Waterfall {
        let mut waterfall = Waterfall {
            detail: 24,
            seconds: 0.1,
            ..Waterfall::default()
        };
        waterfall
            .history
            .rows
            .push_back(super::super::frequency::HistoryRow {
                time: now,
                sequence: 1,
                levels: [-45.0; BANDS],
                magnitudes: vec![],
            });
        waterfall
    }

    fn controls_frame(
        waterfall: &mut Waterfall,
        context: &egui::Context,
        section: super::super::SettingsSection,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        let mut width = 0.0;
        let mut output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(600.0, 1400.0))),
                events,
                ..Default::default()
            },
            |ui| {
                let mut child = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(Rect::from_min_size(Pos2::ZERO, Vec2::new(212.0, 1400.0))),
                );
                child.spacing_mut().item_spacing = Vec2::splat(6.0);
                child.spacing_mut().slider_width = 90.0;
                child.spacing_mut().button_padding = Vec2::new(7.0, 4.0);
                child.spacing_mut().interact_size.y = 24.0;
                for style in [egui::TextStyle::Body, egui::TextStyle::Button] {
                    child
                        .style_mut()
                        .text_styles
                        .insert(style, egui::FontId::proportional(13.0));
                }
                child.data_mut(|data| {
                    data.insert_temp(child.id().with("active-module-section"), section)
                });
                waterfall.controls(&mut child);
                width = child.min_rect().width();
            },
        );
        output.textures_delta.clear();
        assert!(
            width <= 212.1,
            "{section:?} settings overflow the sidebar: {width}"
        );
        output
    }

    fn text_position(output: &egui::FullOutput, label: &str) -> Pos2 {
        output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.job.text.trim() == label => {
                    Some(text.pos + text.galley.size() * 0.5)
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("missing label {label}"))
    }

    fn click_control(
        waterfall: &mut Waterfall,
        context: &egui::Context,
        section: super::super::SettingsSection,
        position: Pos2,
    ) -> egui::FullOutput {
        let mut output = None;
        for pressed in [true, false] {
            output = Some(controls_frame(
                waterfall,
                context,
                section,
                vec![
                    egui::Event::PointerMoved(position),
                    egui::Event::PointerButton {
                        pos: position,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            ));
        }
        output.unwrap()
    }

    #[test]
    fn appearance_dropdown_shows_only_mode_parameters_and_keeps_their_values() {
        use super::super::SettingsSection;
        let context = egui::Context::default();
        let mut waterfall = Waterfall {
            palette: Palette::Heatmap,
            line_width: 3.5,
            dot_size: 8.0,
            ..Waterfall::default()
        };
        let section = SettingsSection::Appearance;
        controls_frame(&mut waterfall, &context, section, vec![]);
        for target in [
            RenderMode::Dots,
            RenderMode::Stems,
            RenderMode::Lines,
            RenderMode::YLines,
            RenderMode::Wireframe,
            RenderMode::Surface,
        ] {
            let closed = controls_frame(&mut waterfall, &context, section, vec![]);
            assert!(!closed.shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.job.text == "Length X")));
            let current = text_position(&closed, waterfall.mode.label());
            click_control(&mut waterfall, &context, section, current);
            let menu = controls_frame(&mut waterfall, &context, section, vec![]);
            // Entries are in the popup layer, after the closed selector's text.
            let target_pos = menu
                .shapes
                .iter()
                .rev()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.job.text.trim() == target.label() => {
                        Some(text.pos + text.galley.size() * 0.5)
                    }
                    _ => None,
                })
                .unwrap();
            click_control(&mut waterfall, &context, section, target_pos);
            assert_eq!(waterfall.mode, target);
            let selected = controls_frame(&mut waterfall, &context, section, vec![]);
            let labels: Vec<_> = selected
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) => Some(text.galley.job.text.as_str()),
                    _ => None,
                })
                .collect();
            assert_eq!(labels.contains(&"Dot size px"), target == RenderMode::Dots);
            assert_eq!(
                labels.contains(&"Base walls"),
                target == RenderMode::Surface
            );
            assert_eq!(
                labels.contains(&"Stem spacing"),
                target == RenderMode::Stems
            );
            assert!(labels.contains(&"Contrast"));
        }
        assert_eq!(waterfall.line_width, 3.5);
        assert_eq!(waterfall.dot_size, 8.0);
        for section in [
            SettingsSection::Camera,
            SettingsSection::Geometry,
            SettingsSection::History,
            SettingsSection::Frequency,
            SettingsSection::Response,
        ] {
            controls_frame(&mut waterfall, &context, section, vec![]);
        }
    }

    #[test]
    fn camera_buttons_snap_explicitly_and_stop_auto_orbit() {
        use super::super::SettingsSection;
        let context = egui::Context::default();
        let mut waterfall = Waterfall {
            zoom: 2.0,
            pan: Vec2::new(0.1, 0.2),
            ..Waterfall::default()
        };
        let section = SettingsSection::Camera;
        controls_frame(&mut waterfall, &context, section, vec![]);
        for (name, axis, positive) in [
            ("Top", Axis::Z, true),
            ("Top", Axis::Z, true),
            ("Bottom", Axis::Z, false),
            ("Back", Axis::Y, true),
            ("Left", Axis::X, false),
        ] {
            waterfall.auto_orbit = true;
            let output = controls_frame(&mut waterfall, &context, section, vec![]);
            click_control(
                &mut waterfall,
                &context,
                section,
                text_position(&output, name),
            );
            assert_eq!((waterfall.yaw, waterfall.elevation), axis.angles(positive));
            assert!(!waterfall.auto_orbit);
            assert_eq!(waterfall.zoom, 2.0);
            assert_eq!(waterfall.pan, Vec2::new(0.1, 0.2));
        }
    }

    #[test]
    fn master_guides_switch_removes_every_reference_without_changing_history() {
        let now = Instant::now();
        let mut waterfall = populated_waterfall(now);
        waterfall.guides = false;
        let hidden = draw_frame(&mut waterfall, now);
        assert!(
            hidden
                .shapes
                .iter()
                .all(|shape| matches!(shape.shape, egui::Shape::Mesh(_) | egui::Shape::Noop))
        );
        let hidden_mesh = hidden
            .shapes
            .iter()
            .find_map(|shape| {
                if let egui::Shape::Mesh(mesh) = &shape.shape {
                    Some(mesh)
                } else {
                    None
                }
            })
            .unwrap();
        waterfall.guides = true;
        let shown = draw_frame(&mut waterfall, now);
        assert!(
            shown
                .shapes
                .iter()
                .any(|shape| matches!(shape.shape, egui::Shape::Text(_)))
        );
        let shown_mesh = shown
            .shapes
            .iter()
            .find_map(|shape| {
                if let egui::Shape::Mesh(mesh) = &shape.shape {
                    Some(mesh)
                } else {
                    None
                }
            })
            .unwrap();
        assert!(
            shown_mesh.vertices.len() > hidden_mesh.vertices.len(),
            "surface mesh overlay is also hidden"
        );
        waterfall.floor_grid = false;
        waterfall.axes = false;
        waterfall.show_gizmo = false;
        let individual = draw_frame(&mut waterfall, now);
        assert!(
            individual
                .shapes
                .iter()
                .all(|shape| matches!(shape.shape, egui::Shape::Mesh(_) | egui::Shape::Noop))
        );
        assert_eq!(waterfall.history.rows.len(), 1);
        assert_eq!(waterfall.history.rows[0].time, now);
    }

    #[test]
    fn styles_and_contrast_change_only_rendering_and_keep_finite_meshes() {
        let now = Instant::now();
        let mut waterfall = populated_waterfall(now);
        waterfall.guides = false;
        waterfall.palette = Palette::Heatmap;
        waterfall.history.rows[0].levels = [-67.5; BANDS];
        for mode in RenderMode::ALL {
            waterfall.mode = mode;
            let mesh_for = |waterfall: &mut Waterfall| {
                draw_frame(waterfall, now)
                    .shapes
                    .into_iter()
                    .find_map(|shape| {
                        if let egui::Shape::Mesh(mesh) = shape.shape {
                            Some(mesh)
                        } else {
                            None
                        }
                    })
                    .unwrap()
            };
            waterfall.contrast = 1.0;
            let original = mesh_for(&mut waterfall);
            waterfall.contrast = 3.0;
            let recolored = mesh_for(&mut waterfall);
            assert!(original.is_valid() && recolored.is_valid());
            assert_eq!(original.vertices.len(), recolored.vertices.len());
            assert!(
                original
                    .vertices
                    .iter()
                    .zip(&recolored.vertices)
                    .all(|(a, b)| a.pos == b.pos)
            );
            assert!(
                original
                    .vertices
                    .iter()
                    .zip(&recolored.vertices)
                    .any(|(a, b)| a.color != b.color)
            );
            for (yaw, elevation) in [
                Axis::Z.angles(true),
                Axis::Z.angles(false),
                Axis::X.angles(true),
                Waterfall::isometric_angles(0, true),
            ] {
                waterfall.snap_view((yaw, elevation));
                let mesh = mesh_for(&mut waterfall);
                assert!(
                    mesh.vertices
                        .iter()
                        .all(|v| v.pos.x.is_finite() && v.pos.y.is_finite())
                );
            }
            assert_eq!(waterfall.history.rows.len(), 1);
            assert_eq!(waterfall.history.rows[0].levels, [-67.5; BANDS]);
        }
    }

    #[test]
    fn mode_spacing_and_sizes_control_the_mesh_without_changing_history() {
        let now = Instant::now();
        let mut waterfall = populated_waterfall(now);
        let count = |waterfall: &mut Waterfall| {
            draw_frame(waterfall, now)
                .shapes
                .iter()
                .filter_map(|shape| {
                    if let egui::Shape::Mesh(mesh) = &shape.shape {
                        Some(mesh.vertices.len())
                    } else {
                        None
                    }
                })
                .sum::<usize>()
        };
        for mode in RenderMode::ALL {
            waterfall.mode = mode;
            let dense = count(&mut waterfall);
            match mode {
                RenderMode::Surface => waterfall.surface_walls = false,
                RenderMode::Lines => waterfall.line_spacing = 4,
                RenderMode::YLines => waterfall.y_line_spacing = 4,
                RenderMode::Wireframe => waterfall.wire_spacing = 4,
                RenderMode::Dots => waterfall.dot_spacing = 4,
                RenderMode::Stems => waterfall.stem_spacing = 8,
            }
            assert!(count(&mut waterfall) < dense, "{mode:?}");
        }
        let mut mesh = egui::Mesh::default();
        mesh_dot(&mut mesh, Pos2::ZERO, egui::Color32::WHITE, 6.0);
        assert!((mesh.vertices[1].pos.distance(Pos2::ZERO) - 6.0).abs() < 0.001);
        assert_eq!(waterfall.history.rows.len(), 1);
    }

    #[test]
    fn isometric_views_have_equal_axis_lengths_and_snaps_preserve_framing() {
        let mut waterfall = Waterfall {
            zoom: 3.0,
            pan: Vec2::new(0.1, -0.2),
            auto_orbit: true,
            ..Waterfall::default()
        };
        let mut distinct = std::collections::HashSet::new();
        for above in [true, false] {
            for corner in 0..4 {
                let (yaw, elevation) = Waterfall::isometric_angles(corner, above);
                distinct.insert((yaw.to_bits(), elevation.to_bits()));
                let lengths = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]].map(|axis| {
                    camera_gizmo::project_direction(yaw, elevation, axis)
                        .0
                        .length()
                });
                assert!((lengths[0] - lengths[1]).abs() < 1.0e-5);
                assert!((lengths[1] - lengths[2]).abs() < 1.0e-5);
                waterfall.snap_view((yaw, elevation));
                assert!(!waterfall.auto_orbit);
                assert_eq!(waterfall.zoom, 3.0);
                assert_eq!(waterfall.pan, Vec2::new(0.1, -0.2));
            }
        }
        assert_eq!(distinct.len(), 8);
        for axis in [Axis::X, Axis::Y, Axis::Z] {
            for sign in [true, false] {
                let angles = axis.angles(sign);
                waterfall.snap_view(angles);
                waterfall.snap_view(angles);
                assert_eq!(
                    (waterfall.yaw, waterfall.elevation),
                    angles,
                    "named presets never toggle to their opposite"
                );
            }
        }
    }

    #[test]
    fn auto_orbit_is_time_based_reversible_and_stops_for_manual_camera_actions() {
        use std::time::Duration;
        let now = Instant::now();
        for fps in [15, 30, 60, 120] {
            let mut waterfall = Waterfall {
                auto_orbit: true,
                orbit_speed: 12.0,
                ..Waterfall::default()
            };
            let initial = waterfall.yaw;
            let elevation = waterfall.elevation;
            waterfall.advance_camera(now);
            for step in 1..=fps {
                waterfall.advance_camera(now + Duration::from_secs_f64(step as f64 / fps as f64));
            }
            assert!((waterfall.yaw - initial - 12.0_f32.to_radians()).abs() < 0.0001);
            assert_eq!(waterfall.elevation, elevation);
            waterfall.orbit_reverse = true;
            waterfall.advance_camera(now + Duration::from_millis(1100));
            assert!((waterfall.yaw - initial - 10.8_f32.to_radians()).abs() < 0.0001);
            let before_gap = waterfall.yaw;
            waterfall.advance_camera(now + Duration::from_secs(60));
            assert!(
                (waterfall.yaw - before_gap).abs() < 0.022,
                "no jump after hidden pane or suspend"
            );
            waterfall.orbit(Vec2::new(1.0, 1.0));
            assert!(!waterfall.auto_orbit);
            waterfall.auto_orbit = true;
            waterfall.zoom_by_scroll(1.0);
            assert!(!waterfall.auto_orbit);
            waterfall.auto_orbit = true;
            waterfall.reset_camera();
            assert!(!waterfall.auto_orbit);
            assert_eq!(waterfall.seconds, DEFAULT_HISTORY_SECONDS);
        }
    }

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
