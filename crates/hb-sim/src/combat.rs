//! Shots, hits and destruction, as `HELLBEND.EXE` does them.
//!
//! Read from the engine:
//!
//! - A straight shot lives in a 256-slot pool at `0x614570`, 104 bytes a slot,
//!   the player's in the first 128 and everyone else's in the rest. The spawner
//!   (`0x476780`) takes damage, speed, side and weapon kind, and gives every
//!   shot two seconds (`0x20000` at `0x47684e`).
//! - The update (`0x476ac0`) moves each shot in eight equal sub-steps a frame
//!   and tests after each one, so a hit is a point test, not a swept one.
//! - A shot that goes below the solid ground under it, or above a ceiling,
//!   stops (`0x41c300` and `0x41c4d0` at `0x476d14`).
//! - The player's laser is weapon 1 of the table at `0x50e7a0`: 32 units a
//!   second on top of the ship's own speed (`0x47d57a` adds the ship's
//!   velocity magnitude) and 1/16 damage.
//! - An actor's hit points are its placement's second field, and a hit takes
//!   the shot's damage times the type's multiplier for that weapon kind
//!   (`0x40d2b0`).
//! - A shot hits an actor if it is inside one of the type's hit spheres, or,
//!   when the type lists none, inside the model's bounding box turned to the
//!   actor's heading (`0x40ce70`).
//! - Enemy shots hit the player inside a box 2 units each way of the ship
//!   (`0x465650`); damage is scaled by the shield (`0x4653a0`).
//!
//! This port's own choices are named where they are made.

use hb_formats::mrgl::Model;
use hb_formats::text::{EnemyDef, Placement};

/// Seconds a straight shot lives: `0x20000` in 16.16, set by the spawner.
pub const SHOT_LIFE: f32 = 2.0;
/// Sub-steps a frame for a straight shot (`0x476e4e`).
pub const SUBSTEPS: usize = 8;

/// One row of the weapon table at `0x50e7a0` (68 bytes a row): speed in units
/// per second and damage as a fraction of full health.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeaponStats {
    pub speed: f32,
    pub damage: f32,
}

/// The first 26 rows of the table, speed and damage columns, as read from
/// `0x50e7a0 + 68 * kind`. Rows 9-17 and 20-22 have no speed: a slot whose
/// speed is zero is a free slot to the update, so those weapons cannot be
/// straight shots.
pub const WEAPONS: [(i32, i32); 26] = [
    (2_097_152, 4096),
    (2_097_152, 4096),
    (1_572_864, 8192),
    (4_194_304, 4096),
    (2_097_152, 16384),
    (2_097_152, 16384),
    (2_097_152, 16384),
    (2_097_152, 16384),
    (2_097_152, 16384),
    (0, 16384),
    (0, 16384),
    (0, 16384),
    (0, 16384),
    (0, 16384),
    (0, 16384),
    (0, 16384),
    (0, 16384),
    (0, 16384),
    (4_194_304, 65536),
    (4_194_304, 65536),
    (0, 0),
    (0, 0),
    (0, 8192),
    (8_388_608, 8192),
    (4_194_304, 65536),
    (4_194_304, 65536),
];

pub fn weapon(kind: usize) -> Option<WeaponStats> {
    let (speed, damage) = *WEAPONS.get(kind)?;
    Some(WeaponStats { speed: speed as f32 / 65536.0, damage: damage as f32 / 65536.0 })
}

/// The weapon kind the player's laser fires. Rows 0 and 1 have the same speed
/// and damage; 1 is taken because kinds 1-3 are the ones a type's laser
/// multiplier applies to. Which row the game starts the player on has not
/// been read.
pub const PLAYER_LASER: usize = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Player,
    Enemy,
}

#[derive(Debug, Clone, Copy)]
pub struct Shot {
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    pub age: f32,
    pub damage: f32,
    /// The weapon kind, which picks the target's damage multiplier.
    pub kind: i32,
    pub side: Side,
}

/// A direction in the engine's convention: heading 0 along +z increasing
/// toward +x, positive pitch nose down.
pub fn direction(heading: f32, pitch: f32) -> [f32; 3] {
    let h = heading * std::f32::consts::TAU / 65536.0;
    let p = pitch * std::f32::consts::TAU / 65536.0;
    [h.sin() * p.cos(), -p.sin(), h.cos() * p.cos()]
}

impl Shot {
    pub fn fire(
        from: [f32; 3],
        dir: [f32; 3],
        speed: f32,
        damage: f32,
        kind: i32,
        side: Side,
    ) -> Shot {
        Shot {
            position: from,
            velocity: [dir[0] * speed, dir[1] * speed, dir[2] * speed],
            age: 0.0,
            damage,
            kind,
            side,
        }
    }

    /// The player's laser, from the eye along the view, faster by however
    /// fast the ship is going.
    pub fn player_laser(from: [f32; 3], heading: u16, pitch: u16, ship_speed: f32) -> Shot {
        let stats = weapon(PLAYER_LASER).expect("row 1 is in the table");
        let dir = direction(heading as f32, (pitch as i16) as f32);
        Shot::fire(
            from,
            dir,
            stats.speed + ship_speed.abs(),
            stats.damage,
            PLAYER_LASER as i32,
            Side::Player,
        )
    }

    pub fn alive(&self) -> bool {
        self.age < SHOT_LIFE
    }
}

/// Which of a type's three `= cannonDamage, laserDamage, missileDamage`
/// multipliers a weapon kind uses, from the jump table at `0x40d558`: kinds
/// 1-3 the laser's, 18, 19 and 24-28 the missile's, 23 the cannon's, and
/// everything else none.
pub fn multiplier(def: &EnemyDef, kind: i32) -> f32 {
    let slot = match kind {
        1..=3 => 1,
        18 | 19 | 24..=28 => 2,
        23 => 0,
        _ => return 1.0,
    };
    def.damage[slot] as f32 / 65536.0
}

/// A placed object's standing in a fight.
#[derive(Debug, Clone, Copy)]
pub struct Health {
    pub hit_points: f32,
    pub destroyed: bool,
}

impl Health {
    /// Hit points from the placement, at the game's default difficulty.
    ///
    /// `0x512628` starts at 1. Difficulty 2 multiplies every actor's hit points
    /// by 1.5 and 3 doubles them (`0x405b0b`); 0 halves the damage the player
    /// takes instead (`0x4653c7`).
    pub fn for_placement(p: &Placement) -> Health {
        Health { hit_points: p.hit_points as f32 / 65536.0, destroyed: false }
    }

    /// Apply a hit. Returns true on the hit that destroys it.
    pub fn take(&mut self, damage: f32) -> bool {
        if self.destroyed {
            return false;
        }
        self.hit_points -= damage;
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

/// Where a type can be hit, in its own frame, in world units.
#[derive(Debug, Clone)]
pub enum HitVolume {
    /// The model's bounding box at the type's radius. The engine fills the
    /// type's box at `0x404d40` from the model.
    Box { min: [f32; 3], max: [f32; 3] },
    /// The `;NewHit` spheres: a vertex's position and a half-size. The engine
    /// tests each axis against the half-size, so each is really a cube.
    Spheres(Vec<([f32; 3], f32)>),
}

impl HitVolume {
    /// The volume for a type. With no mesh - the animated `.TXT` models are
    /// not rigid - a cube of the type's radius stands in, which is this port's
    /// choice.
    pub fn for_type(def: &EnemyDef, mesh: Option<&Model>) -> HitVolume {
        let radius = def.radius();
        let local = |v: &hb_formats::mrgl::Vertex| {
            let w = v.world(radius);
            [w[0] as f32 / 65536.0, w[1] as f32 / 65536.0, w[2] as f32 / 65536.0]
        };
        if let Some(mesh) = mesh {
            if !def.hit_spheres.is_empty() {
                let spheres = def
                    .hit_spheres
                    .iter()
                    .filter_map(|&(vertex, half)| {
                        Some((local(mesh.vertices.get(vertex as usize)?), half as f32 / 65536.0))
                    })
                    .collect::<Vec<_>>();
                if !spheres.is_empty() {
                    return HitVolume::Spheres(spheres);
                }
            }
            if !mesh.vertices.is_empty() {
                let mut min = [f32::MAX; 3];
                let mut max = [f32::MIN; 3];
                for v in &mesh.vertices {
                    let p = local(v);
                    for k in 0..3 {
                        min[k] = min[k].min(p[k]);
                        max[k] = max[k].max(p[k]);
                    }
                }
                return HitVolume::Box { min, max };
            }
        }
        let r = radius as f32 / 65536.0;
        HitVolume::Box { min: [-r; 3], max: [r; 3] }
    }

    /// True if a point in the object's own frame is inside.
    pub fn contains(&self, local: [f32; 3]) -> bool {
        match self {
            HitVolume::Box { min, max } => (0..3).all(|k| local[k] >= min[k] && local[k] <= max[k]),
            HitVolume::Spheres(spheres) => spheres
                .iter()
                .any(|(c, half)| (0..3).all(|k| (local[k] - c[k]).abs() < *half)),
        }
    }

    /// A radius that encloses the volume, for a cheap first rejection.
    pub fn reach(&self) -> f32 {
        let corner = |p: &[f32; 3]| p.iter().map(|v| v * v).sum::<f32>().sqrt();
        match self {
            HitVolume::Box { min, max } => corner(min).max(corner(max)),
            HitVolume::Spheres(spheres) => spheres
                .iter()
                .map(|(c, half)| corner(c) + half * 3f32.sqrt())
                .fold(0.0, f32::max),
        }
    }
}

/// A world point turned into an object's frame: the heading undone. Placed
/// objects have no pitch or roll in any shipped level.
pub fn to_local(point: [f32; 3], origin: [f32; 3], heading: u16) -> [f32; 3] {
    let (s, c) = (heading as f32 * std::f32::consts::TAU / 65536.0).sin_cos();
    let d = [point[0] - origin[0], point[1] - origin[1], point[2] - origin[2]];
    // The inverse of the renderer's `x' = x cos + z sin, z' = -x sin + z cos`.
    [d[0] * c - d[2] * s, d[1], d[0] * s + d[2] * c]
}

/// An object's frame turned into the world: the heading applied.
pub fn to_world(local: [f32; 3], origin: [f32; 3], heading: f32) -> [f32; 3] {
    let (s, c) = (heading * std::f32::consts::TAU / 65536.0).sin_cos();
    [
        origin[0] + local[0] * c + local[2] * s,
        origin[1] + local[1],
        origin[2] - local[0] * s + local[2] * c,
    ]
}

/// A 16.16 placement position in world units.
pub fn position_of(p: &Placement) -> [f32; 3] {
    [p.x as f32 / 65536.0, p.y as f32 / 65536.0, p.z as f32 / 65536.0]
}

/// The shortest signed difference between two positions in the wrapping
/// world, as the engine takes it with `shl 6; sar 6` on 16.16 values.
pub fn wrapped(d: f32) -> f32 {
    (d + 512.0).rem_euclid(1024.0) - 512.0
}

/// What stopped a shot this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stop {
    /// Inside the placement with this index.
    Object(usize),
    Player,
    Ground,
}

/// Move a shot through one frame in the engine's eight sub-steps, testing
/// after each.
///
/// A player's shot tests the objects; an enemy's tests the player. The engine
/// also lets enemy shots hit other actors (`0x40d750` runs for both sides),
/// which this port leaves out: a turret's muzzle can sit inside its own box.
pub fn step_shot(
    shot: &mut Shot,
    dt: f32,
    placements: &[Placement],
    volumes: &[HitVolume],
    alive: impl Fn(usize) -> bool,
    player: [f32; 3],
    solid: impl Fn([f32; 3]) -> bool,
) -> Option<Stop> {
    let step = dt / SUBSTEPS as f32;
    for _ in 0..SUBSTEPS {
        for k in 0..3 {
            shot.position[k] += shot.velocity[k] * step;
        }
        let at = shot.position;
        match shot.side {
            Side::Player => {
                for (i, p) in placements.iter().enumerate() {
                    if !alive(i) {
                        continue;
                    }
                    let Some(volume) = volumes.get(p.kind) else { continue };
                    let origin = position_of(p);
                    let d = [wrapped(at[0] - origin[0]), at[1] - origin[1], wrapped(at[2] - origin[2])];
                    let reach = volume.reach();
                    if d.iter().map(|v| v * v).sum::<f32>() > reach * reach {
                        continue;
                    }
                    let near = [origin[0] + d[0], at[1], origin[2] + d[2]];
                    if volume.contains(to_local(near, origin, p.heading)) {
                        return Some(Stop::Object(i));
                    }
                }
            }
            Side::Enemy => {
                if player_is_hit(player, at) {
                    return Some(Stop::Player);
                }
            }
        }
        if solid(at) {
            return Some(Stop::Ground);
        }
    }
    shot.age += dt;
    None
}

/// The player's hit box: 2 units each way of the ship (`0x46568a`).
pub fn player_is_hit(player: [f32; 3], point: [f32; 3]) -> bool {
    wrapped(player[0] - point[0]).abs() < 2.0
        && (player[1] - point[1]).abs() < 2.0
        && wrapped(player[2] - point[2]).abs() < 2.0
}

/// The player's ship in a fight: health, shield, and a hit's jolt.
#[derive(Debug, Clone, Copy)]
pub struct Pilot {
    /// Full is `0xffff` (`0x436260` and six other sites write it), read here
    /// as 1.0.
    pub health: f32,
    /// Starts at 0.5 (`0x426f47`). Each hit halves the damage by up to half
    /// of this and wears it down by 1/32.
    pub shield: f32,
    /// Seconds since the last hit; the engine sets a 1.0 timer at `0x46559e`
    /// and shakes the view by random amounts while it runs.
    pub since_hit: f32,
}

impl Default for Pilot {
    fn default() -> Pilot {
        Pilot { health: 1.0, shield: 0.5, since_hit: f32::MAX }
    }
}

impl Pilot {
    /// Take a hit, as `0x4653a0` does at the default difficulty. Returns true
    /// on the hit that kills.
    pub fn take(&mut self, damage: f32) -> bool {
        if self.health <= 0.0 {
            return false;
        }
        let taken = damage * (1.0 - self.shield / 2.0);
        self.shield = (self.shield - 1.0 / 32.0).clamp(0.0, 1.0);
        self.health -= taken;
        self.since_hit = 0.0;
        if self.health <= 0.0 {
            self.health = 0.0;
            return true;
        }
        false
    }

    pub fn alive(&self) -> bool {
        self.health > 0.0
    }
}

/// The sound an enemy shot makes passing within 16 units, as `0x4768a0`
/// picks it: `missile.wav` for kind 17, otherwise the weapon row's own sound
/// (the string at `0x50e7b0 + 68 * kind`), otherwise `whiz3.wav`. Rows past 25
/// have not been read and fall back to `whiz3.wav`.
pub fn near_miss_sound(kind: i32) -> &'static str {
    match kind {
        17 => "missile.wav",
        1 => "laser4.wav",
        2 => "laser3.wav",
        3 => "laser5.wav",
        18 => "missile.wav",
        19 => "missl-2.wav",
        23 => "m-gun-r.wav",
        24 => "missl-1.wav",
        25 => "missl-3.wav",
        _ => "whiz3.wav",
    }
}

/// Whether an actor is close enough to the eye to think, and to be drawn: 80
/// units or less on both x and z (`0x42f7b9`, `0x500000`). The actor loop at
/// `0x406650` updates nothing outside it.
pub fn in_range(eye: [f32; 3], at: [f32; 3]) -> bool {
    wrapped(at[0] - eye[0]).abs() <= 80.0 && wrapped(at[2] - eye[2]).abs() <= 80.0
}
