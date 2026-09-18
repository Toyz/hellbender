//! Following a course.
//!
//! The engine's follow logic at `HELLBEND.EXE:0x00421240` is a phase machine
//! over the actor's `+0x64` field. Phase 0 seeds a best distance of
//! `0x40000000`, walks every point of the course, and keeps the nearest - so
//! an actor does not have to be placed on its course. It joins it at whichever
//! point is closest to where it stands. That is why only 202 of the 1,301
//! placements that name a course sit within a unit of it: the rest are meant
//! to fly to it.
//!
//! Phase 2 then follows the course. What the engine does at the far end - and
//! how fast it goes - has not been read, so both are this crate's choice and
//! are named as such.

use hb_formats::course::{Course, Point};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// Heading for the nearest point on the course, from wherever the actor
    /// was placed. The engine's phase 0 chooses it; this is the flight there.
    Joining,
    /// Moving from point to point.
    Following,
    /// A course that does not loop has run out.
    Finished,
}

/// Units per second. The engine's source for speed has not been read - a
/// type's `.DEF` line 1 has a field that reads as 15 to 30 units a second in
/// half the records and zero in the other half - so this is a single chosen
/// value.
pub const DEFAULT_SPEED: f32 = 20.0;

#[derive(Debug, Clone)]
pub struct Follower {
    pub points: Vec<[f32; 3]>,
    pub periodic: bool,
    /// World units, as floats for the simulation's own arithmetic. The files
    /// are 16.16; `position_fixed` converts back.
    pub position: [f32; 3],
    pub target: usize,
    pub phase: Phase,
    pub speed: f32,
}

fn units(p: Point) -> [f32; 3] {
    [p.x as f32 / 65536.0, p.y as f32 / 65536.0, p.z as f32 / 65536.0]
}

fn distance_sq(a: [f32; 3], b: [f32; 3]) -> f32 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum()
}

impl Follower {
    /// An actor placed at `start` that follows `course`. `None` for a course
    /// with no points, which the engine treats as fatal ("No course points for
    /// course").
    pub fn new(course: &Course, start: [i32; 3]) -> Option<Follower> {
        let points: Vec<[f32; 3]> = course.points().into_iter().map(units).collect();
        if points.is_empty() {
            return None;
        }
        let periodic = match course {
            Course::Points { periodic, .. } => *periodic != 0,
            Course::Segments { .. } => false,
        };
        let position = [
            start[0] as f32 / 65536.0,
            start[1] as f32 / 65536.0,
            start[2] as f32 / 65536.0,
        ];
        // Phase 0: the nearest point, exactly as the engine chooses it.
        let target = points
            .iter()
            .enumerate()
            .min_by(|a, b| {
                distance_sq(position, *a.1).total_cmp(&distance_sq(position, *b.1))
            })
            .map(|(i, _)| i)
            .unwrap_or(0);
        Some(Follower {
            points,
            periodic,
            position,
            target,
            phase: Phase::Joining,
            speed: DEFAULT_SPEED,
        })
    }

    /// Advance by `dt` seconds.
    pub fn step(&mut self, dt: f32) {
        if self.phase == Phase::Finished {
            return;
        }
        let mut budget = self.speed * dt;
        // Spend the whole step's distance, possibly passing several points.
        while budget > 0.0 && self.phase != Phase::Finished {
            let goal = self.points[self.target];
            let gap = distance_sq(self.position, goal).sqrt();
            if gap > budget {
                for i in 0..3 {
                    self.position[i] += (goal[i] - self.position[i]) / gap * budget;
                }
                return;
            }
            self.position = goal;
            budget -= gap;
            self.phase = Phase::Following;
            self.advance_target();
        }
    }

    fn advance_target(&mut self) {
        if self.points.len() == 1 {
            // A one-point course is a place to be, not a path.
            self.phase = Phase::Finished;
            return;
        }
        if self.target + 1 < self.points.len() {
            self.target += 1;
        } else if self.periodic {
            self.target = 0;
        } else {
            self.phase = Phase::Finished;
        }
    }

    /// The heading of travel, in the engine's convention: 0 along +z and
    /// increasing toward +x, the same as `atan2(dx, dz)`.
    pub fn heading(&self) -> u16 {
        let goal = self.points[self.target];
        let (dx, dz) = (goal[0] - self.position[0], goal[2] - self.position[2]);
        if dx == 0.0 && dz == 0.0 {
            return 0;
        }
        let turns = dx.atan2(dz) / std::f32::consts::TAU;
        (turns.rem_euclid(1.0) * 65536.0) as u16
    }

    pub fn position_fixed(&self) -> [i32; 3] {
        [
            (self.position[0] * 65536.0) as i32,
            (self.position[1] * 65536.0) as i32,
            (self.position[2] * 65536.0) as i32,
        ]
    }
}
