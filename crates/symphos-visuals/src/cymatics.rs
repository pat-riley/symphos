//! Reduced Kirchhoff–Love plate model. Coordinates cover [-1,1]²; `aspect`
//! stretches Y. Frequencies are dimensionless sqrt(eigenvalue), scaled by the
//! host using sqrt(D/(rho*h))/(width/2)^2. See docs/cymatics.md for assumptions.
use std::f64::consts::PI;
pub const GRID: usize = 65;
pub const MODES: usize = 24;
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlateSpec {
    pub circle: bool,
    pub aspect: f64,
    /// 0: simply supported, 1: clamped, 2: free
    pub edge: u32,
    pub center: bool,
    pub poisson: f64,
}
impl Default for PlateSpec {
    fn default() -> Self {
        Self {
            circle: false,
            aspect: 1.0,
            edge: 0,
            center: false,
            poisson: 0.33,
        }
    }
}
pub struct Mode {
    pub mass: f32,
    pub frequency: f32,
    pub shape: Vec<f32>,
}
pub struct Plate {
    pub spec: PlateSpec,
    pub modes: Vec<Mode>,
}
impl Plate {
    pub fn solve(spec: PlateSpec) -> Self {
        if !spec.circle && spec.edge == 0 && !spec.center {
            let mut modes = Vec::new();
            for m in 1..=8 {
                for n in 1..=8 {
                    let kx = m as f64 * PI / 2.0;
                    let ky = n as f64 * PI / (2.0 * spec.aspect);
                    let shape = grid()
                        .map(|(x, y)| {
                            ((kx * (x + 1.0)).sin() * (n as f64 * PI * (y + 1.0) / 2.0).sin())
                                as f32
                        })
                        .collect();
                    modes.push(Mode {
                        mass: spec.aspect as f32,
                        frequency: (kx * kx + ky * ky) as f32,
                        shape,
                    });
                }
            }
            modes.sort_by(|a, b| a.frequency.total_cmp(&b.frequency));
            modes.truncate(MODES);
            return Self { spec, modes };
        }
        ritz(spec)
    }
}
pub fn grid() -> impl Iterator<Item = (f64, f64)> {
    (0..GRID * GRID).map(|i| {
        (
            2.0 * (i % GRID) as f64 / (GRID - 1) as f64 - 1.0,
            2.0 * (i / GRID) as f64 / (GRID - 1) as f64 - 1.0,
        )
    })
}
pub fn sample(field: &[f32], x: f32, y: f32) -> f32 {
    let x = ((x + 1.0) * 0.5 * (GRID - 1) as f32).clamp(0.0, (GRID - 1) as f32);
    let y = ((y + 1.0) * 0.5 * (GRID - 1) as f32).clamp(0.0, (GRID - 1) as f32);
    let i = (x as usize).min(GRID - 2);
    let j = (y as usize).min(GRID - 2);
    let u = x - i as f32;
    let v = y - j as f32;
    (field[j * GRID + i] * (1.0 - u) + field[j * GRID + i + 1] * u) * (1.0 - v)
        + (field[(j + 1) * GRID + i] * (1.0 - u) + field[(j + 1) * GRID + i + 1] * u) * v
}
// A polynomial and its first/second derivatives in two coordinates.
#[derive(Clone, Copy, Default)]
struct Jet {
    v: f64,
    x: f64,
    y: f64,
    xx: f64,
    yy: f64,
    xy: f64,
}
impl Jet {
    fn constant(v: f64) -> Self {
        Self {
            v,
            ..Self::default()
        }
    }
    fn scale(self, k: f64) -> Self {
        Self {
            v: self.v * k,
            x: self.x * k,
            y: self.y * k,
            xx: self.xx * k,
            yy: self.yy * k,
            xy: self.xy * k,
        }
    }
    fn add(self, b: Self) -> Self {
        Self {
            v: self.v + b.v,
            x: self.x + b.x,
            y: self.y + b.y,
            xx: self.xx + b.xx,
            yy: self.yy + b.yy,
            xy: self.xy + b.xy,
        }
    }
    fn mul(self, b: Self) -> Self {
        Self {
            v: self.v * b.v,
            x: self.x * b.v + self.v * b.x,
            y: self.y * b.v + self.v * b.y,
            xx: self.xx * b.v + 2.0 * self.x * b.x + self.v * b.xx,
            yy: self.yy * b.v + 2.0 * self.y * b.y + self.v * b.yy,
            xy: self.xy * b.v + self.x * b.y + self.y * b.x + self.v * b.xy,
        }
    }
}
fn legendre(x: Jet, n: usize) -> Jet {
    if n == 0 {
        return Jet::constant(1.0);
    }
    let mut a = Jet::constant(1.0);
    let mut b = x;
    for i in 2..=n {
        let c = x
            .mul(b)
            .scale((2 * i - 1) as f64)
            .add(a.scale(-((i - 1) as f64)))
            .scale(1.0 / i as f64);
        a = b;
        b = c;
    }
    b
}
fn basis(x: f64, y: f64, m: usize, n: usize, s: PlateSpec) -> Jet {
    let x = Jet {
        v: x,
        x: 1.0,
        ..Jet::default()
    };
    let y = Jet {
        v: y,
        y: 1.0,
        ..Jet::default()
    };
    let one = Jet::constant(1.0);
    let boundary = if s.circle {
        one.add(x.mul(x).scale(-1.0)).add(y.mul(y).scale(-1.0))
    } else {
        one.add(x.mul(x).scale(-1.0))
            .mul(one.add(y.mul(y).scale(-1.0)))
    };
    let weight = match s.edge {
        0 => boundary,
        1 => boundary.mul(boundary),
        _ => one,
    };
    legendre(x, m).mul(legendre(y, n)).mul(weight)
}
struct Trial {
    samples: Vec<Jet>,
    display: Vec<f64>,
    center: f64,
}
fn ritz(s: PlateSpec) -> Plate {
    // Midpoint quadrature; disk edge is approximated by this fixed quadrature.
    const Q: usize = 52;
    let points: Vec<_> = (0..Q * Q)
        .filter_map(|i| {
            let x = 2.0 * (i % Q) as f64 / Q as f64 + 1.0 / Q as f64 - 1.0;
            let y = 2.0 * (i / Q) as f64 / Q as f64 + 1.0 / Q as f64 - 1.0;
            (!s.circle || x * x + y * y < 1.0).then_some((x, y))
        })
        .collect();
    let area = 4.0 * s.aspect / (Q * Q) as f64;
    let mut trials = Vec::new();
    for m in 0..=8 {
        for n in 0..=8 {
            if m + n > 8 {
                continue;
            }
            trials.push(Trial {
                samples: points.iter().map(|&(x, y)| basis(x, y, m, n, s)).collect(),
                display: grid().map(|(x, y)| basis(x, y, m, n, s).v).collect(),
                center: basis(0.0, 0.0, m, n, s).v,
            });
        }
    }
    if s.center {
        let pivot = trials.remove(0);
        for t in &mut trials {
            let c = t.center / pivot.center;
            for (a, b) in t.samples.iter_mut().zip(&pivot.samples) {
                *a = a.add(b.scale(-c));
            }
            for (a, b) in t.display.iter_mut().zip(&pivot.display) {
                *a -= c * b;
            }
        }
    }
    let mut orth: Vec<Trial> = Vec::new();
    for mut t in trials {
        // Twice-reorthogonalized modified Gram-Schmidt, including derivatives.
        for _ in 0..2 {
            for b in &orth {
                let c = t
                    .samples
                    .iter()
                    .zip(&b.samples)
                    .map(|(a, b)| a.v * b.v * area)
                    .sum::<f64>();
                for (a, b) in t.samples.iter_mut().zip(&b.samples) {
                    *a = a.add(b.scale(-c));
                }
                for (a, b) in t.display.iter_mut().zip(&b.display) {
                    *a -= c * b;
                }
            }
        }
        let norm = (t.samples.iter().map(|a| a.v * a.v * area).sum::<f64>()).sqrt();
        if norm < 1e-9 {
            continue;
        }
        for a in &mut t.samples {
            *a = a.scale(1.0 / norm);
        }
        for a in &mut t.display {
            *a /= norm;
        }
        orth.push(t);
    }
    let n = orth.len();
    let mut k = vec![0.0; n * n];
    for i in 0..n {
        for j in 0..=i {
            let value = orth[i]
                .samples
                .iter()
                .zip(&orth[j].samples)
                .map(|(a, b)| {
                    let ay = a.yy / s.aspect.powi(2);
                    let by = b.yy / s.aspect.powi(2);
                    a.xx * b.xx
                        + ay * by
                        + s.poisson * (a.xx * by + ay * b.xx)
                        + 2.0 * (1.0 - s.poisson) * a.xy * b.xy / s.aspect.powi(2)
                })
                .sum::<f64>()
                * area;
            k[i * n + j] = value;
            k[j * n + i] = value;
        }
    }
    let (values, vectors) = eigen(k, n);
    let mut indices: Vec<_> = (0..n).filter(|&i| values[i] > 0.1).collect();
    indices.sort_by(|&a, &b| values[a].total_cmp(&values[b]));
    indices.truncate(MODES);
    let modes = indices
        .into_iter()
        .map(|i| {
            let mut shape = vec![0.0; GRID * GRID];
            for (j, t) in orth.iter().enumerate() {
                for (a, b) in shape.iter_mut().zip(&t.display) {
                    *a += vectors[j * n + i] * b;
                }
            }
            let maximum = grid()
                .zip(&shape)
                .filter(|((x, y), _)| !s.circle || x * x + y * y <= 1.0)
                .map(|(_, v)| v.abs())
                .fold(0.0, f64::max)
                .max(1e-12);
            Mode {
                mass: (1.0 / (maximum * maximum)) as f32,
                frequency: values[i].sqrt() as f32,
                shape: shape.into_iter().map(|v| (v / maximum) as f32).collect(),
            }
        })
        .collect();
    Plate { spec: s, modes }
}
fn eigen(mut a: Vec<f64>, n: usize) -> (Vec<f64>, Vec<f64>) {
    let mut v = vec![0.0; n * n];
    for i in 0..n {
        v[i * n + i] = 1.0;
    }
    for _ in 0..70 {
        let mut largest = 0.0_f64;
        for p in 0..n {
            for q in p + 1..n {
                let apq = a[p * n + q];
                largest = largest.max(apq.abs());
                if apq.abs() < 1e-10 {
                    continue;
                }
                let tau = (a[q * n + q] - a[p * n + p]) / (2.0 * apq);
                let t =
                    if tau >= 0.0 { 1.0 } else { -1.0 } / (tau.abs() + (1.0 + tau * tau).sqrt());
                let c = 1.0 / (1.0 + t * t).sqrt();
                let sn = t * c;
                a[p * n + p] -= t * apq;
                a[q * n + q] += t * apq;
                a[p * n + q] = 0.0;
                a[q * n + p] = 0.0;
                for r in 0..n {
                    if r != p && r != q {
                        let x = a[r * n + p];
                        let y = a[r * n + q];
                        a[r * n + p] = c * x - sn * y;
                        a[p * n + r] = a[r * n + p];
                        a[r * n + q] = sn * x + c * y;
                        a[q * n + r] = a[r * n + q];
                    }
                    let x = v[r * n + p];
                    let y = v[r * n + q];
                    v[r * n + p] = c * x - sn * y;
                    v[r * n + q] = sn * x + c * y;
                }
            }
        }
        if largest < 1e-7 {
            break;
        }
    }
    ((0..n).map(|i| a[i * n + i]).collect(), v)
}
/// Isotropic thin plate bending stiffness and natural frequency in Hz.
pub fn frequency_hz(
    mode: f32,
    width_m: f32,
    thickness_m: f32,
    young_pa: f32,
    density: f32,
    poisson: f32,
) -> f32 {
    let d = young_pa * thickness_m.powi(3) / (12.0 * (1.0 - poisson * poisson));
    mode * (d / (density * thickness_m)).sqrt() / (width_m * 0.5).powi(2) / std::f32::consts::TAU
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn analytical_square_scaling_and_nodes() {
        let p = Plate::solve(PlateSpec::default());
        assert!((p.modes[0].frequency as f64 - PI * PI / 2.0).abs() < 1e-5);
        let f = frequency_hz(p.modes[0].frequency, 0.4, 0.001, 69e9, 2700.0, 0.33);
        assert!(f > 29.0 && f < 32.0, "{f}");
        assert!(
            (frequency_hz(p.modes[0].frequency, 0.8, 0.001, 69e9, 2700.0, 0.33) / f - 0.25).abs()
                < 1e-6
        );
        assert!(sample(&p.modes[0].shape, -1.0, 0.0).abs() < 1e-6);
    }
    #[test]
    fn ritz_agrees_with_leissa_low_modes() {
        // Leissa NASA SP-160: clamped square omega*a²*sqrt(rho*h/D)=35.99;
        // clamped circular plate omega*R²*sqrt(rho*h/D)=10.2168.
        for (circle, expected) in [(false, 35.99 / 4.0), (true, 10.2168)] {
            let p = Plate::solve(PlateSpec {
                circle,
                edge: 1,
                ..Default::default()
            });
            assert!(
                (p.modes[0].frequency as f64 / expected - 1.0).abs() < 0.025,
                "circle={circle}: {}",
                p.modes[0].frequency
            );
        }
    }
    #[test]
    fn free_and_center_supported_modes_are_finite_and_constrained() {
        for circle in [false, true] {
            for center in [false, true] {
                let p = Plate::solve(PlateSpec {
                    circle,
                    center,
                    edge: 2,
                    ..Default::default()
                });
                assert_eq!(p.modes.len(), MODES);
                for m in &p.modes {
                    assert!(m.frequency.is_finite() && m.frequency > 0.0);
                    assert!(m.shape.iter().all(|v| v.is_finite()));
                    if center {
                        assert!(sample(&m.shape, 0.0, 0.0).abs() < 1e-5);
                    }
                }
            }
        }
    }
}
