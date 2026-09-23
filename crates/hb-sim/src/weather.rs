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

use hb_formats::fixed::{distance, from_units, mul, unit, wrap};

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
        let dt = from_units(dt);
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

/// How many strikes can be armed at once (`0x5bc7c0`, five 40-byte slots).
pub const STRIKES: usize = 5;

/// How long the flash lasts, 16.16 seconds (`+0x1c = 0x8000`).
pub const FLASH: i32 = 0x8000;

/// Seconds of delay per 16.16 unit of distance before the thunder: sound at
/// about 20.6 units a second (`0x4ef954`).
pub const THUNDER_PER_UNIT: f32 = 7.398_200_5e-7;

/// How far from the eye a strike lands, in x and z (`0x280000`).
pub const STRIKE_REACH: i32 = 40 << 16;

/// One armed strike, `0x5bc7c0 + 40 * n`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Strike {
    /// Where it last struck, at the sky layer's altitude.
    pub at: [i32; 3],
    /// Seconds to the next strike, 16.16 (`+0x14`).
    pub countdown: i32,
    /// Seconds until the thunder, 16.16; zero once heard (`+0x18`).
    pub thunder: i32,
    /// Seconds of flash left, 16.16; zero when dark (`+0x1c`).
    pub flash: i32,
    /// The random part of the interval and its base, 16.16 seconds (`+0x20`,
    /// `+0x24`): the `.LVL`'s line 42, as stored.
    pub range: i32,
    pub base: i32,
}

/// What a frame of lightning did, for the caller to play and draw.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Flash {
    /// A strike: `lghtng.wav` at `at` (the eye's height there), and the
    /// light goes up (`0x49d381`, `0x451450`).
    Struck { at: [i32; 3] },
    /// The thunder reaches the eye: `thun-c.wav` at the strike.
    Thunder { at: [i32; 3] },
    /// The flash is over and the light goes back (`0x4514e0`).
    Dark,
}

#[derive(Debug, Clone, Default)]
pub struct Lightning {
    pub strikes: Vec<Strike>,
}

impl Lightning {
    /// `0x49d220`, as the level loads (`0x44bfae`): one strike armed,
    /// `base + rand() % range + 1` away. The `.LVL`'s line 42 is
    /// `983040,1966080` in every level, 15.0 and 30.0 in 16.16, and the engine
    /// does its arithmetic on them as they are - so `rand() % range` is
    /// `rand()`, at most 32,767, half a second. Lightning strikes every 15 to
    /// 15.5 seconds.
    pub fn new(base: i32, range: i32, rng: &mut Rng) -> Lightning {
        let range = range.max(1);
        let countdown = base + rng.next() as i32 % range + 1;
        Lightning { strikes: vec![Strike { countdown, range, base, ..Strike::default() }] }
    }

    /// Whether the light is up this frame.
    pub fn flashing(&self) -> Option<&Strike> {
        self.strikes.iter().find(|s| s.flash > 0)
    }

    /// `0x49d290`, a frame of `dt` seconds with the eye at `eye` and the sky
    /// layer at `sky`, both 16.16. The engine only runs it with the eye at or
    /// above the ground.
    pub fn step(&mut self, dt: f32, eye: [i32; 3], sky: i32, rng: &mut Rng) -> Vec<Flash> {
        let dt = from_units(dt);
        let mut out = Vec::new();
        for s in &mut self.strikes {
            s.countdown -= dt;
            if s.countdown <= 0 {
                s.countdown = s.base + rng.next() as i32 % s.range;
                let near = |e: i32, rng: &mut Rng| {
                    wrap(e + ((rng.next() as i32) << 8) % 0x50_0000 - STRIKE_REACH)
                };
                let x = near(eye[0], rng);
                let z = near(eye[2], rng);
                s.at = [x, sky, z];
                let far = distance(eye, [x, sky, z]);
                s.thunder = ((far as f32 * THUNDER_PER_UNIT) as i32) << 16;
                s.flash = FLASH;
                out.push(Flash::Struck { at: [x, eye[1], z] });
            }
            if s.thunder != 0 {
                s.thunder -= dt;
                if s.thunder <= 0 {
                    s.thunder = 0;
                    out.push(Flash::Thunder { at: s.at });
                }
            }
            if s.flash != 0 {
                s.flash -= dt;
                if s.flash <= 0 {
                    s.flash = 0;
                    out.push(Flash::Dark);
                }
            }
        }
        out
    }
}

/// The bolt, `0x49d640`: a jagged line from the strike down to the floor
/// under it. Each step drops up to four units and wanders up to one either
/// way in x and z, and the whole bolt is drawn afresh every frame of the
/// flash in palette index 110 or 111, chosen per step - so it flickers.
/// `floor` is the surface under a point in 16.16 (`0x41c300`). Returns the
/// segments with their colours.
pub fn bolt(from: [i32; 3], floor: impl Fn([i32; 3]) -> i32, rng: &mut Rng) -> Vec<([i32; 3], [i32; 3], u8)> {
    let mut out = Vec::new();
    let mut at = from;
    // The engine's loop runs until the floor; a cap keeps a bolt over a
    // bottomless point finite.
    while floor(at) < at[1] && out.len() < 256 {
        let colour = 0x6e + (rng.next() & 1) as u8;
        let jitter = |rng: &mut Rng, mask: i32| ((rng.next() as i32) << 16) & mask;
        let x = at[0] + jitter(rng, 0x1ffff) - 0x10000;
        let y = at[1] - jitter(rng, 0x3ffff);
        let z = at[2] + jitter(rng, 0x1ffff) - 0x10000;
        let next = [x, y, z];
        out.push((at, next, colour));
        at = next;
    }
    out
}
