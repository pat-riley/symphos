//! Music-reactive Voronoi cells. Geometry and animation live on the UI thread;
//! the shared capture/analysis path is untouched.
use std::f32::consts::TAU;
use std::time::Instant;

use eframe::egui::{self, Pos2, Rect, Stroke, Vec2};

use super::settings_panel;
use crate::modulation::{ModulationSettings, Modulator, Target, Visualizer};
use crate::{analysis::AnalysisFrame, help::HoverHelp, parameter::Parameter, theme::AppTheme};

use crate::colors::Palette;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub(crate) struct VeroniSettings {
    #[serde(default)]
    pub(crate) modulation: Option<ModulationSettings>,
    performance_open: bool,
    brightness: f32,
    cells: u32,
    seed: u32,
    clustering: f32,
    rounding: f32,
    speed: f32,
    drift: f32,
    turbulence: f32,
    swirl: f32,
    breathing: f32,
    sensitivity: f32,
    gate_db: f32,
    attack_ms: f32,
    release_ms: f32,
    bass_motion: f32,
    mid_motion: f32,
    treble_motion: f32,
    stereo_motion: f32,
    palette: Palette,
    edge_width: f32,
    edge_opacity: f32,
    edge_pulse: f32,
    cell_fill: f32,
    point_size: f32,
    point_pulse: f32,
    glow: f32,
    link_opacity: f32,
    link_reach: f32,
    show_cells: bool,
    show_points: bool,
    show_links: bool,
}
impl Default for VeroniSettings {
    fn default() -> Self {
        Self {
            modulation: Some(ModulationSettings::for_visualizer(Visualizer::Veroni)),
            performance_open: true,
            brightness: 1.0,
            cells: 128,
            seed: 7,
            clustering: 0.65,
            rounding: 0.16,
            speed: 0.65,
            drift: 0.55,
            turbulence: 0.40,
            swirl: 0.45,
            breathing: 0.45,
            sensitivity: 1.25,
            gate_db: -66.0,
            attack_ms: 45.0,
            release_ms: 650.0,
            bass_motion: 0.85,
            mid_motion: 0.65,
            treble_motion: 0.35,
            stereo_motion: 0.30,
            palette: Palette::Lime,
            edge_width: 1.25,
            edge_opacity: 0.60,
            edge_pulse: 0.55,
            cell_fill: 0.025,
            point_size: 2.2,
            point_pulse: 1.4,
            glow: 0.55,
            link_opacity: 0.16,
            link_reach: 1.3,
            show_cells: true,
            show_points: true,
            show_links: true,
        }
    }
}
impl VeroniSettings {
    fn sanitize(&mut self) {
        self.cells = self.cells.clamp(12, 240);
        self.seed = self.seed.min(9999);
        let defaults = Self::default();
        macro_rules! bound {
            ($($field:ident: $lo:expr, $hi:expr);* $(;)?) => {$(
                self.$field = if self.$field.is_finite() {
                    self.$field.clamp($lo, $hi)
                } else { defaults.$field };
            )*};
        }
        bound! {
            brightness: 0.0, 2.0;
            clustering: 0.0, 1.0; rounding: 0.0, 0.4;
            speed: 0.0, 2.0; drift: 0.0, 1.0; turbulence: 0.0, 1.0;
            swirl: 0.0, 1.0; breathing: 0.0, 1.0;
            sensitivity: 0.0, 4.0; gate_db: -90.0, -20.0;
            attack_ms: 5.0, 500.0; release_ms: 50.0, 3000.0;
            bass_motion: 0.0, 2.0; mid_motion: 0.0, 2.0;
            treble_motion: 0.0, 2.0; stereo_motion: 0.0, 2.0;
            edge_width: 0.25, 4.0; edge_opacity: 0.0, 1.0; edge_pulse: 0.0, 2.0;
            cell_fill: 0.0, 0.3; point_size: 0.5, 6.0; point_pulse: 0.0, 3.0;
            glow: 0.0, 1.0; link_opacity: 0.0, 0.6; link_reach: 0.5, 3.0;
        }
        let modulation = self.modulation.get_or_insert_with(|| {
            let mut m = ModulationSettings::migrate(
                Visualizer::Veroni,
                self.sensitivity,
                self.gate_db,
                self.attack_ms,
                self.release_ms,
            );
            for r in &mut m.routes {
                match r.target {
                    Target::Expansion => r.amount = self.bass_motion,
                    Target::Swirl => r.amount = self.mid_motion,
                    Target::Turbulence => r.amount = self.treble_motion,
                    Target::Steering => r.amount = self.stereo_motion,
                    _ => {}
                }
            }
            m
        });
        modulation.sanitize(Visualizer::Veroni);
    }
}

#[derive(Default)]
pub struct Veroni {
    modulator: Modulator,
    pub(crate) settings: VeroniSettings,
    last_time: Option<Instant>,
    phase: f64,
    // Low, mid, high and broadband energy envelopes; never serialized.
    energy: [f32; 4],
    balance: f32,
}
impl Veroni {
    pub(crate) fn macro_controls() -> [super::performance::Control; 12] {
        use super::performance::Control;
        [
            Control::new("Brightness", Target::Brightness, 0.0_f32, 2.0_f32, 1.0_f32),
            Control::new("Speed", Target::Motion, 0.0_f32, 2.0_f32, 0.65_f32),
            Control::new("Expansion", Target::Expansion, 0.0_f32, 1.0_f32, 0.45_f32),
            Control::new("Swirl", Target::Swirl, 0.0_f32, 1.0_f32, 0.45_f32),
            Control::new("Turbulence", Target::Turbulence, 0.0_f32, 1.0_f32, 0.4_f32),
            Control::new("Drift", Target::Drift, 0.0_f32, 1.0_f32, 0.55_f32),
            Control::new("Clustering", Target::Magnetism, 0.0_f32, 1.0_f32, 0.65_f32),
            Control::new("Rounding", Target::Shape, 0.0_f32, 0.4_f32, 0.16_f32),
            Control::new("Points", Target::Size, 0.5_f32, 6.0_f32, 2.2_f32),
            Control::new("Edges", Target::Depth, 0.25_f32, 4.0_f32, 1.25_f32),
            Control::new("Links", Target::Trails, 0.0_f32, 0.6_f32, 0.16_f32),
            Control::new("Glow", Target::Glow, 0.0_f32, 1.0_f32, 0.55_f32),
        ]
    }
    fn macro_values(s: &VeroniSettings) -> [f32; 12] {
        [
            s.brightness,
            s.speed,
            s.breathing,
            s.swirl,
            s.turbulence,
            s.drift,
            s.clustering,
            s.rounding,
            s.point_size,
            s.edge_width,
            s.link_opacity,
            s.glow,
        ]
    }
    fn set_macro_values(s: &mut VeroniSettings, v: [f32; 12]) {
        s.brightness = v[0];
        s.speed = v[1];
        s.breathing = v[2];
        s.swirl = v[3];
        s.turbulence = v[4];
        s.drift = v[5];
        s.clustering = v[6];
        s.rounding = v[7];
        s.point_size = v[8];
        s.edge_width = v[9];
        s.link_opacity = v[10];
        s.glow = v[11];
    }
    fn macro_effective(&self) -> [f32; 12] {
        let controls = Self::macro_controls();
        let values = Self::macro_values(&self.settings);
        std::array::from_fn(|i| {
            let c = controls[i];
            let modulation = self.modulator.value(c.target);
            if i == 0 && values[i] == 0.0 {
                0.0
            } else {
                c.effective(values[i], modulation)
            }
        })
    }
    fn effective_settings(&self) -> VeroniSettings {
        let mut s = self.settings.clone();
        let values = self.macro_effective();
        // Layer banks are evaluated individually in surface_with_response, never collapsed here.

        Self::set_macro_values(&mut s, values);
        s
    }
    #[cfg(test)]
    #[allow(dead_code)]
    pub(crate) fn toggle_performance(&mut self) {
        self.settings.performance_open = !self.settings.performance_open;
    }
    #[cfg(test)]
    #[allow(dead_code)]
    pub(crate) fn performance_height(&self, _width: f32) -> f32 {
        super::performance::height(self.settings.performance_open)
    }
    #[cfg(test)]
    #[allow(dead_code)]
    pub(crate) fn performance_bar(
        &mut self,
        ui: &mut egui::Ui,
        rect: Rect,
        theme: &AppTheme,
        pane: usize,
    ) -> bool {
        self.settings.sanitize();
        let controls = Self::macro_controls();
        let mut values = Self::macro_values(&self.settings);
        let effective = self.macro_effective();
        let action = super::performance::bar(
            ui,
            rect,
            theme,
            pane,
            "Veroni",
            &controls,
            &mut values,
            effective,
            self.settings.modulation.as_mut().unwrap(),
            &mut self.settings.performance_open,
            &["Breathing cells", "Electric lattice", "Slow drift"],
        );
        Self::set_macro_values(&mut self.settings, values);
        if let Some(i) = action.preset {
            let defaults = Self::macro_values(&VeroniSettings::default());
            let mut values = defaults;
            values[1] = match i {
                1 => 1.1,
                2 => 0.18,
                _ => 0.6,
            };
            values[4] = match i {
                1 => 0.7,
                2 => 0.12,
                _ => defaults[4],
            };
            values[11] = if i == 1 { 0.7 } else { 0.4 };
            Self::set_macro_values(&mut self.settings, values);
            super::performance::preset_routes(
                ui,
                Visualizer::Veroni,
                i,
                &controls,
                self.settings.modulation.as_mut().unwrap(),
            );
        }
        if let Some(i) = action.selected {
            self.modulator.select_target(
                controls[i].target,
                self.settings.modulation.as_ref().unwrap(),
            );
        }
        action.inspect
    }

    pub(crate) fn settings_snapshot(&self) -> VeroniSettings {
        self.settings.clone()
    }
    pub(crate) fn apply_settings(&mut self, settings: &VeroniSettings) {
        self.modulator = Modulator::default();
        self.settings = settings.clone();
        self.settings.sanitize();
    }
    pub fn clear(&mut self) {
        self.modulator = Modulator::default();
        self.last_time = None;
        self.phase = 0.0;
        self.energy = [0.0; 4];
        self.balance = 0.0;
    }
    pub fn reset_settings(&mut self) {
        self.settings = VeroniSettings::default();
        self.clear();
    }

    pub fn controls(&mut self, ui: &mut egui::Ui, frame: &AnalysisFrame) {
        self.settings.sanitize();
        let s = &mut self.settings;
        macro_rules! param {
            ($ui:ident, $field:ident, $range:expr, $label:expr, $help:expr) => {
                $ui.add(
                    Parameter::new(&mut s.$field, $range, VeroniSettings::default().$field)
                        .key(stringify!($field))
                        .text($label),
                )
                .help_text($help);
            };
        }
        settings_panel(ui, "Geometry", |ui| {
            param!(
                ui,
                cells,
                12..=240,
                "Cell count",
                "Number of moving sites. Higher counts create finer cells and require more rendering work."
            );
            param!(
                ui,
                seed,
                0..=9999,
                "Pattern seed",
                "Choose a repeatable arrangement. Saved in component and full-view presets."
            );
            param!(
                ui,
                clustering,
                0.0..=1.0,
                "Clustering",
                "Gather points around flowing centers, creating dense small cells and large open cells."
            );
            param!(
                ui,
                rounding,
                0.0..=0.4,
                "Corner softness",
                "Round the cell outlines. Zero shows the exact straight-edged Voronoi polygons."
            );
        });
        settings_panel(ui, "Flow & Motion", |ui| {
            param!(
                ui,
                speed,
                0.0..=2.0,
                "Flow speed",
                "Rate of the continuous flow field. Zero stops its clock; audio can still deform the cells."
            );
            param!(
                ui,
                drift,
                0.0..=1.0,
                "Drift",
                "Broad, smooth travel of the points across the surface."
            );
            param!(
                ui,
                turbulence,
                0.0..=1.0,
                "Turbulence",
                "Smaller overlapping currents; mids and highs add motion to this field."
            );
            param!(
                ui,
                swirl,
                0.0..=1.0,
                "Swirl",
                "Orbit points around their moving cluster centers."
            );
            param!(
                ui,
                breathing,
                0.0..=1.0,
                "Breathing",
                "Slow expansion and contraction, with additional bass-driven displacement."
            );
        });
        settings_panel(ui, "Audio mappings", |ui| {
            self.modulator.editor(
                ui,
                frame,
                s.modulation.as_mut().unwrap(),
                Visualizer::Veroni,
                1,
            );
        });
        settings_panel(ui, "Appearance", |ui| {
            s.palette.controls(ui);
            ui.checkbox(&mut s.show_cells, "Cell outlines");
            param!(
                ui,
                edge_width,
                0.25..=4.0,
                "Edge width",
                "Base line thickness in screen points; large cells have stronger outlines."
            );
            param!(
                ui,
                edge_opacity,
                0.0..=1.0,
                "Edge opacity",
                "Visibility of the cellular mesh."
            );
            param!(
                ui,
                edge_pulse,
                0.0..=2.0,
                "Edge response",
                "Audio-driven brightness and thickness of the cell boundaries."
            );
            param!(
                ui,
                cell_fill,
                0.0..=0.3,
                "Cell tint",
                "Subtle translucent color inside each cell."
            );
            ui.checkbox(&mut s.show_points, "Moving points");
            param!(
                ui,
                point_size,
                0.5..=6.0,
                "Point size",
                "Base radius of each moving site in screen points."
            );
            param!(
                ui,
                point_pulse,
                0.0..=3.0,
                "Point response",
                "Audio-driven changes in the size and brightness of the sites."
            );
            param!(
                ui,
                glow,
                0.0..=1.0,
                "Glow",
                "Soft layered halos around points and cell edges."
            );
            ui.checkbox(&mut s.show_links, "Point connections");
            param!(
                ui,
                link_opacity,
                0.0..=0.6,
                "Link opacity",
                "Fine lines joining each site to up to three nearby sites."
            );
            param!(
                ui,
                link_reach,
                0.5..=3.0,
                "Link reach",
                "Connection distance relative to cell density. Short links emphasize clusters."
            );
        });
    }

    fn advance(&mut self, now: Instant, frame: &AnalysisFrame, live: bool) {
        if self.settings.modulation.is_none() {
            self.settings.sanitize();
        }
        let dt = self.last_time.map_or(0.0, |last| {
            now.saturating_duration_since(last).as_secs_f32().min(0.05)
        });
        self.last_time = Some(now);
        if dt == 0.0 {
            return;
        }
        self.modulator.advance(
            frame,
            live,
            self.settings.modulation.as_ref().unwrap(),
            dt,
            1,
        );
        self.energy = [
            Target::Expansion,
            Target::Swirl,
            Target::Turbulence,
            Target::Brightness,
        ]
        .map(|t| self.modulator.value(t));
        self.balance = self.modulator.value(Target::Steering);
        self.phase += f64::from(
            dt * self.macro_effective()[1]
                * self.settings.modulation.as_ref().unwrap().ambient.max(0.05),
        );
    }

    fn sites(&self, size: Vec2) -> Vec<Pos2> {
        let effective = self.effective_settings();
        let s = &effective;
        let t = self.phase;
        let sin = |speed: f64, phase: f32| (t * speed + f64::from(phase)).sin() as f32;
        let cos = |speed: f64, phase: f32| (t * speed + f64::from(phase)).cos() as f32;
        (0..s.cells)
            .map(|i| {
                let h = |salt| random(i.wrapping_add(s.seed.wrapping_mul(7919)), salt);
                let group = i % 7;
                let g = group as f32 * 2.399_963;
                let center = Vec2::new(0.5 + 0.33 * sin(0.37, g), 0.5 + 0.33 * cos(0.29, g + 1.4));
                let base = Vec2::new(h(1), h(2));
                let angle = h(3) * TAU + sin(0.43, g) * s.swirl;
                let radius = 0.035 + h(4).sqrt() * 0.19;
                let orbit = Vec2::angled(angle) * radius;
                // Keep a quarter of the sites spread over the field so clusters
                // leave organic pockets rather than empty slabs at the edges.
                let cluster = if i % 4 == 0 { 0.0 } else { s.clustering };
                let mut p = base + (center + orbit - base) * cluster;
                let phase = h(5) * TAU;
                let bass = 0.1;
                let highs = 0.2;
                p += (p - center) * s.breathing * (0.16 * sin(0.65, g) + bass * 0.5);
                p += Vec2::new(sin(0.51, base.y * 5.0), cos(0.47, base.x * 5.0)) * s.drift * 0.13;
                p += Vec2::new(
                    sin(1.3, phase + base.y * 9.0),
                    cos(1.1, phase + base.x * 8.0),
                ) * s.turbulence
                    * (0.025 + highs * 0.055);
                p.x += self.balance * 0.06;
                // Reflect at the boundary instead of wrapping (which would teleport
                // seeds and cause a discontinuous change in neighboring cells).
                Pos2::new(reflect(p.x) * size.x, reflect(p.y) * size.y)
            })
            .collect()
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
        self.advance(now, frame, live);
        let bounds = rect.shrink(12.0);
        if bounds.width() < 4.0 || bounds.height() < 4.0 {
            return;
        }
        let sites = self.sites(bounds.size());
        let effective = self.effective_settings();
        let s = &effective;
        let painter = ui.painter_at(rect);
        let average_area = bounds.area() / sites.len() as f32;
        let mut polygon = Vec::with_capacity(24);
        let mut scratch = Vec::with_capacity(24);
        for (i, &site) in sites.iter().enumerate() {
            let accent = s
                .palette
                .cellular(i as f32 / sites.len().max(1) as f32, theme);
            let local = 0.0;
            if s.show_cells || s.cell_fill > 0.0 {
                voronoi_cell(&sites, i, bounds.size(), &mut polygon, &mut scratch);
                if polygon.len() < 3 {
                    continue;
                }
                let area = polygon_area(&polygon);
                let scale = (area / average_area).sqrt().clamp(0.4, 2.6);
                let points: Vec<_> = polygon.iter().map(|p| bounds.min + p.to_vec2()).collect();
                if s.cell_fill > 0.0 {
                    painter.add(egui::Shape::convex_polygon(
                        points.clone(),
                        accent.gamma_multiply(s.brightness * s.cell_fill * (0.4 + local)),
                        Stroke::NONE,
                    ));
                }
                if s.show_cells {
                    let path = rounded_polygon(&points, s.rounding);
                    let opacity = (s.edge_opacity * (0.42 + 0.3 * scale)
                        + local * s.edge_pulse * 0.18)
                        .clamp(0.0, 1.0);
                    let width = s.edge_width
                        * (0.55 + 0.4 * scale)
                        * (1.0 + local * s.edge_pulse * 0.4).max(0.05);
                    if s.glow > 0.0 {
                        painter.add(egui::Shape::closed_line(
                            path.clone(),
                            Stroke::new(
                                width + 3.0,
                                theme
                                    .foreground
                                    .gamma_multiply(s.brightness * opacity * s.glow * 0.07),
                            ),
                        ));
                    }
                    painter.add(egui::Shape::closed_line(
                        path,
                        Stroke::new(
                            width,
                            theme.foreground.gamma_multiply(s.brightness * opacity),
                        ),
                    ));
                }
            }
            if s.show_links {
                let reach = average_area.sqrt() * s.link_reach * 1.5;
                // A bounded nearest-neighbor graph, independent of topology
                // changes in the rounded borders.
                let mut nearest = [(f32::INFINITY, usize::MAX); 3];
                for (j, other) in sites.iter().enumerate() {
                    if i == j {
                        continue;
                    }
                    let distance = site.distance(*other);
                    if distance < reach && distance < nearest[2].0 {
                        nearest[2] = (distance, j);
                        nearest.sort_by(|a, b| a.0.total_cmp(&b.0));
                    }
                }
                for (distance, j) in nearest {
                    if j == usize::MAX {
                        continue;
                    }
                    let opacity = s.link_opacity * (1.0 - distance / reach) * (0.5 + local);
                    painter.line_segment(
                        [bounds.min + site.to_vec2(), bounds.min + sites[j].to_vec2()],
                        Stroke::new(0.8, accent.gamma_multiply(s.brightness * opacity)),
                    );
                }
            }
        }
        // Points sit above all borders and links.
        if s.show_points {
            for (i, site) in sites.iter().enumerate() {
                let accent = s
                    .palette
                    .cellular(i as f32 / sites.len().max(1) as f32, theme);
                let variation = random(i as u32 + s.seed, 9);
                let energy = self.modulator.value(Target::Size) * s.point_pulse;
                let radius = s.point_size * (0.45 + variation * 0.8) * (1.0 + energy).max(0.05);
                let position = bounds.min + site.to_vec2();
                for (size, alpha) in [(3.6, 0.035), (2.4, 0.07), (1.6, 0.13)] {
                    if s.glow > 0.0 {
                        painter.circle_filled(
                            position,
                            radius * size,
                            accent.gamma_multiply(s.brightness * alpha * s.glow),
                        );
                    }
                }
                painter.circle_filled(
                    position,
                    radius,
                    accent.gamma_multiply(
                        s.brightness
                            * (0.3 + self.energy[3] * 0.7 + variation * 0.15).clamp(0.0, 1.0),
                    ),
                );
            }
        }
    }
}

fn random(index: u32, salt: u32) -> f32 {
    let mut x = index
        .wrapping_mul(747_796_405)
        .wrapping_add(salt.wrapping_mul(2_891_336_453));
    x = ((x >> ((x >> 28) + 4)) ^ x).wrapping_mul(277_803_737);
    x = (x >> 22) ^ x;
    (x >> 8) as f32 / 16_777_216.0
}
fn reflect(value: f32) -> f32 {
    0.015 + (1.0 - (value.rem_euclid(2.0) - 1.0).abs()) * 0.97
}

#[cfg(test)]
fn audio_energy(frame: &AnalysisFrame, s: &VeroniSettings) -> [f32; 4] {
    super::reactive::energy(frame, s.sensitivity, s.gate_db)
}

/// Intersect a rectangle with every perpendicular-bisector half plane.
/// Scratch buffers are reused across neighbors and cells (no per-clip allocation).
fn voronoi_cell(
    sites: &[Pos2],
    index: usize,
    size: Vec2,
    polygon: &mut Vec<Pos2>,
    scratch: &mut Vec<Pos2>,
) {
    polygon.clear();
    polygon.extend([
        Pos2::ZERO,
        Pos2::new(size.x, 0.0),
        Pos2::new(size.x, size.y),
        Pos2::new(0.0, size.y),
    ]);
    let site = sites[index];
    for (j, &other) in sites.iter().enumerate() {
        if index == j {
            continue;
        }
        let normal = other - site;
        if normal.length_sq() < 1e-10 {
            // Deterministic ownership of coincident sites; no duplicate cells.
            if j < index {
                polygon.clear();
                return;
            }
            continue;
        }
        let midpoint = site + normal * 0.5;
        scratch.clear();
        let Some(&last) = polygon.last() else {
            return;
        };
        let mut previous = last;
        let mut previous_distance = (previous - midpoint).dot(normal);
        for &current in polygon.iter() {
            let distance = (current - midpoint).dot(normal);
            if (distance <= 0.0) != (previous_distance <= 0.0) {
                let t = previous_distance / (previous_distance - distance);
                scratch.push(previous.lerp(current, t));
            }
            if distance <= 0.0 {
                scratch.push(current);
            }
            previous = current;
            previous_distance = distance;
        }
        std::mem::swap(polygon, scratch);
    }
}
fn polygon_area(points: &[Pos2]) -> f32 {
    points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .take(points.len())
        .map(|(a, b)| a.x * b.y - b.x * a.y)
        .sum::<f32>()
        .abs()
        * 0.5
}
fn rounded_polygon(points: &[Pos2], rounding: f32) -> Vec<Pos2> {
    if rounding <= 0.0 {
        return points.to_vec();
    }
    let mut result = Vec::with_capacity(points.len() * 5);
    for i in 0..points.len() {
        let corner = points[i];
        let previous = points[(i + points.len() - 1) % points.len()];
        let next = points[(i + 1) % points.len()];
        let radius = corner.distance(previous).min(corner.distance(next)) * rounding;
        let start = corner + (previous - corner).normalized() * radius;
        let end = corner + (next - corner).normalized() * radius;
        for step in 0..=4 {
            let t = step as f32 / 4.0;
            result.push(start.lerp(corner, t).lerp(corner.lerp(end, t), t));
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::FrequencyBin;
    use std::time::Duration;

    #[test]
    fn appearance_routes_do_not_reshape_cells_but_expansion_does() {
        let mut active = Veroni::default();
        active
            .settings
            .modulation
            .as_mut()
            .unwrap()
            .routes
            .retain(|r| r.target == Target::Brightness);
        let mut silent = Veroni {
            settings: active.settings.clone(),
            ..Default::default()
        };
        let start = Instant::now();
        for i in 0..=120 {
            let now = start + Duration::from_secs_f64(i as f64 / 120.0);
            active.advance(now, &signal(80.0), true);
            silent.advance(now, &AnalysisFrame::default(), true);
        }
        assert_eq!(
            active.sites(Vec2::splat(800.0)),
            silent.sites(Vec2::splat(800.0))
        );
        assert!(active.energy[3] > 0.5);
        active.settings.modulation.as_mut().unwrap().routes[0].target = Target::Expansion;
        active.advance(start + Duration::from_secs_f64(1.02), &signal(80.0), true);
        silent.advance(
            start + Duration::from_secs_f64(1.02),
            &AnalysisFrame::default(),
            true,
        );
        assert_ne!(
            active.sites(Vec2::splat(800.0)),
            silent.sites(Vec2::splat(800.0))
        );
    }

    #[test]
    fn cells_partition_the_rectangle_and_belong_to_nearest_site() {
        let size = Vec2::new(900.0, 480.0);
        let mut veroni = Veroni {
            phase: 12.3,
            energy: [0.8, 0.4, 0.6, 0.7],
            ..Default::default()
        };
        for count in [12, 128, 240] {
            veroni.settings.cells = count;
            let sites = veroni.sites(size);
            let mut polygon = Vec::new();
            let mut scratch = Vec::new();
            let mut total_area = 0.0;
            for (i, site) in sites.iter().enumerate() {
                voronoi_cell(&sites, i, size, &mut polygon, &mut scratch);
                total_area += polygon_area(&polygon);
                for p in &polygon {
                    assert!(p.x >= -0.01 && p.x <= size.x + 0.01);
                    assert!(p.y >= -0.01 && p.y <= size.y + 0.01);
                    for other in &sites {
                        assert!(p.distance_sq(*site) <= p.distance_sq(*other) + 0.5);
                    }
                }
                assert!(rounded_polygon(&polygon, 0.4).iter().all(|p| p.is_finite()));
            }
            assert!((total_area - size.x * size.y).abs() < 2.0);
        }
    }

    #[test]
    fn coincident_sites_have_one_owner_and_collinear_sites_are_valid() {
        let sites = [
            Pos2::new(25.0, 50.0),
            Pos2::new(25.0, 50.0),
            Pos2::new(75.0, 50.0),
        ];
        let mut polygon = Vec::new();
        let mut scratch = Vec::new();
        for (i, expected) in [5000.0, 0.0, 5000.0].into_iter().enumerate() {
            voronoi_cell(&sites, i, Vec2::splat(100.0), &mut polygon, &mut scratch);
            assert!((polygon_area(&polygon) - expected).abs() < 0.01);
        }
    }

    fn signal(hz: f32) -> AnalysisFrame {
        AnalysisFrame {
            bins: vec![FrequencyBin {
                frequency_hz: hz,
                magnitude: 0.2,
                dbfs: -14.0,
            }],
            rms: [0.1, 0.2],
            mix_rms: 0.15,
            ..Default::default()
        }
    }

    #[test]
    fn signal_bands_are_independent_and_silence_is_zero() {
        let settings = VeroniSettings::default();
        for (i, hz) in [80.0, 1000.0, 8000.0].into_iter().enumerate() {
            let energy = audio_energy(&signal(hz), &settings);
            for (band, value) in energy[..3].iter().enumerate() {
                if band == i {
                    assert!(*value > 0.5);
                } else {
                    assert_eq!(*value, 0.0);
                }
            }
            assert!(energy[3] > 0.4);
        }
        assert_eq!(audio_energy(&AnalysisFrame::default(), &settings), [0.0; 4]);
        let mut quiet = signal(80.0);
        quiet.bins[0].magnitude = 1e-6;
        quiet.mix_rms = 1e-6;
        assert_eq!(audio_energy(&quiet, &settings), [0.0; 4]);
        let mut insensitive = settings;
        insensitive.sensitivity = 0.0;
        assert_eq!(audio_energy(&signal(80.0), &insensitive), [0.0; 4]);
    }

    #[test]
    fn audio_deforms_the_geometry_and_pause_holds_it_still() {
        let now = Instant::now();
        let mut active = Veroni::default();
        let mut silent = Veroni::default();
        active.settings.speed = 0.0;
        silent.settings.speed = 0.0;
        for i in 0..60 {
            let time = now + Duration::from_millis(i * 16);
            active.advance(time, &signal(80.0), true);
            silent.advance(time, &AnalysisFrame::default(), true);
        }
        let size = Vec2::new(800.0, 500.0);
        let moving = active.sites(size);
        assert!(
            moving
                .iter()
                .zip(silent.sites(size))
                .any(|(a, b)| a.distance(b) > 2.0)
        );
        let frozen_time = active.last_time.unwrap();
        let energy = active.energy;
        for _ in 0..100 {
            active.advance(frozen_time, &signal(8000.0), true);
        }
        assert_eq!(active.energy, energy);
        assert_eq!(active.sites(size), moving);
        for i in 1..=300 {
            active.advance(
                frozen_time + Duration::from_millis(i * 16),
                &signal(80.0),
                false,
            );
        }
        assert!(active.energy.iter().all(|value| *value < 0.001));
    }

    #[test]
    fn response_is_frame_rate_independent_and_settings_exclude_runtime() {
        let start = Instant::now();
        let mut fast = Veroni::default();
        let mut slow = Veroni::default();
        for i in 0..=120 {
            fast.advance(
                start + Duration::from_secs_f64(i as f64 / 120.0),
                &signal(1000.0),
                true,
            );
        }
        for i in 0..=30 {
            slow.advance(
                start + Duration::from_secs_f64(i as f64 / 30.0),
                &signal(1000.0),
                true,
            );
        }
        for (a, b) in fast.energy.iter().zip(slow.energy) {
            assert!((a - b).abs() < 0.001);
        }
        let encoded = serde_json::to_string(&fast.settings_snapshot()).unwrap();
        assert!(
            !encoded.contains("phase")
                && !encoded.contains("energy")
                && !encoded.contains("last_time")
        );
        let mut loaded = Veroni::default();
        loaded.apply_settings(&serde_json::from_str(&encoded).unwrap());
        assert_eq!(loaded.settings_snapshot(), fast.settings_snapshot());
        assert_eq!(loaded.phase, 0.0);
        let mut unsafe_settings = fast.settings_snapshot();
        unsafe_settings.cells = u32::MAX;
        unsafe_settings.attack_ms = 0.0;
        unsafe_settings.speed = f32::NAN;
        loaded.apply_settings(&unsafe_settings);
        assert_eq!(loaded.settings.cells, 240);
        assert_eq!(loaded.settings.attack_ms, 5.0);
        assert!(loaded.settings.speed.is_finite());
    }

    #[test]
    #[ignore = "manual rendering timing probe"]
    fn render_timing() {
        let ctx = egui::Context::default();
        let theme = AppTheme::default();
        let mut veroni = Veroni::default();
        for count in [128, 240] {
            veroni.settings.cells = count;
            let start = Instant::now();
            for i in 0..120 {
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(
                            Pos2::ZERO,
                            Vec2::new(1600.0, 900.0),
                        )),
                        ..Default::default()
                    },
                    |ui| {
                        veroni.draw(
                            ui,
                            ui.max_rect(),
                            &signal(80.0),
                            &theme,
                            start + Duration::from_millis(i * 16),
                            true,
                        );
                    },
                );
                output.textures_delta.clear();
                std::hint::black_box(ctx.tessellate(output.shapes, output.pixels_per_point));
            }
            eprintln!(
                "Veroni {count} cells, UI + tessellation: {:.2} ms/frame",
                start.elapsed().as_secs_f64() * 1000.0 / 120.0
            );
        }
    }
    #[test]
    fn every_macro_accepts_one_named_source_offline_without_overwriting_saved_bases() {
        let start = Instant::now();
        let controls = Veroni::macro_controls();
        let mut targets = std::collections::HashSet::new();
        for (i, c) in controls.into_iter().enumerate() {
            assert!(targets.insert(c.target));
            let mut model = Veroni::default();
            model.settings.modulation.as_mut().unwrap().routes = vec![crate::modulation::Route {
                signal: Some(7),
                target: c.target,
                amount: 0.15,
                ..Default::default()
            }];
            let saved = model.settings.clone();
            let base = Veroni::macro_values(&saved)[i];
            let mut snapshot = crate::signals::Snapshot::default();
            snapshot.values.insert(
                7,
                crate::signals::Sample {
                    value: 0.4,
                    input: 0.4,
                    availability: 1.0,
                    status: crate::signals::Status::Live,
                    ..Default::default()
                },
            );
            let frame = AnalysisFrame {
                signals: Some(std::sync::Arc::new(snapshot)),
                ..Default::default()
            };
            model.advance(start, &frame, false);
            model.advance(start + std::time::Duration::from_millis(50), &frame, false);
            assert!(
                model.macro_effective()[i] > base,
                "{} did not receive the source",
                c.label
            );
            assert_eq!(
                model.settings, saved,
                "runtime modulation replaced a manual base"
            );
            assert!((c.min..=c.max).contains(&model.macro_effective()[i]));
        }
    }
}
