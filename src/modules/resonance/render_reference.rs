use super::*;
use crate::modules::waterfall::mesh_segment;
use egui::Color32;
#[derive(Clone, Copy)]
struct Vertex {
    screen: Pos2,
    view: [f32; 3],
    color: Color32,
}
enum Primitive {
    Face([Vertex; 3]),
    Line(Vertex, Vertex),
    Point(Vertex),
}
impl Primitive {
    fn depth(&self) -> f32 {
        match self {
            Self::Face(v) => (v[0].view[2] + v[1].view[2] + v[2].view[2]) / 3.0,
            Self::Line(a, b) => (a.view[2] + b.view[2]) * 0.5,
            Self::Point(v) => v.view[2],
        }
    }
}
impl Resonance {
    pub(super) fn reference_mesh(&self, rect: Rect, theme: &AppTheme) -> egui::Mesh {
        if !rect.is_positive() {
            return egui::Mesh::default();
        }
        let s = &self.settings;
        let rows = s
            .detail
            .min((15000.0 / s.layers as f32).sqrt() as usize)
            .max(12);
        let cols = rows * 2;
        let extent = (s.radius + s.spacing * s.layers.saturating_sub(1) as f32)
            * s.flattening.max(1.0)
            + s.layer_depth.abs() * 0.5;
        let scale = rect.width().min(rect.height()) * 0.38 * s.zoom / extent.max(0.1);
        let center = rect.center() + s.pan * rect.size();
        let axes =
            trackball::orientation_axes(s.yaw, s.elevation, s.roll).map(|v| v.map(|x| x as f32));
        let project = |p: [f32; 3]| -> [f32; 3] {
            std::array::from_fn(|i| axes[0][i] * p[0] + axes[1][i] * p[1] + axes[2][i] * p[2])
        };
        let mut primitives = Vec::with_capacity(s.layers * rows * cols * 2);
        for layer in 0..s.layers {
            let t = self.layer_fraction(layer);
            let fade = if s.flow.abs() > 0.0001 {
                (PI * t).sin().max(0.0).sqrt()
            } else {
                1.0
            };
            let mut vertices: Vec<_> = (0..=rows)
                .flat_map(|y| (0..cols).map(move |x| (x, y)))
                .map(|(x, y)| {
                    let view =
                        project(self.point(layer, x as f32 / cols as f32, y as f32 / rows as f32));
                    Vertex {
                        screen: center + Vec2::new(view[0], view[1]) * scale,
                        view,
                        color: Color32::TRANSPARENT,
                    }
                })
                .collect();
            for y in 0..=rows {
                for x in 0..cols {
                    let subtract = |a: [f32; 3], b: [f32; 3]| -> [f32; 3] {
                        std::array::from_fn(|i| a[i] - b[i])
                    };
                    let horizontal = subtract(
                        vertices[y * cols + (x + 1) % cols].view,
                        vertices[y * cols + (x + cols - 1) % cols].view,
                    );
                    let vertical = subtract(
                        vertices[(y + 1).min(rows) * cols + x].view,
                        vertices[y.saturating_sub(1) * cols + x].view,
                    );
                    let normal = [
                        horizontal[1] * vertical[2] - horizontal[2] * vertical[1],
                        horizontal[2] * vertical[0] - horizontal[0] * vertical[2],
                        horizontal[0] * vertical[1] - horizontal[1] * vertical[0],
                    ];
                    let length = normal.iter().map(|x| x * x).sum::<f32>().sqrt();
                    let facing = if length > 1e-8 {
                        normal[2].abs() / length
                    } else {
                        1.0
                    };
                    let edge = (1.0 - facing).powi(2);
                    let mut color_t = match s.color_by {
                        ColorBy::Layer => t,
                        ColorBy::Petal => (x as f32 / cols as f32 * s.petals as f32).fract(),
                        ColorBy::Energy => self.band(y as f32 / rows as f32),
                        ColorBy::Position => y as f32 / rows as f32,
                    };
                    if s.reverse {
                        color_t = 1.0 - color_t;
                    }
                    color_t = (color_t + s.color_shift).clamp(0.0, 1.0);
                    let color = s.palette.color_with_contrast(color_t, s.contrast, theme);
                    let light = s.brightness
                        * (0.55 + 0.45 * facing.sqrt() + edge * s.rim * 0.6)
                        * (1.0 + s.audio_light * self.energy);
                    let alpha = match s.style {
                        Style::Membranes => {
                            (s.opacity * (0.45 + 0.55 * facing) + s.rim * edge * 0.48).min(1.0)
                        }
                        _ => s.opacity.sqrt(),
                    } * fade;
                    let channel = |c: u8| (c as f32 * light).clamp(0.0, 255.0) as u8;
                    vertices[y * cols + x].color = Color32::from_rgba_unmultiplied(
                        channel(color.r()),
                        channel(color.g()),
                        channel(color.b()),
                        (alpha * 255.0) as u8,
                    );
                }
            }
            for y in 0..rows {
                for x in 0..cols {
                    let a = vertices[y * cols + x];
                    let b = vertices[y * cols + (x + 1) % cols];
                    let c = vertices[(y + 1) * cols + (x + 1) % cols];
                    let d = vertices[(y + 1) * cols + x];
                    if s.style == Style::Membranes {
                        if y > 0 {
                            primitives.push(Primitive::Face([a, b, d]));
                        }
                        if y + 1 < rows || s.opening < 1.0 {
                            primitives.push(Primitive::Face([b, c, d]));
                        }
                    }
                    let lines = s.style == Style::Wireframe
                        || (s.style == Style::Membranes && s.wire_opacity > 0.0);
                    if (lines || s.style == Style::Contours) && y % s.grid_stride == 0 && y > 0 {
                        primitives.push(Primitive::Line(a, b));
                    }
                    if lines && x % s.grid_stride == 0 {
                        primitives.push(Primitive::Line(a, d));
                    }
                    if s.style == Style::Points {
                        primitives.push(Primitive::Point(a));
                    }
                    if y + 1 == rows && s.opening < 1.0 && s.style != Style::Points {
                        primitives.push(Primitive::Line(c, d));
                    }
                }
            }
        }
        // Sorting all layers together keeps intersecting translucent petals coherent.
        primitives.sort_unstable_by(|a, b| a.depth().total_cmp(&b.depth()));
        let mut mesh = egui::Mesh::default();
        for primitive in primitives {
            match primitive {
                Primitive::Face(v) => {
                    let base = mesh.vertices.len() as u32;
                    for p in v {
                        mesh.colored_vertex(p.screen, p.color);
                    }
                    mesh.add_triangle(base, base + 1, base + 2);
                }
                Primitive::Line(a, b) => {
                    let opacity = if s.style == Style::Membranes {
                        s.wire_opacity
                    } else {
                        1.0
                    };
                    mesh_segment(
                        &mut mesh,
                        a.screen,
                        b.screen,
                        a.color.gamma_multiply(opacity),
                        b.color.gamma_multiply(opacity),
                        s.line_width,
                    );
                }
                Primitive::Point(v) => {
                    super::super::retro::cell(
                        &mut mesh,
                        Rect::from_center_size(v.screen, Vec2::splat(s.point_size * 2.0)),
                        v.color,
                        true,
                    );
                }
            }
        }
        mesh
    }
}
