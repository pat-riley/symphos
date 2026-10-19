//! Batched LED geometry: a bounded mesh instead of one egui shape per cell.
use super::*;
use appearance::{Appearance, Finish};

pub(super) fn quad(mesh: &mut egui::Mesh, points: [Pos2; 4], colors: [egui::Color32; 4]) {
    let first = mesh.vertices.len() as u32;
    for (point, color) in points.into_iter().zip(colors) {
        mesh.colored_vertex(point, color);
    }
    mesh.add_triangle(first, first + 1, first + 2);
    mesh.add_triangle(first, first + 2, first + 3);
}
fn dot(mesh: &mut egui::Mesh, rect: Rect, color: egui::Color32) {
    let center = rect.center();
    let radius = rect.width().min(rect.height()) * 0.5;
    let start = mesh.vertices.len() as u32;
    mesh.colored_vertex(center, color);
    const RING: [(f32, f32); 8] = [
        (1.0, 0.0),
        (0.70710677, 0.70710677),
        (0.0, 1.0),
        (-0.70710677, 0.70710677),
        (-1.0, 0.0),
        (-0.70710677, -0.70710677),
        (0.0, -1.0),
        (0.70710677, -0.70710677),
    ];
    for (x, y) in RING {
        mesh.colored_vertex(center + Vec2::new(x * radius, y * radius), color);
    }
    for i in 0..8 {
        mesh.add_triangle(start, start + 1 + i, start + 1 + (i + 1) % 8);
    }
}
fn resolution(plot: Rect, width: f32, appearance: &Appearance) -> (usize, usize) {
    let budget = if appearance.finish == Finish::Dots {
        8192
    } else {
        32768
    };
    let mut pitch = appearance.cell_size.clamp(1.0, 32.0);
    loop {
        let columns = (width / pitch).floor().clamp(1.0, 32.0) as usize;
        let rows = (plot.height() / pitch).floor().clamp(1.0, 256.0) as usize;
        if BANDS * columns * rows <= budget {
            return (columns, rows);
        }
        pitch *= 1.1;
    }
}
#[allow(clippy::too_many_arguments)]
pub(super) fn bars(
    painter: &egui::Painter,
    plot: Rect,
    levels: &[f32; BANDS],
    settings: &FrequencySettings,
    appearance: &Appearance,
    gap: f32,
    palette: Palette,
    contrast: f32,
    theme: &AppTheme,
    live: bool,
) {
    let painter = painter.with_clip_rect(plot.intersect(painter.clip_rect()));
    let band_width = plot.width() / BANDS as f32;
    let width = band_width * (1.0 - gap.clamp(0.0, 95.0) / 100.0);
    let (columns, rows) = resolution(plot, width, appearance);
    let mut mesh = egui::Mesh::default();
    let color =
        |level, height, freq| appearance.color(palette, level, height, freq, contrast, theme);
    for (band, &db) in levels.iter().enumerate() {
        let level = if live { settings.intensity(db) } else { 0.0 };
        let frequency = (band as f32 + 0.5) / BANDS as f32;
        let center = plot.left() + (band as f32 + 0.5) * band_width;
        let left = center - width * 0.5;
        let top = plot.bottom() - level * plot.height();
        if appearance.glow > 0.0 && level > 0.0 {
            let bar =
                Rect::from_min_max(Pos2::new(left, top), Pos2::new(left + width, plot.bottom()));
            let tint = color(level, level, frequency);
            for (spread, alpha) in [(5.0, 0.055), (2.0, 0.11)] {
                painter.rect_filled(
                    bar.expand(spread),
                    spread,
                    tint.gamma_multiply(appearance.glow.clamp(0.0, 1.0) * alpha),
                );
            }
        }
        if appearance.finish == Finish::Solid {
            if level > 0.0 {
                quad(
                    &mut mesh,
                    [
                        Pos2::new(left, top),
                        Pos2::new(left + width, top),
                        Pos2::new(left + width, plot.bottom()),
                        Pos2::new(left, plot.bottom()),
                    ],
                    [
                        color(level, level, frequency),
                        color(level, level, frequency),
                        color(level, 0.0, frequency),
                        color(level, 0.0, frequency),
                    ],
                );
            }
            continue;
        }
        let cell_width = width / columns as f32;
        let cell_height = plot.height() / rows as f32;
        let inset = cell_width.min(cell_height) * appearance.cell_gap.clamp(0.0, 0.85) * 0.5;
        for row in 0..rows {
            let height = (row as f32 + 0.5) / rows as f32;
            let lit = height <= level;
            if !lit && appearance.unlit <= 0.0 {
                continue;
            }
            let color = color(level, height, frequency).gamma_multiply(if lit {
                1.0
            } else {
                appearance.unlit.clamp(0.0, 0.3)
            });
            let top = plot.bottom() - (row + 1) as f32 * cell_height;
            for col in 0..columns {
                let cell = Rect::from_min_size(
                    Pos2::new(left + col as f32 * cell_width, top),
                    Vec2::new(cell_width, cell_height),
                )
                .shrink(inset);
                if appearance.finish == Finish::Dots {
                    dot(&mut mesh, cell, color);
                } else {
                    mesh.add_colored_rect(cell, color);
                }
            }
        }
    }
    painter.add(egui::Shape::mesh(mesh));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn led_and_dot_mesh_cost_is_bounded_for_large_and_tiny_views() {
        for size in [
            Vec2::new(50.0, 20.0),
            Vec2::new(3000.0, 1600.0),
            Vec2::new(12000.0, 6000.0),
        ] {
            for finish in [Finish::Cells, Finish::Dots] {
                let appearance = Appearance {
                    finish,
                    cell_size: 1.0,
                    ..Default::default()
                };
                let plot = Rect::from_min_size(Pos2::ZERO, size);
                let (cols, rows) = resolution(plot, size.x / BANDS as f32, &appearance);
                assert!(BANDS * cols * rows <= if finish == Finish::Dots { 8192 } else { 32768 });
            }
        }
    }
}
