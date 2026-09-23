//! Missile smoke: a trail of short tubes left behind the missiles that
//! smoke.
//!
//! Each frame `0x478d90` walks the missile pool and counts each missile's
//! smoke timer (`+0x3c`) down. It starts at 3/16 of a second (`0x477939`),
//! and once it runs out a Viper, a SAM site's missile (both kind 19), a
//! cruise missile (24) or a cluster missile (25) lays a segment every frame
//! from where it was (`+0x40`) to where it is - the timer is never wound
//! back up for these. Other kinds do other things on a sixteenth-second clock
//! (`0x4010e0`, `0x401460`) that this does not cover.
//!
//! A segment goes into a pool of 100 at `0x612430`, 48 bytes each, taken in
//! turn whether in use or not (`0x478f00`). It keeps both ends, its heading
//! and pitch from one to the other, a two-second life (`+0x2c`), a start
//! delay of an eighth of a second (`+0x28`) so it does not sit over the
//! missile, and a width of `0x61a8`, 0.38 units (`+0x20`). `0x479480` ages
//! it and draws it once past the delay; `0x479040` draws a triangular tube
//! along it, textured `puff4.raw`, whose radius is the width times the part
//! of its life still to come - so a trail thins to nothing from its old end.

use hb_formats::fixed::to_units;

/// The pool's size.
pub const SEGMENTS: usize = 100;

/// Seconds a segment lasts (`0x20000`).
pub const LIFE: f32 = 2.0;

/// Seconds before it shows (`0x2000`).
pub const DELAY: f32 = 0.125;

/// Its radius when new, units (`0x61a8`).
pub const WIDTH: f32 = to_units(25_000);

/// Seconds after launch before a missile starts smoking (`0x3000`).
pub const FIRST: f32 = 0.1875;

/// The missile kinds that smoke this way.
pub const SMOKING: [i32; 3] = [19, 24, 25];

/// The tube runs a hundredth longer than the segment, so neighbours overlap
/// (`0x1028f`).
pub const OVERLAP: f32 = to_units(66_191);

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Segment {
    pub from: [f32; 3],
    pub to: [f32; 3],
    /// Seconds since it was laid (`+0x24`).
    pub clock: f32,
    /// Seconds it lasts; zero once it is gone (`+0x2c`).
    pub life: f32,
}

impl Segment {
    /// Its radius now, if it is showing: the width times the life left.
    pub fn radius(&self) -> Option<f32> {
        (self.life > 0.0 && self.clock > DELAY).then(|| WIDTH * (self.life - self.clock) / self.life)
    }
}

#[derive(Debug, Clone)]
pub struct Smoke {
    pub segments: Vec<Segment>,
    /// The slot the next one takes (`0x50e6f8`).
    next: usize,
}

impl Default for Smoke {
    fn default() -> Smoke {
        Smoke { segments: vec![Segment::default(); SEGMENTS], next: 0 }
    }
}

impl Smoke {
    /// Lay a segment, over whatever was in the next slot.
    pub fn lay(&mut self, from: [f32; 3], to: [f32; 3]) {
        self.segments[self.next] = Segment { from, to, clock: 0.0, life: LIFE };
        self.next = (self.next + 1) % SEGMENTS;
    }

    /// A frame: every live segment ages, and one past its life goes.
    pub fn step(&mut self, dt: f32) {
        for s in &mut self.segments {
            if s.life > 0.0 {
                s.clock += dt;
                if s.clock > s.life {
                    s.life = 0.0;
                }
            }
        }
    }

    /// The segments showing, with their radii.
    pub fn shown(&self) -> impl Iterator<Item = (&Segment, f32)> {
        self.segments.iter().filter_map(|s| s.radius().map(|r| (s, r)))
    }
}

/// A missile's own part in it: where it was, and its timer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Trail {
    pub last: [f32; 3],
    pub wait: f32,
}

impl Trail {
    /// For a missile launched at `at` (`0x477929`, `0x477939`).
    pub fn new(at: [f32; 3]) -> Trail {
        Trail { last: at, wait: FIRST }
    }

    /// A frame of a missile of `kind` now at `at`: the segment to lay, if
    /// any.
    pub fn step(&mut self, kind: i32, at: [f32; 3], dt: f32) -> Option<([f32; 3], [f32; 3])> {
        self.wait -= dt;
        if self.wait >= 0.0 || !SMOKING.contains(&kind) {
            return None;
        }
        let from = std::mem::replace(&mut self.last, at);
        Some((from, at))
    }
}
