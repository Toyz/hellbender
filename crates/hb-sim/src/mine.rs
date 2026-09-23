//! The floating mine, weapon 28.
//!
//! The one weapon that is not fired but left behind. Dropping one
//! (`0x47d82a`) tests the ship's speed against `0x61a80` - 6.1 units a second
//! - and refuses below it with `Go Faster to Deploy Mine`. Above it the mine
//! goes down at the ship's position less the third row of its rotation, a
//! unit behind it, into the first free slot of a hundred at `0x61bde0`.
//!
//! `0x479670` runs the pool once a frame. A mine arms when the **local ship**
//! comes within 16 units of it, per axis and wrapped for the world's edge;
//! armed, it turns its two angles at a quarter and an eighth of a circle a
//! second, and when the ship is inside again it splashes 32 units of its
//! damage and clears its slot.
//!
//! Nothing else is tested - not the actors, not other shots. In a network
//! game that is what makes it a weapon, because the pool holds the mines
//! other players dropped and the ship it tests is yours. In a single-player
//! level only the player who laid it can set it off, and what it kills is
//! whatever else is inside the blast.
//!
//! See `docs/engine/simulation.md`.

use hb_formats::fixed::to_units;
use hb_formats::vector::{scale, sub, within};

/// Slower than this and the drop is refused: `0x61a80` in 16.16.
pub const SPEED: f32 = to_units(400_000);

/// How far behind the ship it goes: one row of the ship's rotation.
pub const BEHIND: f32 = 1.0;

/// A hundred slots of 48 bytes at `0x61bde0`.
pub const SLOTS: usize = 100;

/// It arms and goes off within this, per axis (`0x47d91c`).
pub const TRIGGER: f32 = 16.0;

/// And splashes this far (`0x47d923`, `0x4798a2`).
pub const BLAST: f32 = 32.0;

/// The weapon kind the splash is dealt as (`0x47988a`).
pub const KIND: i32 = 26;

/// The explosion it draws, in units: `0x186a0` in 16.16 (`0x479a8f`).
pub const BURST: f32 = to_units(100_000);

/// The two angles of an armed mine, in the 16-bit circle a second
/// (`0x4796f2`, `0x479711`).
pub const SPIN: [f32; 2] = [16_384.0, 8_192.0];

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mine {
    pub at: [f32; 3],
    /// The two the engine turns; the drawer uses them as pitch and heading.
    pub angles: [f32; 2],
    pub damage: f32,
    pub armed: bool,
}

/// A mine that has just gone off, for the caller to splash and draw.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Blast {
    pub at: [f32; 3],
    pub damage: f32,
}

/// The hundred slots.
#[derive(Debug, Clone, Default)]
pub struct Field {
    pub slots: Vec<Option<Mine>>,
}

impl Field {
    pub fn new() -> Field {
        Field { slots: vec![None; SLOTS] }
    }

    /// Lay one behind the ship. `back` is the ship's forward direction, which
    /// the engine subtracts a row of. `None` when the ship is too slow or the
    /// hundred slots are full - the engine says nothing about the second, it
    /// simply does nothing.
    pub fn lay(&mut self, at: [f32; 3], back: [f32; 3], speed: f32, damage: f32) -> Option<usize> {
        if speed < SPEED {
            return None;
        }
        let free = self.slots.iter().position(Option::is_none)?;
        self.slots[free] = Some(Mine {
            at: sub(at, scale(back, BEHIND)),
            angles: [0.0; 2],
            damage,
            armed: false,
        });
        Some(free)
    }

    /// Whether the ship is moving fast enough to lay one at all, which is
    /// what the refusal on the HUD is about.
    pub fn fast_enough(speed: f32) -> bool {
        speed >= SPEED
    }

    /// One frame. Returns the mines that went off.
    pub fn step(&mut self, dt: f32, ship: [f32; 3]) -> Vec<Blast> {
        let mut blasts = Vec::new();
        for slot in &mut self.slots {
            let Some(mine) = slot.as_mut() else { continue };
            let near = near(mine.at, ship);
            if !mine.armed {
                mine.armed = near;
            } else {
                mine.angles[0] = (mine.angles[0] + SPIN[0] * dt).rem_euclid(65536.0);
                mine.angles[1] = (mine.angles[1] + SPIN[1] * dt).rem_euclid(65536.0);
            }
            // The arming test and the trigger test are the same test, one
            // after the other, so a mine can arm and go off in one frame.
            if mine.armed && near {
                blasts.push(Blast { at: mine.at, damage: mine.damage });
                *slot = None;
            }
        }
        blasts
    }

    pub fn live(&self) -> impl Iterator<Item = &Mine> {
        self.slots.iter().flatten()
    }
}

/// The engine's test: each axis apart, wrapped for the world's edge, against
/// the trigger radius (`0x4797cb`).
fn near(mine: [f32; 3], ship: [f32; 3]) -> bool {
    within(ship, mine, TRIGGER)
}

/// The mines the enemy lays, which are a different thing in a different
/// pool: a hundred slots of 48 bytes at `0x5c00e0`, stepped by `0x495de0`
/// and written only by the mine layers' own AI (class 55, [`crate::flyer`]).
///
/// It does not arm, spin or wait. Each frame `0x479b60` tests the player
/// against the mine's own radius on each of the three axes, wrapped at the
/// world's edge; inside all three, the player takes the laying type's shot
/// damage scaled by how far in he is, and the mine is gone.
pub mod laid {
    use hb_formats::vector::{distance, flat_length, offset, within};

    /// A hundred slots, as the player's pool has.
    pub const SLOTS: usize = super::SLOTS;

    /// The least a hit can be scaled to, whatever the distance
    /// (`0x495e93`): a quarter.
    pub const LEAST: f32 = 0.25;

    /// The explosion it leaves, in units (`0x495ed5`) - the same size the
    /// player's mine draws.
    pub const BURST: f32 = super::BURST;

    /// How far from the player it refuses to be laid at all
    /// (`0x496666`): eight units across, measured flat.
    pub const CLEAR: f32 = 8.0;

    /// Where in front of the player it is laid (`0x49658f`): 24 units along
    /// the player's own nose, which is where he is about to be.
    pub const AHEAD: f32 = 24.0;

    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct Mine {
        pub at: [f32; 3],
        /// The laying type's retreat range, which is how far this reaches.
        pub radius: f32,
        /// The laying type's shot damage, before the falloff.
        pub damage: f32,
    }

    /// One that has gone off: what the player takes, and where to draw it.
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct Blast {
        pub at: [f32; 3],
        pub damage: f32,
    }

    #[derive(Debug, Clone, Default)]
    pub struct Field {
        pub slots: Vec<Option<Mine>>,
    }

    impl Field {
        pub fn new() -> Field {
            Field { slots: vec![None; SLOTS] }
        }

        /// Lay one. `None` when every slot is taken, which is all the engine
        /// does about it.
        pub fn lay(&mut self, at: [f32; 3], radius: f32, damage: f32) -> Option<usize> {
            let free = self.slots.iter().position(Option::is_none)?;
            self.slots[free] = Some(Mine { at, radius, damage });
            Some(free)
        }

        /// Whether a mine may be laid here at all: the flat distance from the
        /// player has to be over [`CLEAR`] (`0x496666`).
        pub fn clear_of(at: [f32; 3], player: [f32; 3]) -> bool {
            flat_length(offset(player, at)) > CLEAR
        }

        /// One frame against the player. Returns what went off.
        pub fn step(&mut self, player: [f32; 3]) -> Vec<Blast> {
            let mut blasts = Vec::new();
            for slot in &mut self.slots {
                let Some(mine) = *slot else { continue };
                // A box on each axis first, as the engine does, then the
                // real distance for the falloff.
                if !within(player, mine.at, mine.radius) {
                    continue;
                }
                let distance = distance(player, mine.at);
                let scale = if mine.radius > 0.0 {
                    ((mine.radius - distance).abs() / mine.radius).max(LEAST)
                } else {
                    LEAST
                };
                blasts.push(Blast { at: mine.at, damage: mine.damage * scale });
                *slot = None;
            }
            blasts
        }
    }
}
