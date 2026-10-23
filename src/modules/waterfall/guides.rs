use crate::{help::HoverHelp, parameter::Parameter, theme::AppTheme};
use egui::{Painter, Pos2, Stroke};

use super::super::plot_grid;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub(super) struct Settings {
    pub grid: plot_grid::Settings,
    pub level_lines: bool,
    pub level_divisions: usize,
    pub level_labels: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            grid: plot_grid::Settings {
                x_divisions: 8,
                y_divisions: 8,
                subdivisions: 1,
                opacity: 0.35,
                line_width: 0.6,
                ..Default::default()
            },
            level_lines: false,
            level_divisions: 4,
            level_labels: true,
        }
    }
}

impl Settings {
    pub fn grid_controls(&mut self, ui: &mut egui::Ui, floor: bool, axes: bool) {
        ui.add_enabled_ui(floor, |ui| {
            ui.checkbox(&mut self.grid.vertical, "Frequency grid lines");
            ui.checkbox(&mut self.grid.horizontal, "Time grid lines");
        });
        ui.add_enabled_ui(floor || axes, |ui| {
            ui.add(Parameter::new(&mut self.grid.x_divisions, 2..=24, 8).key("guide_detail.grid.x_divisions").text("Frequency divisions"))
                .help_text("Major divisions across the displayed frequency range. Follows the signal's linear or logarithmic frequency scale without changing the audio.");
            ui.add(Parameter::new(&mut self.grid.y_divisions, 2..=24, 8).key("guide_detail.grid.y_divisions").text("Time divisions"))
                .help_text("Divide the displayed history into equal time intervals. Does not change history duration or time slices.");
        });
        ui.checkbox(&mut self.level_lines, "Level guides")
            .help_text("Draw horizontal dB reference lines behind the waterfall, along the back and side of the volume.");
        ui.add_enabled_ui(self.level_lines || (axes && self.level_labels), |ui| {
            ui.add(
                Parameter::new(&mut self.level_divisions, 2..=16, 4)
                    .key("guide_detail.level_divisions")
                    .text("Level divisions"),
            );
        });
        ui.add_enabled_ui(floor || self.level_lines, |ui| {
            ui.add(Parameter::new(&mut self.grid.subdivisions, 1..=8, 1).key("guide_detail.grid.subdivisions").text("Minor divisions"))
                .help_text("Subdivide major grid cells. One hides minor lines; finer lines disappear when projected closer than five pixels apart.");
            ui.add(Parameter::new(&mut self.grid.opacity, 0.0..=1.0, 0.35).key("guide_detail.grid.opacity").text("Grid opacity"));
            ui.add(Parameter::new(&mut self.grid.minor_opacity, 0.0..=1.0, 0.35).key("guide_detail.grid.minor_opacity").text("Minor strength"));
            ui.add(Parameter::new(&mut self.grid.line_width, 0.3..=2.0, 0.6).key("guide_detail.grid.line_width").text("Grid width px"));
        });
    }

    pub fn label_controls(&mut self, ui: &mut egui::Ui, axes: bool) {
        ui.add_enabled_ui(axes, |ui| {
            ui.checkbox(&mut self.grid.x_labels, "Frequency labels");
            ui.checkbox(&mut self.grid.y_labels, "Time labels");
            ui.checkbox(&mut self.level_labels, "Level labels");
        });
    }

    pub fn paint(
        &self,
        painter: &Painter,
        project: &impl Fn(f32, f32, f32) -> Pos2,
        floor: bool,
        height: f32,
        theme: &AppTheme,
    ) {
        let draw = |a, b, major| {
            let strength = self.grid.opacity.clamp(0.0, 1.0)
                * if major {
                    1.0
                } else {
                    self.grid.minor_opacity.clamp(0.0, 1.0)
                };
            painter.line_segment(
                [a, b],
                Stroke::new(
                    self.grid.line_width.clamp(0.3, 2.0) * if major { 1.0 } else { 0.7 },
                    theme.muted.gamma_multiply(strength),
                ),
            );
        };
        // Use the shorter projected edge so a tilted grid does not crowd minor lines.
        let frequency_extent = project(-1.4, 0.0, -1.0)
            .distance(project(1.4, 0.0, -1.0))
            .min(project(-1.4, 0.0, 1.0).distance(project(1.4, 0.0, 1.0)));
        let time_extent = project(-1.4, 0.0, -1.0)
            .distance(project(-1.4, 0.0, 1.0))
            .min(project(1.4, 0.0, -1.0).distance(project(1.4, 0.0, 1.0)));
        if floor && self.grid.vertical {
            for (t, major) in plot_grid::lines(
                self.grid.x_divisions,
                self.grid.subdivisions,
                frequency_extent,
            ) {
                let x = -1.4 + t * 2.8;
                draw(project(x, 0.0, -1.0), project(x, 0.0, 1.0), major);
            }
        }
        if floor && self.grid.horizontal {
            for (t, major) in
                plot_grid::lines(self.grid.y_divisions, self.grid.subdivisions, time_extent)
            {
                let z = -1.0 + t * 2.0;
                draw(project(-1.4, 0.0, z), project(1.4, 0.0, z), major);
            }
        }
        if self.level_lines {
            let extent = project(-1.4, 0.0, -1.0).distance(project(-1.4, height, -1.0));
            for (t, major) in
                plot_grid::lines(self.level_divisions.min(16), self.grid.subdivisions, extent)
            {
                if t == 0.0 {
                    continue;
                }
                let y = t * height;
                draw(project(-1.4, y, -1.0), project(1.4, y, -1.0), major);
                draw(project(1.4, y, -1.0), project(1.4, y, 1.0), major);
            }
        }
    }
}
