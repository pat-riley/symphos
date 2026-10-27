use super::*;
use crate::modules::waterfall::mesh_segment;

const SEGMENT_BUDGET: usize = 24_000;
const DENSE_SEGMENT_BUDGET: usize = 64_000;
struct Segment {
    a: Pos2,
    b: Pos2,
    depth: f32,
    color: Color32,
    light: f32,
}
impl Filament {
    pub(super) fn point(&self, shell: usize, t: f32, theta: f32) -> [f32; 3] {
        self.point_with_reach(shell, t, theta, None)
    }
    fn point_with_reach(
        &self,
        shell: usize,
        t: f32,
        theta: f32,
        strand_level: Option<f32>,
    ) -> [f32; 3] {
        let s = &self.settings;
        let phi = PI * t;
        let band = self.band((t * 0.65 + (0.5 - 0.5 * theta.cos()) * 0.35).clamp(0.0, 1.0));
        let fold = (theta * s.lobes as f32 + t * s.twist * TAU + self.phase).cos();
        let mut a = theta + s.twist * t * 0.35;
        let motion =
            (self.flow_time * std::f64::consts::TAU).rem_euclid(std::f64::consts::TAU) as f32;
        let (radius, z) = match s.form {
            Form::Loops => (
                (0.22 + phi.sin().powf(s.taper)) * (1.0 + s.fold * fold),
                (2.0 * t - 1.0) * s.depth + 0.18 * s.fold * (2.0 * theta + self.phase).sin(),
            ),
            Form::Spindles => (
                (0.035 + phi.sin().powf(s.taper) * (0.72 + 0.28 * (TAU * t).cos()))
                    * (1.0 + s.fold * fold * 0.4),
                (2.0 * t - 1.0) * s.depth * 1.5,
            ),
            Form::Folds => (
                (0.35 + phi.sin().powf(s.taper)) * (1.0 + s.fold * fold),
                (2.0 * t - 1.0) * s.depth + s.fold * (2.0 * theta + t * TAU + self.phase).sin(),
            ),
            Form::Vortex | Form::GravityWell => {
                // Open streamlines run from the rim toward the core. Increasing
                // angular density near the core gives inward-moving marks the
                // appearance of accelerating around the attraction center.
                let u = (theta / TAU).clamp(0.0, 1.0);
                a = theta * s.spiral_turns * (1.0 + 0.5 * s.central_pull * u)
                    + t * TAU * s.twist
                    + motion;
                let radius = 0.055 + 1.45 * (1.0 - u).powf(0.6 + s.central_pull * 0.6);
                let ripple = s.fold * 0.12 * (theta * 2.0 + t * TAU + self.phase).sin();
                let z = if s.form == Form::GravityWell {
                    s.depth * (0.45 - 1.5 * u.powf(1.4)) + (t - 0.5) * 0.12
                } else {
                    s.depth * (t - 0.5) * 0.45 + ripple
                };
                (radius * (1.0 + ripple), z)
            }
            Form::Orbital => {
                let lobes = s.spiral_turns.round().max(1.0);
                a = theta + s.twist * t + motion;
                (
                    (0.3 + 0.9 * phi.sin().powf(s.taper))
                        * (1.0 + s.central_pull * 0.35 * (lobes * theta + t * TAU).cos()),
                    s.depth * (0.45 * (theta * lobes + t * TAU).sin() + t - 0.5),
                )
            }
        };
        let bend = if s.audio_response == AudioResponse::Spectrum {
            0.0
        } else {
            s.audio_bend
        };
        let reach = if s.audio_response.reaches() {
            s.spectrum_reach * strand_level.unwrap_or_else(|| self.spectrum_level(t, theta)) * 1.25
        } else {
            0.0
        };
        let r = radius * (1.0 + s.audio_expand * self.energy) + bend * band * 0.35 + reach;
        let p = [
            r * a.cos(),
            r * a.sin(),
            z + bend * band * (theta * 2.0 + self.phase).sin() * 0.3,
        ];
        // Tilt forms around different axes so their overlapping contours read as volumes.
        let u = shell as f32 / s.shells.max(1) as f32;
        let tilt = (u * TAU).sin() * 0.65;
        let (sin, cos) = tilt.sin_cos();
        let x = p[0] * cos + p[2] * sin;
        let z = -p[0] * sin + p[2] * cos;
        let angle = u * TAU;
        let offset = if s.shells == 1 { 0.0 } else { s.spread };
        [
            x + angle.cos() * offset,
            p[1] + angle.sin() * offset,
            z + (shell as f32 - (s.shells - 1) as f32 * 0.5) * s.spread * 0.2,
        ]
    }
    pub(super) fn build_mesh(&self, rect: Rect, theme: &AppTheme) -> egui::Mesh {
        let s = &self.settings;
        let mut mesh = egui::Mesh::default();
        if !rect.is_positive() {
            return mesh;
        }
        let strands = s.strands * if s.dense_weave { 2 } else { 1 };
        let budget = if s.dense_weave {
            DENSE_SEGMENT_BUDGET
        } else {
            SEGMENT_BUDGET
        };
        // Extra turns need more samples. Density has its own bounded budget so
        // enabling it does not trade every added thread for a visibly coarse curve.
        let detail = if s.form.spirals() {
            (s.detail as f32 * s.spiral_turns.max(1.0)).ceil() as usize
        } else {
            s.detail
        };
        let samples = detail.min(budget / (s.shells * strands)).max(32);
        let width_scale = if s.fine_strands { 0.45 } else { 1.0 };
        let width = s.line_width * width_scale;
        let axes =
            trackball::orientation_axes(s.yaw, s.elevation, s.roll).map(|v| v.map(|x| x as f32));
        let extent = 1.4 + s.fold + s.spread + s.depth * 0.45 + s.audio_bend * 0.2;
        let scale = rect.width().min(rect.height()) * 0.47 * s.zoom / extent;
        let center = rect.center() + s.pan * rect.size();
        let project = |p: [f32; 3]| -> ([f32; 3], Pos2) {
            let v =
                std::array::from_fn(|i| axes[0][i] * p[0] + axes[1][i] * p[1] + axes[2][i] * p[2]);
            (v, center + Vec2::new(v[0], v[1]) * scale)
        };
        let mut segments = Vec::with_capacity(s.shells * strands * samples);
        for shell in 0..s.shells {
            for strand in 0..strands {
                let t = (strand as f32 + 0.5) / strands as f32;
                let palette_t = (shell as f32 / s.shells as f32 * (1.0 - s.color_spread)
                    + t * s.color_spread
                    + s.color_shift)
                    .rem_euclid(1.0);
                let color = s.palette.color(palette_t, theme);
                let travel = if s.form.spirals() {
                    self.flow_time.rem_euclid(1.0) as f32
                } else {
                    0.0
                };
                let gap_phase = (strand as f32 * 0.618_034 + shell as f32 * 0.37).fract();
                let strand_level = (s.audio_response.reaches()
                    && s.spectrum_mapping == SpectrumMapping::Strands)
                    .then(|| self.spectrum_level(t, 0.0));
                let point = |theta| {
                    if let Some(level) = strand_level {
                        self.point_with_reach(shell, t, theta, Some(level))
                    } else {
                        self.point(shell, t, theta)
                    }
                };
                let mut last = project(point(0.0));
                for sample in 1..=samples {
                    let u = sample as f32 / samples as f32;
                    let next = project(point(u * TAU));
                    if (u * s.dash_count as f32 + gap_phase - travel).rem_euclid(1.0) >= s.breaks {
                        let delta = next.1 - last.1;
                        let light = (0.65
                            + 0.35 * delta.normalized().dot(Vec2::new(-0.6, -0.8)).abs())
                        .clamp(0.0, 1.0);
                        segments.push(Segment {
                            a: last.1,
                            b: next.1,
                            depth: (last.0[2] + next.0[2]) * 0.5,
                            color,
                            light,
                        });
                    }
                    last = next;
                }
            }
        }
        segments.sort_unstable_by(|a, b| a.depth.total_cmp(&b.depth));
        for segment in segments {
            let depth = (0.65 + 0.35 * (segment.depth / extent * 0.5 + 0.5)).clamp(0.35, 1.0);
            let rgb = segment.color.to_srgba_unmultiplied();
            let alpha = rgb[3] as f32 * s.opacity;
            let color = |factor: f32| {
                Color32::from_rgba_unmultiplied(
                    (rgb[0] as f32 * factor * s.brightness).min(255.0) as u8,
                    (rgb[1] as f32 * factor * s.brightness).min(255.0) as u8,
                    (rgb[2] as f32 * factor * s.brightness).min(255.0) as u8,
                    alpha as u8,
                )
            };
            let edge = color(0.4 * depth);
            mesh_segment(
                &mut mesh,
                segment.a,
                segment.b,
                edge,
                edge,
                width + 0.7 * s.highlights * width_scale,
            );
            let base = color(segment.light * depth);
            mesh_segment(&mut mesh, segment.a, segment.b, base, base, width);
            if s.highlights > 0.01 {
                let shift = Vec2::new(-0.2, -0.2) * width;
                let highlight = super::super::mix(
                    base,
                    Color32::from_rgba_unmultiplied(255, 245, 221, alpha as u8),
                    s.highlights * 0.55,
                );
                mesh_segment(
                    &mut mesh,
                    segment.a + shift,
                    segment.b + shift,
                    highlight,
                    highlight,
                    (s.line_width * 0.24).max(0.2) * width_scale,
                );
            }
        }
        mesh
    }
}
