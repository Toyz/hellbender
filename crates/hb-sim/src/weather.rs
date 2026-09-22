//! Snow and rain: a box of particles that travels with the eye.
//!
//! Bit 0 of the `.LVL`'s weather line (level struct `+0x41c`, `0x6670cc`)
//! turns on snow, bit 1 rain. Both are drawn by the frame (`0x4811e0`) after
//! the world and before the cockpit - snow by `0x49c9b0`, rain by
//! `0x49ce80` - and both work the same way:
//!
//! - A pool of particles, 300 flakes of 24 bytes at `0x5be4b0` and 200 drops
//!   of 36 at `0x5bc890`, scattered through a 16-unit cube centred on the eye
//!   when the level starts (`0x49c8d0`, `0x49cd30`).
//! - Each frame every particle moves by its velocity: straight down, 4.88
//!   units a second for snow (`0xfffb1e00`) and 9.77 for rain
//!   (`0xfff63c00`), with a sideways drift of under a sixty-fourth of a unit
//!   a second either way in x and z.
//! - One that ends up more than eight units from the eye on an axis moves
//!   sixteen units the other way along it, so the cube never runs out. It is
//!   not drawn on the frame it moves.
//! - None of it happens - no movement, no drawing - when the eye is below the
//!   ground, above the sky layer, or under a box that overhangs it
//!   (`0x41c4d0` reports something above).
//!
//! A flake is one dot, one 320x200 pixel whatever the mode (`0x486e20`),
//! colour ramp 1 at its brightest: palette index 14. A drop is a line from
//! the drop to a point 0.305 units (`0x4e20`) along the sum of two vectors -
//! straight up, which the drop keeps as the opposite of its own fall, and how
//! far the eye moved this frame (`0x62d6d4..dc`, written by the flight step at
//! `0x464cbe`). So a drop streaks up at rest and leans along the ship's travel
//! when it flies: the drop's motion as the eye sees it. Colour ramp 1 at
//! `0x6000`, palette index 5.

use crate::turret::Rng;

/// The pool sizes.
pub const FLAKES: usize = 300;
pub const DROPS: usize = 200;

/// Half the cube's side, 16.16.
pub const HALF: i32 = 8 << 16;

/// How fast each falls, 16.16 units a second.
pub const SNOW_FALL: i32 = 0xfffb_1e00_u32 as i32;
pub const RAIN_FALL: i32 = 0xfff6_3c00_u32 as i32;

/// The streak's length, 16.16 (`0x49d107`).
pub const STREAK: i32 = 0x4e20;

/// The palette indices they are drawn in: colour ramp 1 (indices 0 to 15)
/// at `0xffff` and `0x6000` (`0x4566c0`).
pub const SNOW_INDEX: u8 = 14;
pub const RAIN_INDEX: u8 = 5;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Particle {
    /// 16.16 world units.
    pub position: [i32; 3],
    /// 16.16 units a second.
    pub velocity: [i32; 3],
    /// A drop's up vector, the opposite of its fall as a unit vector; zero
    /// for a flake.
    pub up: [i32; 3],
    /// Whether it may be drawn this frame: false on a frame it wrapped.
    pub shown: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Weather {
    pub snow: Vec<Particle>,
    pub rain: Vec<Particle>,
}

fn wrap(v: i32) -> i32 {
    v.wrapping_shl(6) >> 6
}

fn mul(a: i32, b: i32) -> i32 {
    ((a as i64 * b as i64) >> 16) as i32
}

/// A vector scaled to unit length in 16.16 (`0x42bb80`).
pub fn unit(v: [i32; 3]) -> [i32; 3] {
    let f = v.map(|c| c as f64);
    let len = (f[0] * f[0] + f[1] * f[1] + f[2] * f[2]).sqrt();
    if len == 0.0 {
        return [0; 3];
    }
    f.map(|c| (c / len * 65536.0) as i32)
}

/// One particle somewhere in the cube around `eye`, falling at `fall`.
fn scatter(eye: [i32; 3], fall: i32, rng: &mut Rng) -> Particle {
    // `(rand() << 8) & 0xfffff`, less eight units: anywhere in the cube.
    let mut position = [0; 3];
    for (p, e) in position.iter_mut().zip(eye) {
        *p = wrap(e + ((rng.next() as i32) << 8 & 0xfffff) - HALF);
    }
    // `(rand() & 0x7ff) - 0x400` in x and z.
    let drift = |rng: &mut Rng| (rng.next() as i32 & 0x7ff) - 0x400;
    let x = drift(rng);
    let z = drift(rng);
    Particle { position, velocity: [x, fall, z], up: [0; 3], shown: false }
}

impl Weather {
    /// The pools, scattered around the eye as the level starts. Both are
    /// filled whatever the level's weather, as the engine does.
    pub fn new(eye: [i32; 3], rng: &mut Rng) -> Weather {
        let snow = (0..FLAKES).map(|_| scatter(eye, SNOW_FALL, rng)).collect();
        let rain = (0..DROPS)
            .map(|_| {
                let mut p = scatter(eye, RAIN_FALL, rng);
                p.up = unit(p.velocity).map(|c| -c);
                p
            })
            .collect();
        Weather { snow, rain }
    }

    /// Whether any of it happens for an eye at `eye_y`, with the sky layer at
    /// `sky` and the underside of whatever overhangs the eye at `over`
    /// (`None` for nothing) - all 16.16.
    pub fn falls(eye_y: i32, sky: i32, over: Option<i32>) -> bool {
        eye_y >= 0 && eye_y <= sky && over.is_none_or(|bottom| eye_y >= bottom)
    }

    /// Move one pool a frame of `dt` seconds.
    pub fn step(pool: &mut [Particle], dt: f32, eye: [i32; 3]) {
        let dt = (dt * 65536.0) as i32;
        for p in pool {
            for k in 0..3 {
                p.position[k] += mul(p.velocity[k], dt);
            }
            p.shown = true;
            for k in 0..3 {
                let d = wrap(p.position[k] - eye[k]);
                if d > HALF {
                    p.position[k] = wrap(p.position[k] - 2 * HALF);
                } else if d < -HALF {
                    p.position[k] = wrap(p.position[k] + 2 * HALF);
                }
                if d.abs() >= HALF {
                    p.shown = false;
                }
            }
        }
    }

    /// Where a drop's streak ends, given how far the eye moved this frame
    /// (16.16).
    pub fn streak_end(drop: &Particle, moved: [i32; 3]) -> [i32; 3] {
        let dir = unit([drop.up[0] + moved[0], drop.up[1] + moved[1], drop.up[2] + moved[2]]);
        std::array::from_fn(|k| drop.position[k] + mul(STREAK, dir[k]))
    }
}
