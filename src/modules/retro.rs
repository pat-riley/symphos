//! Shared display styling for waveform traces and spectrogram cells.
use super::{Palette, properties_combo, property_row};
use crate::{help::HoverHelp, parameter::Parameter};
use eframe::egui::{self, Color32, Pos2, Rect, Vec2};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(super) enum Finish {
    #[default]
    Smooth,
    Cells,
    Dots,
}
impl Finish {
    fn label(self) -> &'static str {
        match self {
            Self::Smooth => "Smooth",
            Self::Cells => "LED cells",
            Self::Dots => "Dot matrix",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(super) struct Appearance {
    pub finish: Finish,
    pub cell_size: f32,
    pub cell_gap: f32,
    pub brightness: f32,
    pub reverse: bool,
    pub glow: f32,
    pub black_background: bool,
}
impl Default for Appearance {
    fn default() -> Self {
        Self {
            finish: Finish::Smooth,
            cell_size: 4.0,
            cell_gap: 0.22,
            brightness: 1.0,
            reverse: false,
            glow: 0.0,
            black_background: false,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Look {
    Studio,
    PixelFire,
    Classic,
    Neon,
    Phosphor,
}
impl Look {
    pub const ALL: [Self; 5] = [
        Self::Studio,
        Self::PixelFire,
        Self::Classic,
        Self::Neon,
        Self::Phosphor,
    ];
    fn label(self) -> &'static str {
        match self {
            Self::Studio => "Studio",
            Self::PixelFire => "Pixel fire",
            Self::Classic => "Classic LEDs",
            Self::Neon => "Neon trace",
            Self::Phosphor => "Phosphor dots",
        }
    }
    pub fn style(self) -> (Appearance, Palette) {
        let mut a = Appearance::default();
        if self != Self::Studio {
            a.black_background = true;
            a.finish = Finish::Cells;
            a.glow = 0.08;
        }
        let palette = match self {
            Self::Studio => Palette::Theme,
            Self::PixelFire => {
                a.cell_size = 3.0;
                Palette::PixelFire
            }
            Self::Classic => {
                a.cell_size = 5.0;
                a.cell_gap = 0.28;
                Palette::ClassicLed
            }
            Self::Neon => {
                a.finish = Finish::Smooth;
                a.glow = 0.35;
                Palette::NeonNights
            }
            Self::Phosphor => {
                a.finish = Finish::Dots;
                a.cell_size = 5.0;
                a.glow = 0.18;
                Palette::Phosphor
            }
        };
        (a, palette)
    }
}
pub(super) fn look_controls(ui: &mut egui::Ui) -> Option<Look> {
    let mut chosen = None;
    property_row(ui, "Look", |ui| {
        properties_combo("retro-look", ui.available_width(), 5).selected_text("Choose a starting look…").show_ui(ui, |ui| {
            for look in Look::ALL { if ui.selectable_label(false, look.label()).clicked() { chosen = Some(look); ui.close(); } }
        }).response.help_text("Appearance starting points\nChoose Studio, Pixel fire, Classic LEDs, Neon trace or Phosphor dots, then customize below. Capture, analysis, trigger, history, measurements and macro assignments are retained.");
    });
    chosen
}
impl Appearance {
    pub fn controls(&mut self, ui: &mut egui::Ui, waveform: bool) {
        property_row(ui, "Texture", |ui| {
            properties_combo("retro-finish", ui.available_width(), 3).selected_text(self.finish.label()).show_ui(ui, |ui| {
                for finish in [Finish::Smooth, Finish::Cells, Finish::Dots] { ui.selectable_value(&mut self.finish, finish, finish.label()); }
            }).response.help_text("Smooth drawing, separated LED cells, or a dot matrix. Display texture does not change the input signal or measured values.");
        });
        if self.finish != Finish::Smooth {
            if waveform {
                ui.add(Parameter::new(&mut self.cell_size, 1.5..=14.0, 4.0).key("appearance.cell_size").bounds(1.0..=32.0).text("Cell size px"))
                    .help_text("Cell size in logical pixels. Dense views automatically limit the grid; waveform min/max excursions are retained.");
            }
            ui.add(Parameter::new(&mut self.cell_gap, 0.0..=0.65, 0.22).key("appearance.cell_gap").bounds(0.0..=0.85).text("Cell gap"))
                .help_text("Space between LED cells or dots. Spectrogram cell dimensions follow Time cells and Freq. cells.");
        }
        ui.add(
            Parameter::new(&mut self.brightness, 0.1..=2.0, 1.0)
                .key("appearance.brightness")
                .bounds(0.0..=3.0)
                .text("Brightness"),
        )
        .help_text("Display brightness only. Does not change audio gain or the measurement scale.");
        ui.add(Parameter::new(&mut self.glow, 0.0..=1.0, 0.0).key("appearance.glow").text("Glow"))
            .help_text("A soft halo around waveform traces, or luminous edges around spectrogram cells. Smooth spectrogram pixels use color bloom.");
        ui.checkbox(&mut self.reverse, "Reverse gradient")
            .help_text(
                "Reverse the color gradient without reversing the signal or scrolling direction.",
            );
        if !crate::modules::flat_inspector(ui) {
            ui.checkbox(&mut self.black_background, "Black canvas")
                .help_text("Use a black component canvas independently of the app theme.");
        }
    }
    pub fn coordinate(&self, t: f32) -> f32 {
        if self.reverse {
            1.0 - t.clamp(0.0, 1.0)
        } else {
            t.clamp(0.0, 1.0)
        }
    }
    pub fn tint(&self, c: Color32) -> Color32 {
        let brightness = self.brightness.clamp(0.0, 3.0);
        Color32::from_rgba_unmultiplied(
            (c.r() as f32 * brightness).min(255.0) as u8,
            (c.g() as f32 * brightness).min(255.0) as u8,
            (c.b() as f32 * brightness).min(255.0) as u8,
            c.a(),
        )
    }
}
pub(super) fn cell(mesh: &mut egui::Mesh, rect: Rect, color: Color32, dots: bool) {
    if dots {
        let base = mesh.vertices.len() as u32;
        mesh.colored_vertex(rect.center(), color);
        for i in 0..8 {
            let angle = i as f32 * std::f32::consts::TAU / 8.0;
            mesh.colored_vertex(
                rect.center()
                    + Vec2::new(
                        angle.cos() * rect.width() * 0.5,
                        angle.sin() * rect.height() * 0.5,
                    ),
                color,
            );
        }
        for i in 0..8 {
            mesh.add_triangle(base, base + 1 + i, base + 1 + (i + 1) % 8);
        }
    } else {
        mesh.add_colored_rect(rect, color);
    }
}
/// Rasterize reduced min/max trace segments onto a bounded grid. Ranges are
/// supplied separately so a looping trace never joins across its cursor seam.
pub(super) fn pixel_trace(
    painter: &egui::Painter,
    trace: &[(Pos2, Color32)],
    rect: Rect,
    filled: Option<f32>,
    a: &Appearance,
    fill_gradient: Option<&dyn Fn(f32) -> Color32>,
) {
    let mut size = a.cell_size.clamp(1.0, 32.0);
    let budget = if a.finish == Finish::Dots {
        4096.0
    } else {
        8192.0
    };
    size = size.max((rect.area() / budget).sqrt());
    let cols = (rect.width() / size).floor().max(1.0) as usize;
    let rows = (rect.height() / size).floor().max(1.0) as usize;
    let mut colors = vec![Color32::TRANSPARENT; cols * rows];
    let fill_colors: Option<Vec<_>> = fill_gradient.map(|gradient| {
        (0..rows)
            .map(|row| gradient(((0.5 - (row as f32 + 0.5) / rows as f32) / 0.45).clamp(-1.0, 1.0)))
            .collect()
    });
    let grid = |p: Pos2| -> (usize, usize) {
        (
            (((p.x - rect.left()) / rect.width() * cols as f32) as usize).min(cols - 1),
            (((p.y - rect.top()) / rect.height() * rows as f32) as usize).min(rows - 1),
        )
    };
    for pair in trace.windows(2) {
        let (x0, y0) = grid(pair[0].0);
        let (x1, y1) = grid(pair[1].0);
        let steps = x0.abs_diff(x1).max(y0.abs_diff(y1)).max(1);
        for step in 0..=steps {
            let t = step as f32 / steps as f32;
            let x = egui::lerp(x0 as f32..=x1 as f32, t).round() as usize;
            let y = egui::lerp(y0 as f32..=y1 as f32, t).round() as usize;
            let color = super::mix(pair[0].1, pair[1].1, t);
            if let Some(opacity) = filled {
                for fill_y in y.min(rows / 2)..=y.max(rows / 2) {
                    if colors[fill_y * cols + x] == Color32::TRANSPARENT {
                        colors[fill_y * cols + x] = fill_colors
                            .as_ref()
                            .map_or(color, |ramp| ramp[fill_y])
                            .gamma_multiply(opacity.clamp(0.0, 1.0));
                    }
                }
            }
            colors[y * cols + x] = color;
        }
    }
    let mut mesh = egui::Mesh::default();
    let mut halo = egui::Mesh::default();
    let step = Vec2::new(rect.width() / cols as f32, rect.height() / rows as f32);
    for (i, color) in colors
        .into_iter()
        .enumerate()
        .filter(|(_, c)| *c != Color32::TRANSPARENT)
    {
        let bounds = Rect::from_min_size(
            rect.min + Vec2::new((i % cols) as f32 * step.x, (i / cols) as f32 * step.y),
            step,
        )
        .shrink2(step * (a.cell_gap.clamp(0.0, 0.85) * 0.5));
        let dots = a.finish == Finish::Dots;
        if a.glow > 0.0 {
            cell(
                &mut halo,
                bounds.expand(size * 0.35),
                color.gamma_multiply(a.glow.clamp(0.0, 1.0) * 0.18),
                dots,
            );
        }
        cell(&mut mesh, bounds, color, dots);
    }
    painter.add(egui::Shape::mesh(halo));
    painter.add(egui::Shape::mesh(mesh));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dense_pixel_traces_preserve_both_excursions_and_bound_geometry() {
        let context = egui::Context::default();
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(1920.0, 1080.0));
        let trace = [
            (Pos2::new(10.0, 540.0), Color32::RED),
            (Pos2::new(960.0, 60.0), Color32::YELLOW),
            (Pos2::new(960.0, 1020.0), Color32::BLUE),
            (Pos2::new(1910.0, 540.0), Color32::RED),
        ];
        for finish in [Finish::Cells, Finish::Dots] {
            let a = Appearance {
                finish,
                cell_size: 1.0,
                glow: 1.0,
                ..Default::default()
            };
            let mut output = context.run_ui(
                egui::RawInput {
                    screen_rect: Some(rect),
                    ..Default::default()
                },
                |ui| {
                    pixel_trace(
                        ui.painter(),
                        &trace,
                        rect,
                        Some(0.75),
                        &a,
                        Some(&|level| {
                            if level < 0.0 {
                                Color32::BLUE
                            } else {
                                Color32::YELLOW
                            }
                        }),
                    );
                },
            );
            output.textures_delta.clear();
            let mut positions = vec![];
            for shape in output.shapes {
                if let egui::Shape::Mesh(mesh) = shape.shape {
                    positions.extend(mesh.vertices.iter().map(|v| v.pos));
                }
            }
            assert!(!positions.is_empty());
            assert!(positions.iter().all(|p| p.is_finite()));
            assert!(positions.len() < 80_000, "{finish:?}: {}", positions.len());
            assert!(positions.iter().any(|p| p.y < 90.0));
            assert!(positions.iter().any(|p| p.y > 990.0));
        }
    }
}
