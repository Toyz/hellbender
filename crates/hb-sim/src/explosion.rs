//! Explosions: the puffs of the engine's effect pool.
//!
//! `0x47f3f0` makes one out of eleven puffs: ten scattered within the size it
//! is given and one at the centre, all through `0x476f90`, which takes the
//! first free slot of the sixteen at `0x612230`. Each puff keeps a position, a
//! size, a clock and a rate; `0x4777c0` runs the clock and `0x4771e0` draws it
//! as a square facing the eye, textured with `blast1.raw` to `blast16.raw`,
//! one frame every sixteenth of its own second.

use crate::turret::Rng;

/// `blast%d.raw`, 1 to 16 (`0x50f3dc`).
pub const FRAMES: usize = 16;
/// A frame every `1 << 12` of the puff's clock (`0x4771f5`).
pub const FRAME_TIME: f32 = 0x1000 as f32 / 65536.0;
/// The life a puff is given (`0x4770cb`); the frames run out first.
pub const LIFE: f32 = 2.0;
/// The effect pool's sixteen slots (`0x612230` to `0x612448`).
pub const SLOTS: usize = 16;
/// How many puffs an explosion is: ten around it and one at the centre.
pub const PUFFS: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Puff {
    pub position: [f32; 3],
    /// Half the width of the square it is drawn as, units.
    pub size: f32,
    /// Its own clock, which runs at [`Puff::rate`] (`+0x14`).
    pub age: f32,
    /// A quarter to three quarters of real time (`0x476fe7`).
    pub rate: f32,
}

impl Puff {
    /// Which frame it shows, or `None` once it has run out.
    pub fn frame(&self) -> Option<usize> {
        let frame = (self.age / FRAME_TIME) as usize;
        (frame < FRAMES && self.age <= LIFE).then_some(frame)
    }

    /// Returns false once it is done.
    pub fn step(&mut self, dt: f32) -> bool {
        self.age += dt * self.rate;
        self.frame().is_some()
    }
}

/// Every puff burning at once, in the engine's sixteen slots.
#[derive(Debug, Clone, Default)]
pub struct Blasts {
    pub puffs: Vec<Puff>,
}

impl Blasts {
    /// One explosion of `size` units at `at` (`0x47f3f0`): ten puffs of twice
    /// the size scattered within it, then one of four times at the centre.
    /// The engine takes the first free slot and writes over the first slot
    /// when there is none, so a big explosion can cut an older one short.
    /// One puff, which is what most things make. `0x476f90` is called
    /// directly all over the engine - a shot's mark, a missile's, and what a
    /// destroyed actor leaves (`0x407c20`, which passes the type's own
    /// radius) - and each of those is a single square, not a burst. Only a
    /// handful of places go through [`Blasts::burst`].
    ///
    /// The rate is 1.0: `0x476fd3` only draws a random one for mode 2, which
    /// is the burst's.
    pub fn puff(&mut self, at: [f32; 3], size: f32) {
        self.light(Puff { position: at, size, age: 0.0, rate: 1.0 });
    }

    pub fn burst(&mut self, at: [f32; 3], size: f32, rng: &mut Rng) {
        for _ in 0..PUFFS {
            let offset = |rng: &mut Rng| (rng.next() as f32 / 65536.0 - 0.25) * size * 4.0;
            let (dx, dy, dz) = (offset(rng), offset(rng), offset(rng));
            let position = [at[0] + dx, at[1] + dy, at[2] + dz];
            self.light(Puff { position, size: size * 2.0, age: 0.0, rate: rate(rng) });
        }
        self.light(Puff { position: at, size: size * 4.0, age: 0.0, rate: rate(rng) });
    }

    fn light(&mut self, puff: Puff) {
        if self.puffs.len() < SLOTS {
            self.puffs.push(puff);
        } else {
            self.puffs[0] = puff;
        }
    }

    pub fn step(&mut self, dt: f32) {
        self.puffs.retain_mut(|p| p.step(dt));
    }
}

/// A quarter to three quarters (`rand() + 0x4000`).
fn rate(rng: &mut Rng) -> f32 {
    (rng.next() as f32 + 0x4000 as f32) / 65536.0
}
