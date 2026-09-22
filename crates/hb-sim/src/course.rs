//! Following a course: behaviour class 47, the engine's
//! `logicFollowGroundPath` (`HELLBEND.EXE:0x421240`).
//!
//! Only the course classes read a course - 46, 47, 48, 50, 51, 52 and 62, the
//! seven routines that look one up through `0x49da10`. A type of any other
//! class can name a course in its `.DEF` and the engine never reads it. Class
//! 47 is 285 of the 302 placements that do: the cars, boats and morbots, the
//! T-rexes, the Kraaken. The other six - 17 placements, the transports and
//! `FX4` - have routines of their own that are not read yet; this crate runs
//! them on class 47's, which is its choice and not the engine's.
//!
//! It is a phase machine over the actor's `+0x64`:
//!
//! - **0** walks every point of the course and keeps the nearest (`0x42b960`,
//!   world-wrapped straight-line distance), and goes to phase 2.
//! - **2** puts the actor *on* that point - its position becomes the point's -
//!   and goes to phase 3. So an actor placed away from its course does not fly
//!   to it: it is there on its third frame.
//! - **3** is `0x4231b0`, every frame after.
//!
//! Phase 3 steers rather than slides. It aims at the point it is heading for
//! (`0x423b60`, `atan2` of the world-wrapped difference) and eases its heading
//! toward that by the type's turn rate, then moves along the heading it had,
//! at the type's move rate. Within eight units of the point in both x and z it
//! takes the next one. The height is not the course's: it is the floor under
//! the actor (`0x41c300`) plus the type's `+0x14`, which is zero for every
//! class 47 type shipped - the cars sit on the ground and climb its hills.
//!
//! At the end of a course a looping one (`periodic`, record `+0x08`) goes back
//! to its first point, and one that does not turns round and comes back.
//!
//! Every actor also gets a small, fixed difference in speed and turn from its
//! place in the level (`0x404e90`): the placement index's low two bits pick 0,
//! +1/4, -1/2 or -1/4 of a unit a second, so a column of identical cars on the
//! same course spreads out.
//!
//! The arithmetic is the engine's 16.16 throughout.

use hb_formats::course::Course;
use hb_formats::fixed::{cos, distance, mul, sin, wrap};
use hb_formats::text::{EnemyDef, Placement};

/// The classes whose routines read a course.
pub const COURSE_CLASSES: [i64; 7] = [46, 47, 48, 50, 51, 52, 62];

/// The course classes that shoot as well: their routines end in the
/// turrets' trigger, `0x407770`, with a range it always passes. The
/// transports - 50, 51, 52 - do not call it.
pub const SHOOTING: [i64; 4] = [46, 47, 48, 62];

/// How close counts as at the point, in x and in z separately (`0x423225`).
pub const REACHED: i32 = 8 << 16;

/// The actor's `+0x64`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// 0: about to choose the nearest point.
    Start,
    /// 2: about to be put on it.
    Placed,
    /// 3: following.
    Following,
}

#[derive(Debug, Clone)]
pub struct Follower {
    points: Vec<[i32; 3]>,
    periodic: bool,
    /// 16.16 world units, the actor's `+0x00..+0x08`.
    pub position: [i32; 3],
    /// The engine's 16-bit circle, `+0x14`.
    pub heading: i32,
    /// `+0x0c`, which scales the step; zero for every shipped placement.
    pub pitch: i32,
    /// The heading it is turning toward, `+0x48`.
    pub wanted: i32,
    /// The point it is heading for, `+0x140`.
    pub target: usize,
    /// `+0x144`: 1 walks the course forward, anything else backward.
    pub direction: i32,
    pub phase: Phase,
    /// 16.16 units a second: the type's move rate and the actor's own bias.
    pub speed: i32,
    /// 16.16 a second: the type's turn rate and the actor's own bias.
    pub turn: i32,
    /// The type's `+0x14`, added to the floor.
    pub height: i32,
}

/// The speed and turn bias `0x404e90` gives the actor at `index` in the level:
/// `(index << 30) >> 16`. The turn gets half of it when the whole would not
/// be positive.
pub fn bias(index: usize, turn_rate: i32) -> (i32, i32) {
    let b = ((index as i32) << 30) >> 16;
    let turn = if turn_rate + b <= 0 { b / 2 } else { b };
    (b, turn)
}

impl Follower {
    /// The actor placed as `placement`, the `index`th in its level, of type
    /// `kind`. `None` for a course with no points, which the engine treats as
    /// fatal ("No course points for course").
    pub fn new(course: &Course, placement: &Placement, kind: &EnemyDef, index: usize) -> Option<Follower> {
        let points: Vec<[i32; 3]> = course.points().into_iter().map(|p| [p.x, p.y, p.z]).collect();
        if points.is_empty() {
            return None;
        }
        let periodic = match course {
            Course::Points { periodic, .. } => *periodic != 0,
            // The engine's loader reads only the point form; a segment
            // course, whatever it would have been, never loops.
            Course::Segments { .. } => false,
        };
        let (speed_bias, turn_bias) = bias(index, kind.turn_rate);
        Some(Follower {
            points,
            periodic,
            position: [wrap(placement.x), placement.y, wrap(placement.z)],
            heading: placement.heading as i32,
            pitch: placement.pitch,
            wanted: 0,
            target: 0,
            direction: 1,
            phase: Phase::Start,
            speed: kind.move_rate + speed_bias,
            turn: kind.turn_rate + turn_bias,
            height: kind.fields[4] as i32,
        })
    }

    /// One frame of `dt` seconds. `floor` is the surface under a point in
    /// units (`0x41c300`), given the point.
    pub fn step(&mut self, dt: f32, floor: impl Fn([f32; 3]) -> f32) {
        match self.phase {
            Phase::Start => {
                let mut best = 0x4000_0000;
                for (i, &p) in self.points.iter().enumerate() {
                    let d = distance(self.position, p);
                    if best > d {
                        best = d;
                        self.target = i;
                    }
                }
                self.phase = Phase::Placed;
            }
            Phase::Placed => {
                self.position = self.points[self.target];
                self.phase = Phase::Following;
            }
            Phase::Following => self.follow(dt, floor),
        }
    }

    /// `0x423b60`: aim at a point.
    fn aim(&mut self, at: [i32; 3]) {
        let dx = wrap(at[0].wrapping_sub(self.position[0])) as f64;
        let dz = wrap(at[2].wrapping_sub(self.position[2])) as f64;
        self.wanted = (dx.atan2(dz) * (65536.0 / std::f64::consts::TAU)) as i32;
    }

    /// The next point along, `0x423264`: forward or back by `direction`, and
    /// at an end either round to the other end or back the way it came.
    fn next(&mut self) {
        let n = self.points.len() as i32;
        let was = self.target as i32 + if self.direction == 1 { 1 } else { -1 };
        let now = if self.periodic {
            if was < 0 {
                n - was.abs() % n
            } else if was >= n {
                was % n
            } else {
                was
            }
        } else if was < 0 {
            was.abs() % n
        } else if was >= n {
            n - was % n - 1
        } else {
            was
        };
        if now != was && !self.periodic {
            self.direction = -self.direction;
        }
        self.target = now as usize;
    }

    /// `0x4231b0`.
    fn follow(&mut self, dt: f32, floor: impl Fn([f32; 3]) -> f32) {
        let point = self.points[self.target];
        self.aim(point);
        self.position[1] = point[1];
        let dx = wrap(point[0].wrapping_sub(self.position[0])).abs();
        let dz = wrap(point[2].wrapping_sub(self.position[2])).abs();
        if dx < REACHED && dz < REACHED {
            self.next();
            self.aim(self.points[self.target]);
        }

        let dt = (dt * 65536.0) as i32;
        let turn = mul(self.turn, dt);
        let step = mul(self.speed, dt);
        // The step goes along the heading the actor had at the start of the
        // frame; the turn applies after.
        let across = mul(sin(self.heading), cos(self.pitch));
        let along = mul(cos(self.heading), cos(self.pitch));
        let error = ((self.wanted - self.heading) << 16) >> 16;
        self.heading = (self.heading + mul(error, turn)) & 0xffff;

        self.position[0] += mul(step, across);
        // The floor is found after x moves and before z does.
        let at = [
            self.position[0] as f32 / 65536.0,
            self.position[1] as f32 / 65536.0,
            self.position[2] as f32 / 65536.0,
        ];
        self.position[1] = (floor(at) * 65536.0) as i32 + self.height;
        self.position[2] += mul(step, along);
    }

    pub fn heading(&self) -> u16 {
        self.heading as u16
    }

    /// Where it is, wrapped into the world.
    pub fn position_fixed(&self) -> [i32; 3] {
        [wrap(self.position[0]), self.position[1], wrap(self.position[2])]
    }
}
