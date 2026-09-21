//! The small behaviour classes: the actors that only move.
//!
//! Each of the engine's 65 classes runs one routine a frame off a jump table
//! at `0x40c6cc`. Four of them are short enough to read at a sitting and
//! between them drive 626 of the placements across the levels.
//!
//! **Class 26** hovers. 523 of the placements are this class - the asteroids,
//! the Death Ankhs, the floating stones - and its routine (`0x40ab80`) is
//! four lines long. The first frame it remembers where the level put the
//! actor. Every frame after, it advances a phase and its heading by the frame
//! time over eight, and sets the actor's altitude to where it started plus
//! the sine of that phase times a quarter of its type's radius.
//!
//! So it bobs by a quarter of its own size and turns on the spot, both taking
//! eight seconds to come round: the frame time over eight is 8,192 of the
//! 16-bit circle a second, and the circle is 65,536.
//!
//! The class skips the visibility computation its neighbours in the jump
//! table run (`0x40c305` jumps straight past it), but the actor loop still
//! only thinks about what is within 80 units, so one far away holds still.
//!
//! **Class 17** leaves. 54 placements, named `Shipping out!` and
//! `Frigate and Container`: `0x40a230` lifts the actor at four units a second
//! and turns it a sixteenth of a circle a second, and once it is higher than
//! twice the [sky layer](../../../docs/formats/lvl.md) - the same ceiling a
//! dropped powerup is held under - clears the actor's `+0x1c` and it is gone.
//!
//! **Class 18** falls. 29 placements, all named some spelling of asteroid:
//! `0x40a2c0` drops one from half the sky layer's height above the ground,
//! scattered up to sixteen units from where the level put it, tumbling on all
//! three axes, at 64 units a second. When it reaches the ground it explodes
//! and starts again from the top. The explosion is offset by three type
//! fields at `+0x8c` that nothing in the executable ever writes, so it is at
//! the point it landed.

/// Angle units a second, for both the bob and the turn: the frame time over
/// eight (`0x40abad`).
pub const RATE: f32 = 8192.0;

/// A full circle, and so a full bob, in [`RATE`] seconds.
pub const CIRCLE: f32 = 65536.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hover {
    /// Where the level put it, which is what it bobs about (`0x40ab98`).
    pub start: [f32; 3],
    /// How far through the bob it is, in the 16-bit circle.
    pub phase: f32,
    pub heading: f32,
}

impl Hover {
    pub fn new(at: [f32; 3], heading: f32) -> Hover {
        Hover { start: at, phase: 0.0, heading }
    }

    /// One frame. `radius` is the type's own radius in units - the same value
    /// the model is drawn at - and the bob is a quarter of it.
    pub fn step(&mut self, dt: f32, radius: f32) -> ([f32; 3], f32) {
        self.phase = (self.phase + RATE * dt).rem_euclid(CIRCLE);
        self.heading = (self.heading + RATE * dt).rem_euclid(CIRCLE);
        let y = self.start[1] + sine(self.phase) * radius / 4.0;
        ([self.start[0], y, self.start[2]], self.heading)
    }
}

/// The engine's sine (`0x429ea0`): a 256-entry table over the 16-bit circle,
/// interpolated on the low byte. This is the same curve without the table.
pub fn sine(angle: f32) -> f32 {
    (angle * std::f32::consts::TAU / CIRCLE).sin()
}

/// What the world around an actor is, for the classes that need to know.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Around {
    /// The type's radius in units, which is how far class 26 bobs.
    pub radius: f32,
    /// The level's sky layer altitude in units, which class 17 leaves past
    /// and class 18 falls from.
    pub sky: f32,
    /// The surface under the actor.
    pub ground: f32,
}

/// Class 17: up, turning, and away. Four units a second (`0x40a251`) and a
/// sixteenth of a circle (`0x40a278`).
pub const RISE: f32 = 4.0;
pub const RISE_TURN: f32 = 4096.0;

/// Class 18: down at 64 units a second (`0x40a35a`), tumbling a whole circle
/// a second about one axis and a quarter about the other two.
pub const FALL: f32 = 64.0;
pub const TUMBLE: f32 = 65536.0;
pub const TUMBLE_SLOW: f32 = 16384.0;
/// How far from its placement an asteroid starts, either way on both axes:
/// `(rand() - 0x4000) * 64.0` in 16.16 (`0x40a315`).
pub const SCATTER: f32 = 16.0;

/// One actor's motion, for the classes that only move.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Motion {
    /// Class 26.
    Hovers(Hover),
    /// Class 17: it has left once `gone` is set.
    Leaves { at: [f32; 3], heading: f32, gone: bool },
    /// Class 18, which needs placing before its first fall.
    Falls { start: [f32; 3], at: [f32; 3], angles: [f32; 3], placed: bool },
}

/// Where an actor ended up this frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Moved {
    pub at: [f32; 3],
    /// Pitch, roll and heading, in the 16-bit circle.
    pub angles: [f32; 3],
    /// It has gone: stop drawing it (`0x40a2a6` clears the actor's `+0x1c`).
    pub gone: bool,
    /// It hit the ground here and starts again (`0x40a429`).
    pub blast: Option<[f32; 3]>,
}

impl Motion {
    /// The motion a class runs, or `None` for a class this module does not
    /// have.
    pub fn of(class: i64, at: [f32; 3], angles: [f32; 3]) -> Option<Motion> {
        match class {
            26 => Some(Motion::Hovers(Hover::new(at, angles[2]))),
            17 => Some(Motion::Leaves { at, heading: angles[2], gone: false }),
            18 => Some(Motion::Falls { start: at, at, angles, placed: false }),
            _ => None,
        }
    }

    pub fn step(&mut self, dt: f32, around: Around, rng: &mut crate::turret::Rng) -> Moved {
        match self {
            Motion::Hovers(hover) => {
                let (at, heading) = hover.step(dt, around.radius);
                Moved { at, angles: [0.0, 0.0, heading], gone: false, blast: None }
            }
            Motion::Leaves { at, heading, gone } => {
                at[1] += RISE * dt;
                *heading = (*heading + RISE_TURN * dt).rem_euclid(CIRCLE);
                // Twice the sky layer, which is the `.LVL`'s line 30 in
                // units, and it is out of the level (`0x40a298`).
                *gone |= at[1] > 2.0 * around.sky;
                Moved { at: *at, angles: [0.0, 0.0, *heading], gone: *gone, blast: None }
            }
            Motion::Falls { start, at, angles, placed } => {
                let mut blast = None;
                if !*placed {
                    *placed = true;
                    let spread = |rng: &mut crate::turret::Rng| {
                        (rng.next() as f32 - 16384.0) / 16384.0 * SCATTER
                    };
                    at[0] = start[0] + spread(rng);
                    at[2] = start[2] + spread(rng);
                    at[1] = around.ground + around.sky / 2.0;
                }
                at[1] -= FALL * dt;
                angles[0] = (angles[0] + TUMBLE_SLOW * dt).rem_euclid(CIRCLE);
                angles[1] = (angles[1] + TUMBLE * dt).rem_euclid(CIRCLE);
                angles[2] = (angles[2] + TUMBLE_SLOW * dt).rem_euclid(CIRCLE);
                if at[1] <= around.ground {
                    blast = Some(*at);
                    *placed = false;
                }
                Moved { at: *at, angles: *angles, gone: false, blast }
            }
        }
    }
}
