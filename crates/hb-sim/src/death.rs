//! The ship's last few seconds.
//!
//! With the hull at zero the player's update stops flying the ship and runs
//! `0x464870` instead: the velocity is dropped, the nose pitches down and the
//! ship rolls, both a quarter of a turn a second, the pitch stopping at
//! `0x1fff` - forty-five degrees; the ship drifts forward at `0x7a120`, seven
//! and a half units a second. When it comes within two units of the ground it
//! explodes (`0x47f3f0` at two units) and the loadout goes back to what it
//! started with (`0x426e90`). After that `0x512618` gathers the frame time,
//! and the game loop ends the level once it passes five seconds
//! (`0x4822a4`).

use hb_formats::fixed::to_units;

/// A quarter of a turn a second: the frame time over four, in the 16-bit
/// circle (`0x464890`).
pub const TURN: f32 = 16384.0;
/// The nose stops here, forty-five degrees down (`0x4648a0`).
pub const PITCH_LIMIT: f32 = 0x1fff as f32;
/// Units a second it keeps drifting (`0x4648d5`).
pub const DRIFT: f32 = to_units(0x7a120);
/// It blows up this far over the ground (`0x46498b`).
pub const BLAST: f32 = 2.0;
/// And the level ends this long after (`0x4822a9`).
pub const WAIT: f32 = 5.0;

/// What the dying ship wants done.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wants {
    /// It has hit: an explosion two units across, and the loadout back.
    Explode,
    /// Five seconds have passed: the level is over.
    Over,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Wreck {
    /// The angles it tumbles through, in the 16-bit circle.
    pub pitch: f32,
    pub roll: f32,
    pub heading: f32,
    pub exploded: bool,
    /// Seconds since it did.
    pub since: f32,
}

impl Wreck {
    /// Start from the pose the ship had.
    pub fn new(pitch: f32, roll: f32, heading: f32) -> Wreck {
        Wreck { pitch, roll, heading, exploded: false, since: 0.0 }
    }

    /// One frame. `ground` is the surface under the ship.
    pub fn step(&mut self, position: &mut [f32; 3], dt: f32, ground: f32) -> Option<Wants> {
        if self.exploded {
            self.since += dt;
            return (self.since >= WAIT).then_some(Wants::Over);
        }
        self.pitch = (self.pitch + TURN * dt).min(PITCH_LIMIT);
        self.roll += TURN * dt;
        let forward = hb_formats::vector::direction(self.heading, self.pitch);
        for k in 0..3 {
            position[k] += forward[k] * DRIFT * dt;
        }
        if position[1] < ground + BLAST {
            self.exploded = true;
            return Some(Wants::Explode);
        }
        None
    }
}
