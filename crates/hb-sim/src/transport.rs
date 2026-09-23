//! The transports, behaviour classes 50, 51 and 52: `logicTransportDisappear`
//! (`0x421d90`), `logicTransportTakeoffLand` (`0x4220f0`) and
//! `logicTransportTakeoffLandLeave` (`0x422790`). Sixteen placements: the
//! Rishi in `IOWAH2` and the Coalition shuttle and transport in `IOWAH3`
//! (50), a troop transport in `IOWAH2` and two recon craft in `ROID3` (51),
//! and the ten "Shipping out!" transports of `FLOAT` and `FLOAT2` (52).
//!
//! All three fly their course with [the steering](crate::steer) at half
//! thrust, and move on from a point once they have passed it (`0x423ca0`).
//!
//! **Class 50** finds the nearest point and flies to it - or, on a course
//! whose ground flag is set, is put on it - then follows the course. At the
//! last point, within eight units on every axis, it is gone (`+0x1c`
//! cleared): a burst (`0x4017d0`) and its escape sound, the Rishi's "safe".
//!
//! **Class 51** takes off first: four units a second straight up, turning a
//! sixteenth of a circle a second, for four seconds (phase 400). Then it
//! flies to its point and along the course. Within 24 units of the course's
//! end - the last point going forward, the first coming back - it stops
//! thrusting and turns toward the player (phase 1001, the steering at speed
//! 0 at a point eight units along its own nose from the player) until its
//! forward speed has all but gone; comes down at four units a second,
//! levelling out, until it is on the floor plus the type's `+0x14` (1000);
//! turns round, sits four seconds (2000), and takes off again. Then, on a
//! plain course, it goes nowhere: its target is still the last point, the
//! leg behind it (`0x423ca0`, the target plus one folded back) is that same
//! point, and a leg of no length is never passed, so it circles the last
//! point for good. All three shipped are on plain courses. On a looping one
//! it would ply the course end to end.
//!
//! **Class 52** is class 51 with two differences and an ending. It flies to
//! its first point with `0x407960` rather than the steering, aiming with
//! `0x423b60`; it lands only at the course's last point; and after the four
//! seconds on the ground it takes off again (2001) and climbs to the sky
//! layer (3000). Within eight units of it, it is gone - burst, escape sound -
//! and **the level is lost** (`0x422ec7` sets `0x512720`). The shipped ones
//! are the enemy's: "Transport has escaped."

use hb_formats::course::Course;
use hb_formats::fixed::{from_units, signed, to_units, TURN};
use hb_formats::text::{EnemyDef, Placement};
use hb_formats::vector::{add, scale, within};

use crate::combat::position_of;
use crate::course::{aim, bias, nearest, passed, Walk};
use crate::steer::{Body, Mode, Order, Surfaces};

/// Close enough to a point on every axis: classes 50 and 52 (`0x7fee0`, just
/// under eight units) and class 51 (`0x17fca0`, just under 24).
pub const NEAR: f32 = to_units(0x7fee0);
pub const NEAR_51: f32 = to_units(0x17fca0);

/// The thrust factor every transport hands the steering (`0x3f000000`).
pub const THRUST: f32 = 0.5;

/// Taking off and landing: four units a second up or down (`0x40000`),
/// turning a sixteenth of a circle a second (`0x1000`), four seconds of it.
pub const CLIMB: f32 = 4.0;
pub const SPIN: f32 = 4096.0;
pub const SPELL: f32 = 4.0;

/// Braking before landing ends once the forward speed is this or less
/// (`0x14`, in 16.16).
pub const STOPPED: f32 = to_units(0x14);

/// Where a braking transport looks: this far along its own nose from the
/// player (`0x80000`).
pub const LOOK: f32 = 8.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// 50: to the end of its course, and gone.
    Disappear,
    /// 51: back and forth, landing at each end.
    Shuttle,
    /// 52: out and back, then up and away.
    Leave,
}

impl Kind {
    pub fn of(class: i64) -> Option<Kind> {
        match class {
            50 => Some(Kind::Disappear),
            51 => Some(Kind::Shuttle),
            52 => Some(Kind::Leave),
            _ => None,
        }
    }
}

/// What a frame of a transport can end in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gone {
    /// It reached where it was going and left the level.
    Away,
    /// It reached the sky, and the level is lost.
    Escaped,
}

#[derive(Debug, Clone)]
pub struct Transport {
    pub kind: Kind,
    pub body: Body,
    /// The actor's `+0x64`.
    pub phase: u32,
    /// Seconds in a timed phase (`+0x70`).
    clock: f32,
    pub walk: Walk,
    points: Vec<[f32; 3]>,
    /// The course record's first number: put on the first point rather than
    /// flown to it (class 50 only).
    ground: bool,
    /// Units a second, and circle units a second: the type's `+0x64` and
    /// `+0x68` with the actor's bias (`+0x74`, `+0x78`).
    speed: f32,
    turn: f32,
    /// The type's `+0x14`, how far above the floor it stands on landing.
    height: f32,
    class: i32,
    clearance: f32,
}

impl Transport {
    /// The transport placed as `placement`, the `index`th in its level.
    /// `None` for a class that is not a transport or a course with no
    /// points.
    pub fn new(course: &Course, placement: &Placement, kind: &EnemyDef, index: usize) -> Option<Transport> {
        let which = Kind::of(kind.class())?;
        let points: Vec<[f32; 3]> = course.points().iter().map(|p| [p.x, p.y, p.z].map(to_units)).collect();
        if points.is_empty() {
            return None;
        }
        let (ground, periodic) = match course {
            Course::Points { ground, periodic, .. } => (*ground != 0, *periodic != 0),
            Course::Segments { .. } => (false, false),
        };
        let (speed_bias, turn_bias) = bias(index, kind.turn_rate);
        Some(Transport {
            kind: which,
            body: Body::new(position_of(placement), placement.heading as f32),
            phase: 0,
            clock: 0.0,
            walk: Walk::new(points.len(), periodic),
            points,
            ground,
            speed: to_units(kind.move_rate + speed_bias),
            turn: (kind.turn_rate + turn_bias) as f32,
            height: to_units(kind.fields[4] as i32),
            class: kind.class() as i32,
            clearance: to_units(kind.radius()),
        })
    }

    fn point(&self) -> [f32; 3] {
        self.points[self.walk.target]
    }

    fn near(&self, at: [f32; 3], reach: f32) -> bool {
        within(self.body.position, at, reach)
    }

    fn steer(&mut self, target: [f32; 3], speed: f32, world: &impl Surfaces, dt: f32) {
        let order = Order {
            target,
            mode: Mode::Toward,
            speed,
            turn: self.turn,
            thrust: THRUST,
            class: self.class,
            clearance: self.clearance,
            breaking: false,
            aim: None,
        };
        self.body.steer(&order, world, dt);
    }

    /// Follow the course: steer for the point, and on to the next once past
    /// it.
    fn follow(&mut self, world: &impl Surfaces, dt: f32) {
        self.steer(self.point(), self.speed, world, dt);
        if passed(&self.points, &self.walk, self.body.position) {
            self.walk.advance();
        }
    }

    /// Straight up or down at the climb rate, turning (phases 400, 1000,
    /// 2001).
    fn lift(&mut self, up: f32, dt: f32) {
        self.body.position[1] += up * CLIMB * dt;
        self.body.heading = (self.body.heading + SPIN * dt).rem_euclid(TURN);
    }

    /// Whether the four seconds of a timed phase are up.
    fn timed(&mut self, dt: f32) -> bool {
        self.clock += dt;
        if self.clock > SPELL {
            self.clock = 0.0;
            true
        } else {
            false
        }
    }

    /// One frame. `player` is where the player is (`0x5b3830`) and `sky` the
    /// sky layer's height (`0x5055d4`).
    pub fn step(&mut self, dt: f32, player: [f32; 3], sky: f32, world: &impl Surfaces) -> Option<Gone> {
        match (self.kind, self.phase) {
            (Kind::Disappear, 0) => {
                let points: Vec<[i32; 3]> = self.points.iter().map(|p| p.map(from_units)).collect();
                self.walk.target = nearest(&points, self.body.position.map(from_units));
                self.phase = 2;
            }
            (_, 0) => {
                self.phase = 400;
                self.clock = 0.0;
                self.body.pitch = 0.0;
            }
            (Kind::Disappear, 2) => {
                let point = self.point();
                if self.near(point, NEAR) {
                    self.phase = 3;
                }
                if self.ground {
                    self.body.position = point;
                    self.phase = 3;
                } else {
                    self.steer(point, self.speed, world, dt);
                }
            }
            (Kind::Shuttle, 2) => {
                let point = self.point();
                if self.near(point, NEAR_51) {
                    self.phase = 3;
                }
                self.steer(point, self.speed, world, dt);
            }
            (Kind::Leave, 2) => {
                let point = self.point();
                let wanted = aim(self.body.position, point);
                if self.near(point, NEAR) {
                    self.phase = 3;
                }
                self.body.glide(wanted, to_units(self.turn as i32), self.speed, self.clearance, world, dt);
            }
            (Kind::Disappear, 3) => {
                let gone = self.walk.at_last() && self.near(self.point(), NEAR);
                self.follow(world, dt);
                if gone {
                    return Some(Gone::Away);
                }
            }
            (Kind::Shuttle, 3) | (Kind::Leave, 3) => {
                let end = match self.kind {
                    Kind::Shuttle if self.walk.direction == 1 => self.walk.at_last(),
                    Kind::Shuttle => self.walk.target == 0,
                    _ => self.walk.at_last(),
                };
                let reach = if self.kind == Kind::Shuttle { NEAR_51 } else { NEAR };
                if end && self.near(self.point(), reach) {
                    self.phase = 1001;
                }
                self.follow(world, dt);
            }
            (_, 400) => {
                self.lift(1.0, dt);
                if self.timed(dt) {
                    self.phase = 2;
                }
            }
            (_, 1001) => {
                let look = add(player, scale(self.body.axes()[2], LOOK));
                self.steer(look, 0.0, world, dt);
                if self.body.velocity[2] <= STOPPED {
                    self.phase = 1000;
                    self.body.velocity[2] = 0.0;
                }
            }
            (_, 1000) => {
                self.lift(-1.0, dt);
                // Levelling out as it comes down: the pitch by at most the
                // turn rate's worth, and that again scaled by the frame
                // (`0x4225b4`, the frame time applied twice); the roll by
                // its whole self a second.
                let most = self.turn * dt;
                let pitch = signed(self.body.pitch);
                self.body.pitch = (pitch + (-pitch).clamp(-most, most) * dt).rem_euclid(TURN);
                let roll = signed(self.body.roll);
                self.body.roll = (roll - roll * dt).rem_euclid(TURN);
                let p = self.body.position;
                if world.floor(p) + self.height > p[1] {
                    self.phase = 2000;
                    self.clock = 0.0;
                    self.walk.direction = if self.walk.direction == 1 { -1 } else { 1 };
                }
            }
            (Kind::Leave, 2000) => {
                if self.timed(dt) {
                    self.phase = 2001;
                }
            }
            (_, 2000) => {
                if self.timed(dt) {
                    self.phase = 400;
                }
            }
            (Kind::Leave, 2001) => {
                self.lift(1.0, dt);
                if self.timed(dt) {
                    self.phase = 3000;
                }
            }
            (Kind::Leave, 3000) => {
                let p = self.body.position;
                let up = [p[0], sky, p[2]];
                let escaped = self.near(up, NEAR);
                self.steer(up, self.speed, world, dt);
                if escaped {
                    return Some(Gone::Escaped);
                }
            }
            _ => {}
        }
        None
    }
}
