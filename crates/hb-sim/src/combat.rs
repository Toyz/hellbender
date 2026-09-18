//! Shots, hits and destruction.
//!
//! What comes from the data: which model a destroyed object becomes, which
//! sound it makes, its per-weapon damage multipliers, and the value this port
//! reads as its hit points. What does not: how fast a shot flies, how long it
//! lives, and how much a laser does. Each choice is named where it is made.

use hb_formats::text::{EnemyDef, Placement};

/// Units per second. Not read from the engine.
pub const SHOT_SPEED: f32 = 180.0;
/// Seconds before a shot that hits nothing is dropped. Not read from the
/// engine; long enough to cross most of the draw distance.
pub const SHOT_LIFE: f32 = 1.4;
/// What one laser hit does before the target's own multiplier. Not read from
/// the engine.
pub const LASER_DAMAGE: f32 = 1.0;

#[derive(Debug, Clone, Copy)]
pub struct Shot {
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    pub age: f32,
}

impl Shot {
    /// Fired from `from` along the heading and pitch, in the engine's
    /// convention: heading 0 along +z increasing toward +x, positive pitch
    /// nose down.
    pub fn fire(from: [f32; 3], heading: u16, pitch: u16) -> Shot {
        let h = heading as f32 * std::f32::consts::TAU / 65536.0;
        let p = (pitch as i16) as f32 * std::f32::consts::TAU / 65536.0;
        let dir = [h.sin() * p.cos(), -p.sin(), h.cos() * p.cos()];
        Shot {
            position: from,
            velocity: [dir[0] * SHOT_SPEED, dir[1] * SHOT_SPEED, dir[2] * SHOT_SPEED],
            age: 0.0,
        }
    }

    pub fn alive(&self) -> bool {
        self.age < SHOT_LIFE
    }
}

/// A placed object's standing in a fight.
#[derive(Debug, Clone, Copy)]
pub struct Health {
    pub hit_points: f32,
    pub laser_multiplier: f32,
    pub destroyed: bool,
}

impl Health {
    /// Hit points from the type record.
    ///
    /// Field 2 of a `.DEF` type's first line is a function of the model - 242
    /// of 250 models take one value - which is the shape hit points should
    /// have, and read as 16.16 it orders sensibly: a cube 0.55, a bunker 4.65,
    /// a control centre 11.9. That is an inference, not a reading of the
    /// engine; it is the best available and it is labelled as such.
    pub fn for_kind(kind: &EnemyDef) -> Health {
        let hp = (kind.fields[2] as f32 / 65536.0).max(0.25);
        // The damage lines are cannon, laser, missile, in 16.16. 1.0 in all
        // but a few types.
        let laser = kind.damage[1] as f32 / 65536.0;
        Health { hit_points: hp, laser_multiplier: laser, destroyed: false }
    }

    /// Apply a laser hit. Returns true on the hit that destroys it.
    pub fn hit(&mut self) -> bool {
        if self.destroyed {
            return false;
        }
        self.hit_points -= LASER_DAMAGE * self.laser_multiplier;
        if self.hit_points <= 0.0 {
            self.destroyed = true;
            return true;
        }
        false
    }
}

/// What a destroyed object turns into.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Wreck {
    /// A real wreck model: `wbnkruin.bin` for the bunkers, five dome ruins.
    Model(String),
    /// Nothing left. The type names `cube.bin`, which is the un-normalised
    /// test model from entry 16 and reads as the exporter's placeholder for
    /// "no wreck" - 1,394 of the 1,848 types use it. That reading is an
    /// inference.
    Gone,
}

pub fn wreck_of(kind: &EnemyDef) -> Wreck {
    if kind.wreck.eq_ignore_ascii_case("cube.bin") {
        Wreck::Gone
    } else {
        Wreck::Model(kind.wreck.clone())
    }
}

/// The index of the first placement a shot passes through this step, if any.
///
/// A placed object is treated as a sphere of radius `scale`: models are
/// normalised to +/-1.0, so the placement's scale is its half-extent.
pub fn first_hit(
    shot: &Shot,
    dt: f32,
    placements: &[Placement],
    alive: impl Fn(usize) -> bool,
) -> Option<usize> {
    let from = shot.position;
    let to = [
        from[0] + shot.velocity[0] * dt,
        from[1] + shot.velocity[1] * dt,
        from[2] + shot.velocity[2] * dt,
    ];
    let seg = [to[0] - from[0], to[1] - from[1], to[2] - from[2]];
    let len_sq = seg.iter().map(|v| v * v).sum::<f32>().max(1e-9);

    let mut best: Option<(usize, f32)> = None;
    for (i, p) in placements.iter().enumerate() {
        if !alive(i) {
            continue;
        }
        let centre = [p.x as f32 / 65536.0, p.y as f32 / 65536.0, p.z as f32 / 65536.0];
        let radius = (p.scale as f32 / 65536.0).max(0.5);
        // Closest point on the segment to the centre.
        let t = ((0..3).map(|k| (centre[k] - from[k]) * seg[k]).sum::<f32>() / len_sq).clamp(0.0, 1.0);
        let d_sq: f32 = (0..3).map(|k| (from[k] + seg[k] * t - centre[k]).powi(2)).sum();
        if d_sq <= radius * radius && best.is_none_or(|(_, bt)| t < bt) {
            best = Some((i, t));
        }
    }
    best.map(|(i, _)| i)
}

pub fn advance(shot: &mut Shot, dt: f32) {
    for k in 0..3 {
        shot.position[k] += shot.velocity[k] * dt;
    }
    shot.age += dt;
}
