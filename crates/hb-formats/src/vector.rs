//! Three-vectors of floats, in units, and the world's wrap applied to them.
//!
//! The engine does this arithmetic in 16.16 and x87 by hand at every site;
//! the port does it once, here. A difference between two positions is taken
//! the short way round the world - x and z wrapped into -512..512, the way the
//! engine's `shl 6; sar 6` does it ([`crate::fixed::wrapped`]) - and y, which
//! does not wrap, as it is.

use crate::fixed::{radians, wrapped, UNITS_PER_RADIAN};

pub type Vec3 = [f32; 3];

pub fn add(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

pub fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub fn scale(a: Vec3, k: f32) -> Vec3 {
    [a[0] * k, a[1] * k, a[2] * k]
}

pub fn dot(a: Vec3, b: Vec3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub fn length(a: Vec3) -> f32 {
    dot(a, a).sqrt()
}

/// Across x and z only, as the engine measures range on the ground.
pub fn flat_length(a: Vec3) -> f32 {
    (a[0] * a[0] + a[2] * a[2]).sqrt()
}

/// Scaled to length one; zero stays zero.
pub fn normalise(a: Vec3) -> Vec3 {
    let l = length(a);
    if l == 0.0 {
        a
    } else {
        scale(a, 1.0 / l)
    }
}

pub fn cross(a: Vec3, b: Vec3) -> Vec3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

/// `to - from` the short way round the world.
pub fn offset(from: Vec3, to: Vec3) -> Vec3 {
    [wrapped(to[0] - from[0]), to[1] - from[1], wrapped(to[2] - from[2])]
}

/// How far apart two points are, the short way round.
pub fn distance(a: Vec3, b: Vec3) -> f32 {
    length(offset(a, b))
}

/// Whether two points are within `reach` of each other on every axis, the
/// short way round: the box test the engine makes before, or instead of, a
/// distance.
pub fn within(a: Vec3, b: Vec3, reach: f32) -> bool {
    offset(a, b).iter().all(|c| c.abs() < reach)
}

/// The same on x and z alone.
pub fn flat_within(a: Vec3, b: Vec3, reach: f32) -> bool {
    let d = offset(a, b);
    d[0].abs() < reach && d[2].abs() < reach
}

/// The copy of `at` nearest `eye`: `eye` plus the short way to `at`, so the
/// two can be compared or drawn in one frame even across the world's edge.
pub fn nearest(eye: Vec3, at: Vec3) -> Vec3 {
    add(eye, offset(eye, at))
}

/// The heading and pitch that point along `v`, in the 16-bit circle and
/// signed: heading 0 along +z and a quarter turn along +x, pitch positive
/// nose down - `atan2(x, z)` and `-atan2(y, flat)`, as every aiming routine
/// in the engine takes them.
pub fn angles_of(v: Vec3) -> (f32, f32) {
    (v[0].atan2(v[2]) * UNITS_PER_RADIAN, -v[1].atan2(flat_length(v)) * UNITS_PER_RADIAN)
}

/// A point brought back into the world, x and z into -512..512, as the engine
/// keeps every position after a move.
pub fn in_world(p: Vec3) -> Vec3 {
    [wrapped(p[0]), p[1], wrapped(p[2])]
}

/// The axes a heading, pitch and roll give - right, up, forward - as the
/// actor's matrix holds them (`0x40b1c0`) and the camera and the renderer use
/// them: heading 0 along +z, positive pitch nose down, positive roll the left
/// wing down. Angles in the 16-bit circle.
pub fn axes(heading: f32, pitch: f32, roll: f32) -> [Vec3; 3] {
    let (sh, ch) = radians(heading).sin_cos();
    let (sp, cp) = radians(pitch).sin_cos();
    let (sr, cr) = radians(roll).sin_cos();
    let forward = [sh * cp, -sp, ch * cp];
    let level_up = [sh * sp, cp, ch * sp];
    let level_right = [ch, 0.0, -sh];
    let right = add(scale(level_right, cr), scale(level_up, sr));
    let up = sub(scale(level_up, cr), scale(level_right, sr));
    [right, up, forward]
}

/// The way a heading and pitch point: [`axes`]' forward.
pub fn direction(heading: f32, pitch: f32) -> Vec3 {
    let (sh, ch) = radians(heading).sin_cos();
    let (sp, cp) = radians(pitch).sin_cos();
    [sh * cp, -sp, ch * cp]
}

/// A point in a vector's own frame: its components along three axes.
pub fn along(v: Vec3, axes: &[Vec3; 3]) -> Vec3 {
    [dot(v, axes[0]), dot(v, axes[1]), dot(v, axes[2])]
}

/// A vector given in a frame, back in the world: the axes weighted by its
/// components.
pub fn from_frame(v: Vec3, axes: &[Vec3; 3]) -> Vec3 {
    std::array::from_fn(|k| axes[0][k] * v[0] + axes[1][k] * v[1] + axes[2][k] * v[2])
}
