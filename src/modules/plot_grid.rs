//! Display-only rulers shared by the spectrum and oscilloscope.
use crate::{help::HoverHelp, parameter::Parameter, theme::AppTheme};
use egui::{Color32, Painter, Pos2, Rect, Stroke};

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub(super) struct Settings {
    pub vertical: bool,
    pub horizontal: bool,
    pub x_divisions: usize,
    pub y_divisions: usize,
    pub subdivisions: usize,
    pub opacity: f32,
    pub minor_opacity: f32,
    pub line_width: f32,
    pub x_labels: bool,
    pub y_labels: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            vertical: true,
            horizontal: true,
            x_divisions: 8,
            y_divisions: 6,
            subdivisions: 4,
            opacity: 0.28,
            minor_opacity: 0.35,
            line_width: 0.7,
            x_labels: true,
            y_labels: true,
        }
    }
}
impl Settings {
    pub fn controls(&mut self, ui: &mut egui::Ui, enabled: bool, x: &str, y: &str) {
        ui.add_enabled_ui(enabled, |ui| {
            ui.vertical(|ui| {
                ui.checkbox(&mut self.vertical, "Vertical lines");
                ui.checkbox(&mut self.horizontal, "Horizontal lines");
            });
            ui.add(Parameter::new(&mut self.x_divisions, 2..=24, 8).key("grid_detail.x_divisions").text(x))
                .help_text("Major divisions across the displayed range. Changes only the grid; audio processing and trace detail are unchanged.");
            ui.add(Parameter::new(&mut self.y_divisions, 2..=24, 6).key("grid_detail.y_divisions").text(y));
            ui.add(Parameter::new(&mut self.subdivisions, 1..=8, 4).key("grid_detail.subdivisions").text("Minor divisions"))
                .help_text("Subdivide each major cell. One hides minor lines; fine lines disappear automatically when they would be less than five pixels apart.");
            ui.add(Parameter::new(&mut self.opacity, 0.0..=1.0, 0.28).key("grid_detail.opacity").text("Grid opacity"));
            ui.add(Parameter::new(&mut self.minor_opacity, 0.0..=1.0, 0.35).key("grid_detail.minor_opacity").text("Minor strength"));
            ui.add(Parameter::new(&mut self.line_width, 0.3..=2.0, 0.7).key("grid_detail.line_width").text("Grid width px"));
        });
        ui.vertical(|ui| {
            ui.checkbox(&mut self.x_labels, "Horizontal labels");
            ui.checkbox(&mut self.y_labels, "Vertical labels");
        });
    }
    pub fn paint(&self, painter: &Painter, plot: Rect, theme: &AppTheme) {
        let opacity = self.opacity.clamp(0.0, 1.0);
        let width = self.line_width.clamp(0.3, 2.0);
        let color = theme.muted;
        for (vertical, enabled, divisions, extent) in [
            (true, self.vertical, self.x_divisions, plot.width()),
            (false, self.horizontal, self.y_divisions, plot.height()),
        ] {
            if !enabled {
                continue;
            }
            for (t, major) in lines(divisions, self.subdivisions, extent) {
                let strength = if major {
                    opacity
                } else {
                    opacity * self.minor_opacity.clamp(0.0, 1.0)
                };
                let stroke = Stroke::new(
                    if major { width } else { width * 0.7 },
                    color.gamma_multiply(strength),
                );
                if vertical {
                    let x = plot.left() + t * plot.width();
                    painter.line_segment(
                        [Pos2::new(x, plot.top()), Pos2::new(x, plot.bottom())],
                        stroke,
                    );
                } else {
                    let y = plot.bottom() - t * plot.height();
                    painter.line_segment(
                        [Pos2::new(plot.left(), y), Pos2::new(plot.right(), y)],
                        stroke,
                    );
                }
            }
        }
    }
    pub fn label_ticks(&self, horizontal: bool, extent: f32) -> Vec<f32> {
        let divisions = if horizontal {
            self.x_divisions
        } else {
            self.y_divisions
        }
        .clamp(2, 24);
        let pixels = if horizontal { 70.0 } else { 32.0 };
        let stride = (divisions as f32 * pixels / extent.max(1.0))
            .ceil()
            .max(1.0) as usize;
        (0..=divisions)
            .filter(|i| *i == 0 || *i == divisions || (i % stride == 0 && divisions - i >= stride))
            .map(|i| i as f32 / divisions as f32)
            .collect()
    }
}
pub(super) fn lines(divisions: usize, subdivisions: usize, extent: f32) -> Vec<(f32, bool)> {
    let divisions = divisions.clamp(2, 24);
    let subdivisions = subdivisions.clamp(1, 8);
    let minor = extent / (divisions * subdivisions) as f32 >= 5.0;
    (0..=divisions * subdivisions)
        .filter_map(|i| {
            let major = i % subdivisions == 0;
            (major || minor).then_some((i as f32 / (divisions * subdivisions) as f32, major))
        })
        .collect()
}

pub(super) fn axis_label(
    painter: &Painter,
    pos: Pos2,
    align: egui::Align2,
    text: String,
    color: Color32,
) {
    painter.text(pos, align, text, egui::FontId::monospace(9.0), color);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn density_is_bounded_and_small_panes_keep_major_lines_without_minor_clutter() {
        let detailed = lines(8, 4, 1000.0);
        assert_eq!(detailed.len(), 33);
        let small = lines(8, 4, 100.0);
        assert_eq!(small.len(), 9);
        assert!(small.iter().all(|(_, major)| *major));
        assert!(lines(usize::MAX, usize::MAX, 2000.0).len() <= 193);
        for (t, _) in detailed {
            assert!((0.0..=1.0).contains(&t));
        }
    }
    #[test]
    #[ignore = "manual CPU timing for adaptive grid painting and tessellation"]
    fn benchmark_plot_grids() {
        let ctx = egui::Context::default();
        for (name, settings) in [
            (
                "simple",
                Settings {
                    vertical: false,
                    y_divisions: 3,
                    subdivisions: 1,
                    ..Default::default()
                },
            ),
            ("default", Settings::default()),
            (
                "maximum",
                Settings {
                    x_divisions: 24,
                    y_divisions: 24,
                    subdivisions: 8,
                    ..Default::default()
                },
            ),
        ] {
            let mut times = Vec::new();
            for frame in 0..200 {
                let began = std::time::Instant::now();
                let mut out = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(
                            Pos2::ZERO,
                            egui::vec2(1200.0, 900.0),
                        )),
                        ..Default::default()
                    },
                    |ui| settings.paint(ui.painter(), ui.max_rect(), &AppTheme::default()),
                );
                out.textures_delta.clear();
                std::hint::black_box(ctx.tessellate(out.shapes, out.pixels_per_point));
                if frame > 30 {
                    times.push(began.elapsed());
                }
            }
            times.sort();
            eprintln!(
                "1200×900 grid {name}: CPU median {:?}, p95 {:?}",
                times[times.len() / 2],
                times[times.len() * 95 / 100]
            );
        }
    }

    #[test]
    fn old_settings_gain_defaults_and_labels_stay_on_major_ticks() {
        let settings: Settings = serde_json::from_str("{}").unwrap();
        assert_eq!(settings, Settings::default());
        for tick in settings.label_ticks(true, 200.0) {
            assert!((tick * settings.x_divisions as f32).fract().abs() < 1e-5);
        }
    }
}
