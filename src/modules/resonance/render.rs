use super::*;
use crate::modules::waterfall::mesh_segment;
use egui::Color32;
#[derive(Clone, Copy)]
struct Vertex {
    screen: Pos2,
    view: [f32; 3],
    color: Color32,
}
#[derive(Clone, Copy)]
enum Kind {
    Face([u32; 3]),
    Line(u32, u32),
    Point(u32),
}
struct Primitive {
    depth: f32,
    kind: Kind,
}
impl Primitive {
    fn face(indices: [u32; 3], vertices: &[Vertex]) -> Self {
        Self {
            depth: indices
                .iter()
                .map(|i| vertices[*i as usize].view[2])
                .sum::<f32>()
                / 3.0,
            kind: Kind::Face(indices),
        }
    }
    fn line(a: u32, b: u32, vertices: &[Vertex]) -> Self {
        Self {
            depth: (vertices[a as usize].view[2] + vertices[b as usize].view[2]) * 0.5,
            kind: Kind::Line(a, b),
        }
    }
    fn point(i: u32, vertices: &[Vertex]) -> Self {
        Self {
            depth: vertices[i as usize].view[2],
            kind: Kind::Point(i),
        }
    }
}
impl Resonance {
    pub(super) fn build_mesh(&self, rect: Rect, theme: &AppTheme) -> egui::Mesh {
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
        let mut all_vertices = Vec::with_capacity(s.layers * (rows + 1) * cols);
        let profiles: Vec<_> = (0..cols)
            .map(|x| {
                let angle = x as f32 / cols as f32 * TAU;
                let petal = (0.5 + 0.5 * (angle * s.petals as f32).cos())
                    .max(0.0)
                    .powf(s.sharpness);
                (angle, s.waist + (1.0 - s.waist) * petal)
            })
            .collect();
        let row_data: Vec<_> = (0..=rows)
            .map(|y| {
                let v = y as f32 / rows as f32;
                let (sin, cos) = (PI * v * s.opening).sin_cos();
                (v, sin, cos, self.band(v))
            })
            .collect();
        for layer in 0..s.layers {
            let t = self.layer_fraction(layer);
            let fade = if s.flow.abs() > 0.0001 {
                (PI * t).sin().max(0.0).sqrt()
            } else {
                1.0
            };
            let radius = (s.radius + t * s.spacing * s.layers.saturating_sub(1) as f32)
                * (1.0 + self.energy * s.audio_expand);
            let turn = self.spin_phase
                * if s.alternate && layer % 2 == 1 {
                    -1.0
                } else {
                    1.0
                }
                + t * s.layers.saturating_sub(1) as f32 * s.twist.to_radians();
            let columns: Vec<_> = profiles
                .iter()
                .map(|(angle, profile)| {
                    let (sin, cos) = (angle + turn).sin_cos();
                    (*profile, sin, cos)
                })
                .collect();
            let mut vertices = Vec::with_capacity((rows + 1) * cols);
            for &(v, sin_phi, cos_phi, band) in &row_data {
                let ripple =
                    (v * PI * s.ripple_count - self.ripple_phase + t * TAU * s.phase_spread).sin()
                        * (s.ripple_amount + s.audio_ripple * band);
                let radial = radius * sin_phi * (1.0 + ripple);
                let depth = radius * cos_phi * s.flattening + (t - 0.5) * s.layer_depth;
                for &(profile, sin_theta, cos_theta) in &columns {
                    let view = project([
                        radial * profile * cos_theta,
                        radial * profile * sin_theta,
                        depth,
                    ]);
                    vertices.push(Vertex {
                        screen: center + Vec2::new(view[0], view[1]) * scale,
                        view,
                        color: Color32::TRANSPARENT,
                    });
                }
            }
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
            let offset = all_vertices.len() as u32;
            all_vertices.extend(vertices);
            for y in 0..rows {
                for x in 0..cols {
                    let a = offset + (y * cols + x) as u32;
                    let b = offset + (y * cols + (x + 1) % cols) as u32;
                    let c = offset + ((y + 1) * cols + (x + 1) % cols) as u32;
                    let d = offset + ((y + 1) * cols + x) as u32;
                    if s.style == Style::Membranes {
                        if y > 0 {
                            primitives.push(Primitive::face([a, b, d], &all_vertices));
                        }
                        if y + 1 < rows || s.opening < 1.0 {
                            primitives.push(Primitive::face([b, c, d], &all_vertices));
                        }
                    }
                    let lines = s.style == Style::Wireframe
                        || (s.style == Style::Membranes && s.wire_opacity > 0.0);
                    if (lines || s.style == Style::Contours) && y % s.grid_stride == 0 && y > 0 {
                        primitives.push(Primitive::line(a, b, &all_vertices));
                    }
                    if lines && x % s.grid_stride == 0 {
                        primitives.push(Primitive::line(a, d, &all_vertices));
                    }
                    if s.style == Style::Points {
                        primitives.push(Primitive::point(a, &all_vertices));
                    }
                    if y + 1 == rows && s.opening < 1.0 && s.style != Style::Points {
                        primitives.push(Primitive::line(c, d, &all_vertices));
                    }
                }
            }
        }
        // Sorting all layers together keeps intersecting translucent petals coherent.
        primitives.sort_unstable_by(|a, b| a.depth.total_cmp(&b.depth));
        let mut mesh = egui::Mesh::default();
        if s.style == Style::Membranes {
            mesh.vertices.reserve(all_vertices.len());
            for v in &all_vertices {
                mesh.colored_vertex(v.screen, v.color);
            }
        }
        mesh.indices.reserve(primitives.len() * 3);
        for primitive in primitives {
            match primitive.kind {
                Kind::Face(indices) => mesh.indices.extend(indices),
                Kind::Line(a, b) => {
                    let a = all_vertices[a as usize];
                    let b = all_vertices[b as usize];
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
                Kind::Point(i) => {
                    let v = all_vertices[i as usize];
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
