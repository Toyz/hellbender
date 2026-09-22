//! The engine's numbers: 16.16 fixed point, the 16-bit circle, and a world
//! that wraps.
//!
//! Positions, speeds and times are `i32` with sixteen bits of fraction, so a
//! unit - an eighth of a cell - is `0x10000`. The world is 1,024 units across
//! and wraps, and the engine takes every difference between two positions with
//! `shl 6; sar 6`, which keeps the low 26 bits signed: the shortest way round.
//! These are the pieces of that arithmetic the port needs in more than one
//! place.

/// One unit, 1.0 in 16.16.
pub const ONE: i32 = 0x10000;

/// 16.16 to units.
pub fn to_units(v: i32) -> f32 {
    v as f32 / 65536.0
}

/// Units to 16.16, truncating as the engine's `_ftol` does.
pub fn from_units(v: f32) -> i32 {
    (v * 65536.0) as i32
}

/// A 16.16 product, `imul` then `shrd 16` - the low half of the 64-bit
/// result shifted down, which rounds toward minus infinity.
pub fn mul(a: i32, b: i32) -> i32 {
    ((a as i64 * b as i64) >> 16) as i32
}

/// A difference between two world positions, 16.16, the short way round:
/// `shl 6; sar 6`.
pub fn wrap(v: i32) -> i32 {
    v.wrapping_shl(6) >> 6
}

/// The same for a difference in units: into -512..512.
pub fn wrapped(d: f32) -> f32 {
    (d + 512.0).rem_euclid(1024.0) - 512.0
}

/// A whole turn of the 16-bit circle, for the float code that keeps angles
/// in its units.
pub const TURN: f32 = 65536.0;

/// An angle in the 16-bit circle, as radians.
pub fn radians(circle: f32) -> f32 {
    circle * std::f32::consts::TAU / TURN
}

/// Radians into the 16-bit circle, 0 up to a whole turn.
pub fn circle(radians: f32) -> f32 {
    (radians / std::f32::consts::TAU * TURN).rem_euclid(TURN)
}

/// An angle, or the difference of two, folded into minus half a turn to
/// plus half a turn: the short way round, as the engine's `shl 16; sar 16`
/// takes it before easing a heading.
pub fn signed(a: f32) -> f32 {
    (a + TURN / 2.0).rem_euclid(TURN) - TURN / 2.0
}

/// The engine's sine and cosine of an angle in the 16-bit circle, 16.16
/// (`0x429ea0`, `0x429ed0`). The engine interpolates a 256-entry table; this
/// is the curve the table samples.
pub fn sin(angle: i32) -> i32 {
    ((angle as f64 * std::f64::consts::TAU / 65536.0).sin() * 65536.0).round() as i32
}

pub fn cos(angle: i32) -> i32 {
    ((angle as f64 * std::f64::consts::TAU / 65536.0).cos() * 65536.0).round() as i32
}

/// A vector scaled to unit length, 16.16 (`0x42bb80`). Zero stays zero.
pub fn unit(v: [i32; 3]) -> [i32; 3] {
    let f = v.map(|c| c as f64);
    let length = (f[0] * f[0] + f[1] * f[1] + f[2] * f[2]).sqrt();
    if length == 0.0 {
        return [0; 3];
    }
    f.map(|c| (c / length * 65536.0) as i32)
}

/// The distance between two points, 16.16, each difference wrapped and the
/// result truncated (`0x42b960`).
pub fn distance(a: [i32; 3], b: [i32; 3]) -> i32 {
    let d = |i: usize| wrap(a[i].wrapping_sub(b[i])) as f64;
    (d(0) * d(0) + d(1) * d(1) + d(2) * d(2)).sqrt() as i32
}
