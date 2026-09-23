//! Keeping the ship out of the world's solids.
//!
//! The engine's own collision (`0x427280`) takes the ship's old and new
//! positions, grows a box a unit each way around them, asks every cell the box
//! spans for the surfaces inside it (`0x4279b0`, `0x428790`), and for each one
//! that the new position is behind pushes the position back along the
//! surface's normal until it is in front of it, with a little to spare -
//! `0x4277c0` multiplies the push by `0x103e8`, a ninety-fifth of a percent
//! over.
//!
//! What that collects for a cell is the ground's two triangles, the faces of
//! its boxes, and the chamber's floor and ceiling. This port does the same for
//! boxes, which is what the ship flies into: the ground it keeps with the
//! height query it already had, and the boxes it treats as the solid blocks
//! they are, pushing the ship out of the face it is least far through. The
//! difference from the engine is that the engine pushes a point onto the
//! surface and this pushes a sphere off it.

/// The ship's half-size, the unit the engine grows its query box by
/// (`0x4272d2`).
pub const SHIP: f32 = 1.0;

use hb_formats::vector::{length, scale, sub};

/// The engine's overshoot on a push (`0x427841`).
pub const OVERSHOOT: f32 = 0x103e8 as f32 / 65536.0;

/// A solid box, in units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Solid {
    pub min: [f32; 3],
    pub max: [f32; 3],
}

impl Solid {
    /// From the world's 16.16 box.
    pub fn of(solid: hb_world::grid::Solid) -> Solid {
        let units = |v: [i32; 3]| v.map(|c| c as f32 / 65536.0);
        Solid { min: units(solid.min), max: units(solid.max) }
    }

    fn contains(&self, p: [f32; 3]) -> bool {
        (0..3).all(|k| p[k] > self.min[k] && p[k] < self.max[k])
    }

    fn nearest(&self, p: [f32; 3]) -> [f32; 3] {
        std::array::from_fn(|k| p[k].clamp(self.min[k], self.max[k]))
    }
}

/// Where a push came from, so the caller can tell landing from hitting a wall.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Push {
    /// Pushed up, off the top of something.
    Up,
    /// Pushed sideways or down.
    Side,
}

/// Move `position` out of every solid it is inside of, a sphere of `radius`.
/// Returns where it ends up and what it was pushed by.
pub fn push_out(position: [f32; 3], radius: f32, solids: &[Solid]) -> ([f32; 3], Option<Push>) {
    let mut p = position;
    let mut push = None;
    // Twice, so that a corner between two boxes settles.
    for _ in 0..2 {
        for solid in solids {
            let out = if solid.contains(p) {
                // Inside: out through the face it is least far through.
                let mut best = (f32::MAX, 0usize, 1.0f32);
                for k in 0..3 {
                    let (low, high) = (p[k] - solid.min[k], solid.max[k] - p[k]);
                    if low < best.0 {
                        best = (low, k, -1.0);
                    }
                    if high < best.0 {
                        best = (high, k, 1.0);
                    }
                }
                let (depth, axis, sign) = best;
                let mut step = [0.0; 3];
                step[axis] = sign * (depth + radius) * OVERSHOOT;
                Some((step, axis == 1 && sign > 0.0))
            } else {
                let near = solid.nearest(p);
                let d = sub(p, near);
                let length = length(d);
                if length >= radius || length <= 0.0 {
                    None
                } else {
                    let want = (radius - length) * OVERSHOOT;
                    let step = scale(d, want / length);
                    Some((step, d[1] > 0.0 && d[1] >= d[0].abs() && d[1] >= d[2].abs()))
                }
            };
            if let Some((step, upward)) = out {
                for k in 0..3 {
                    p[k] += step[k];
                }
                push = Some(match (push, upward) {
                    (Some(Push::Side), _) | (_, false) => Push::Side,
                    _ => Push::Up,
                });
            }
        }
    }
    (p, push)
}

/// The world box a placed object fills: its hit volume turned by its heading
/// and squared off, which is the shape [`push_out`] works in.
///
/// The engine has nothing like this. Its own test against a placed object
/// (`0x40d650`) damages both and pushes neither, and it skips classes 0 and 9 -
/// the scenery and the bunkers - entirely, so in the original the ship flies
/// through a radar dish without so much as a scratch. The port stops it, which
/// is a departure and the only one in its collision.
pub fn solid_of(volume: &crate::combat::HitVolume, at: [f32; 3], heading: u16) -> Solid {
    let (lo, hi) = volume.bounds();
    let mut min = [f32::MAX; 3];
    let mut max = [f32::MIN; 3];
    for &x in &[lo[0], hi[0]] {
        for &z in &[lo[2], hi[2]] {
            let w = crate::combat::to_world([x, 0.0, z], at, heading as f32);
            min[0] = min[0].min(w[0]);
            max[0] = max[0].max(w[0]);
            min[2] = min[2].min(w[2]);
            max[2] = max[2].max(w[2]);
        }
    }
    min[1] = at[1] + lo[1];
    max[1] = at[1] + hi[1];
    Solid { min, max }
}
