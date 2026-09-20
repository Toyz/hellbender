//! Class 26: the things that hang in the air and turn.
//!
//! 523 of the placements across the 26 levels are this class - the asteroids,
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
