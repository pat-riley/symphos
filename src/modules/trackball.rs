//! Shared free rotation for 3D primitives.
use egui::Pos2;

pub fn orientation_axes(yaw: f32, elevation: f32, roll: f32) -> [[f64; 3]; 3] {
    // Double precision keeps the composed orientation orthogonal when
    // converting back through nearly pole-on Euler representations.
    let (sy, cy) = f64::from(yaw).sin_cos();
    let (se, ce) = f64::from(elevation).sin_cos();
    let (sr, cr) = f64::from(roll).sin_cos();
    [
        [cr * cy + sr * se * sy, sr * cy - cr * se * sy, -ce * sy],
        [sr * ce, -cr * ce, se],
        [cr * sy - sr * se * cy, sr * sy + cr * se * cy, ce * cy],
    ]
}

pub fn drag(
    yaw: &mut f32,
    elevation: &mut f32,
    roll: &mut f32,
    from: Pos2,
    to: Pos2,
    center: Pos2,
    radius: f32,
) {
    if from == to {
        return;
    }
    let a = trackball_point(from, center, radius);
    let b = trackball_point(to, center, radius);
    let turn = cross(a, b);
    let length = dot(turn, turn).sqrt();
    let cosine = dot(a, b).clamp(-1.0, 1.0);
    let (axis, angle) = if length > 1e-6 {
        (turn.map(|v| v / length), length.atan2(cosine))
    } else if cosine < 0.0 {
        let helper = if a[0].abs() < 0.9 {
            [1.0, 0.0, 0.0]
        } else {
            [0.0, 1.0, 0.0]
        };
        let axis = cross(a, helper);
        let length = dot(axis, axis).sqrt();
        (axis.map(|v| v / length), std::f64::consts::PI)
    } else {
        return;
    };
    // Compose in camera space. Rebuilding Euler controls from the resulting
    // orientation keeps old projects/macros compatible without constraining
    // drag movement to world yaw/elevation axes (including at either pole).
    let columns =
        orientation_axes(*yaw, *elevation, *roll).map(|basis| rotate_vector(basis, axis, angle));
    let m: [[f64; 3]; 3] = std::array::from_fn(|r| std::array::from_fn(|c| columns[c][r]));
    let horizontal = m[2][0].hypot(m[2][2]);
    *elevation = m[2][1].atan2(horizontal) as f32;
    if horizontal > 1e-10 {
        *yaw = (-m[2][0]).atan2(m[2][2]) as f32;
        *roll = m[0][1].atan2(-m[1][1]) as f32;
    } else {
        *yaw = 0.0;
        *roll = m[1][0].atan2(m[0][0]) as f32;
    }
}

pub(super) fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a.into_iter().zip(b).map(|(a, b)| a * b).sum()
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn trackball_point(pointer: Pos2, center: Pos2, radius: f32) -> [f64; 3] {
    let p = (pointer - center) / radius.max(1.0);
    let x = f64::from(p.x);
    let y = f64::from(p.y);
    let length = x * x + y * y;
    // A hyperbolic rim lets drags outside the silhouette produce roll smoothly
    // instead of clamping the pointer to an equator or locking an axis.
    let z = if length <= 0.5 {
        (1.0 - length).sqrt()
    } else {
        0.5 / length.sqrt()
    };
    let v = [x, y, z];
    let length = dot(v, v).sqrt();
    v.map(|v| v / length)
}
fn rotate_vector(v: [f64; 3], axis: [f64; 3], angle: f64) -> [f64; 3] {
    let (sin, cos) = angle.sin_cos();
    let side = cross(axis, v);
    let along = dot(axis, v) * (1.0 - cos);
    std::array::from_fn(|i| v[i] * cos + side[i] * sin + axis[i] * along)
}
