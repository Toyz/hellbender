//! Class 10: the turret, and the guided missile a SAM site fires.
//!
//! Read from `HELLBEND.EXE`:
//!
//! - The class-10 routine (`0x408c30`) aims at where the player will be: the
//!   time a shot takes to cross the distance, at the type's shot speed, times
//!   the player's velocity, added to the player's position - horizontally
//!   only, since the aim is a heading. It asks for pitch 0.
//! - `0x4068f0` eases the actor toward that: each angle closes by the error
//!   times `turn_rate * dt`. It is an exponential approach, not a fixed rate.
//! - Frame time accumulates into the actor; when it passes the type's fire
//!   interval the interval is subtracted and the actor fires - weapon 19
//!   through `0x4074e0` (a guided missile), anything else through `0x406dc0`.
//! - `0x406dc0` cycles barrels for the two multi-barrel modes, builds the
//!   shot's direction from the actor's angles plus the barrel's offset, and
//!   fires only if the player is in front of that direction. The shot leaves
//!   from a muzzle vertex picked at random from the type's list, or from the
//!   actor's origin when the list is empty; a muzzle vertex at the model's
//!   origin fires nothing.
//! - `0x4074e0` has no in-front test. The missile (`0x477890`) starts at the
//!   type's move rate plus one sixty-five-thousandth - effectively at rest -
//!   gains 16 units a second every second (`0x478673`) up to 64
//!   (`0x400000`), lives six seconds, and does 1/4 damage (`0x4000`).
//! - The missile steers (`0x478670`) in four sub-steps a frame. Its gain on
//!   the angle error rises from nothing to 4.0 over its first second, holds
//!   4.0 to one and a half seconds, and after that it points straight at the
//!   player every sub-step.
//!
//! A turret only thinks while it is within 80 units of the eye on both axes:
//! the actor loop at `0x406650` runs the draw-and-cull routine `0x40da00`
//! first and skips the update when it reports the actor out of range
//! (`0x42f7b9`). See [`crate::combat::in_range`].

use hb_formats::mrgl::Model;
use hb_formats::text::{EnemyDef, Placement};

use crate::combat::{direction, position_of, to_world, wrapped, Shot, Side};

/// The guided missile's weapon kind.
pub const GUIDED: i32 = 19;
/// `0x500760` and `0x500778`: pitch and heading offsets per barrel.
const BARREL_PITCH: [i32; 5] = [0, -2048, 0, 2048, 0];
const BARREL_HEADING: [i32; 5] = [0, 0, -2048, 0, 2048];

/// A small deterministic generator for the engine's `rand() % n` choices.
#[derive(Debug, Clone)]
pub struct Rng(u32);

impl Rng {
    pub fn new(seed: u32) -> Rng {
        Rng(seed.max(1))
    }

    /// 0 to 32,767, the range of the C runtime's `rand` (`0x4aba20`).
    pub fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0 & 0x7fff
    }

    pub fn below(&mut self, n: usize) -> usize {
        (self.next() as usize * n) >> 15
    }
}

/// The turn of an angle difference into -32,768..32,768, as the engine takes
/// it before easing a heading.
fn angle_error(to: f32, from: f32) -> f32 {
    (to - from + 32768.0).rem_euclid(65536.0) - 32768.0
}

#[derive(Debug, Clone)]
pub struct Turret {
    /// The engine's 16-bit circle, kept as a float so small eases accumulate.
    pub heading: f32,
    pub pitch: f32,
    /// Frame time since the last shot (actor offset 0x68).
    pub waited: f32,
    /// The barrel the next shot leaves from (actor offset 0xac).
    pub barrel: usize,
    /// Class 1 rather than class 10: it leads in all three axes and pitches
    /// as well as turns (`0x4077c0`).
    pub aims: bool,
    /// Where it aims from, when that is not the actor's own origin. Class 14
    /// asks its model for the part its gun is on and measures from there
    /// (`0x406bf0`); the caller sets this because only it has the model.
    pub aim_from: Option<[f32; 3]>,
}

/// What a turret put into the world this frame.
#[derive(Debug, Clone)]
pub enum Launch {
    Shot(Shot),
    Missile(Missile),
}

impl Turret {
    pub fn new(p: &Placement) -> Turret {
        Turret {
            heading: p.heading as f32,
            pitch: p.pitch as f32,
            waited: 0.0,
            barrel: 0,
            aims: false,
            aim_from: None,
        }
    }

    /// The class 1 gun: the same state, aiming in three dimensions.
    pub fn aiming(p: &Placement) -> Turret {
        Turret { aims: true, ..Turret::new(p) }
    }

    /// One frame for the actor standing at `p`: `0x408c30` for a class 10
    /// turret, and `0x4077c0` for a class 1 gun, which differ only in the
    /// aim.
    #[allow(clippy::too_many_arguments)]
    pub fn step(
        &mut self,
        def: &EnemyDef,
        mesh: Option<&Model>,
        p: &Placement,
        player: [f32; 3],
        player_velocity: [f32; 3],
        dt: f32,
        rng: &mut Rng,
    ) -> Option<Launch> {
        let at = position_of(p);
        // The aim is measured from the gun where the type names one.
        let eye = self.aim_from.unwrap_or(at);
        let d = [wrapped(player[0] - eye[0]), player[1] - eye[1], wrapped(player[2] - eye[2])];
        let distance = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        let speed = def.shot_speed() as f32 / 65536.0;
        // Two turret types have no shot speed; the engine divides by it anyway.
        let flight = if speed > 0.0 { distance / speed } else { 0.0 };
        let lead_x = d[0] + player_velocity[0] * flight;
        let lead_z = d[2] + player_velocity[2] * flight;
        let mut wanted = lead_x.atan2(lead_z) * 65536.0 / std::f32::consts::TAU;

        // Class 10 asks for pitch 0. Class 1 leads in y as well and aims up
        // and down, wrapping the pair past vertical the way `0x4078c8` does:
        // past a quarter turn the pitch reflects and the heading turns
        // about, and below minus a quarter the engine adds half a turn to
        // both, which is not the mirror of the first and is left as it is.
        let mut wanted_pitch = 0.0;
        if self.aims {
            let lead_y = d[1] + player_velocity[1] * flight;
            let flat = (lead_x * lead_x + lead_z * lead_z).sqrt();
            wanted_pitch = -lead_y.atan2(flat) * 65536.0 / std::f32::consts::TAU;
            if wanted_pitch > 16384.0 {
                wanted_pitch = 32768.0 - wanted_pitch;
                wanted += 32768.0;
            } else if wanted_pitch < -16384.0 {
                wanted_pitch += 32768.0;
                wanted += 32768.0;
            }
        }

        let ease = def.turn_rate as f32 / 65536.0 * dt;
        self.heading = (self.heading + angle_error(wanted, self.heading) * ease).rem_euclid(65536.0);
        self.pitch += angle_error(wanted_pitch, self.pitch) * ease;

        self.trigger(def, mesh, at, player, dt, def.shot_speed() as f32 / 65536.0, rng)
    }

    /// `0x407770`: frame time accumulates, and past the fire interval the
    /// weapon fires - a guided missile for weapon 19 (`0x4074e0`), a straight
    /// shot otherwise (`0x406dc0`). Turrets fire at the type's shot speed;
    /// the flyers call it with twice their own speed.
    pub fn trigger(
        &mut self,
        def: &EnemyDef,
        mesh: Option<&Model>,
        at: [f32; 3],
        player: [f32; 3],
        dt: f32,
        shot_speed: f32,
        rng: &mut Rng,
    ) -> Option<Launch> {
        self.waited += dt;
        let interval = def.fire_interval as f32 / 65536.0;
        if self.waited <= interval {
            return None;
        }
        self.waited -= interval;
        if def.weapon == GUIDED {
            self.launch_missile(def, mesh, at, rng).map(Launch::Missile)
        } else {
            self.fire(def, mesh, at, player, shot_speed, rng).map(Launch::Shot)
        }
    }

    /// Where a shot leaves from: a muzzle vertex turned to the actor, or the
    /// origin when the type lists none. `None` when the muzzle picked is at the
    /// model's origin, which the engine treats as nowhere to fire from.
    fn muzzle(&self, def: &EnemyDef, mesh: Option<&Model>, at: [f32; 3], rng: &mut Rng) -> Option<[f32; 3]> {
        if def.muzzles.is_empty() {
            return Some(at);
        }
        let pick = def.muzzles[rng.below(def.muzzles.len())];
        let Some(mesh) = mesh else { return Some(at) };
        let v = mesh.vertices.get(pick as usize)?;
        let w = v.world(def.radius());
        if w == [0, 0, 0] {
            return None;
        }
        let local = [w[0] as f32 / 65536.0, w[1] as f32 / 65536.0, w[2] as f32 / 65536.0];
        Some(to_world(local, at, self.heading))
    }

    fn fire(
        &mut self,
        def: &EnemyDef,
        mesh: Option<&Model>,
        at: [f32; 3],
        player: [f32; 3],
        speed: f32,
        rng: &mut Rng,
    ) -> Option<Shot> {
        let barrel = self.barrel;
        self.barrel = match def.second_weapon.barrels {
            1 if barrel < 2 => barrel + 1,
            2 if barrel < 4 => barrel + 1,
            _ => 0,
        };
        if speed <= 0.0 {
            // A zero-speed slot is a free slot to the shot update.
            return None;
        }
        let dir = direction(
            self.heading + BARREL_HEADING[barrel] as f32,
            self.pitch + BARREL_PITCH[barrel] as f32,
        );
        // Only when the player is ahead of the barrel (`0x406fc6`).
        let behind = wrapped(at[0] - player[0]) * dir[0]
            + (at[1] - player[1]) * dir[1]
            + wrapped(at[2] - player[2]) * dir[2];
        if behind > 0.0 {
            return None;
        }
        let from = self.muzzle(def, mesh, at, rng)?;
        Some(Shot::fire(
            from,
            dir,
            speed,
            def.shot_damage as f32 / 65536.0,
            def.weapon,
            Side::Enemy,
        ))
    }

    fn launch_missile(
        &mut self,
        def: &EnemyDef,
        mesh: Option<&Model>,
        at: [f32; 3],
        rng: &mut Rng,
    ) -> Option<Missile> {
        let from = self.muzzle(def, mesh, at, rng)?;
        Some(Missile::enemy(from, self.heading, self.pitch, (def.move_rate + 1) as f32 / 65536.0))
    }
}

/// A guided missile in flight, one of the 16 slots at `0x613700`: an
/// enemy's homes on the player, the player's on the placement it was
/// launched at, and one with nothing to home on flies straight
/// (`0x477f20`).
#[derive(Debug, Clone, Copy)]
pub struct Missile {
    pub position: [f32; 3],
    pub heading: f32,
    pub pitch: f32,
    /// Units per second.
    pub speed: f32,
    pub age: f32,
    /// What a hit takes (`+0`).
    pub damage: f32,
    /// The weapon kind (`+0x14`), which picks the target's multiplier.
    pub kind: i32,
    /// Whose it is (`+0xc`: 0 for the player).
    pub side: Side,
    /// For the player's, the placement it homes on (`+0x10`). Dropped when
    /// that placement is destroyed.
    pub target: Option<usize>,
    /// Seconds it flies (`+4`): six, ten for the cruise missile.
    pub life: f32,
}

/// The two missiles that break up in flight: the MIRV, whose ten go out
/// unguided (`0x477b90`), and the guided MIRV, whose ten are Vipers, each
/// sent at a target of its own (`0x477d10`).
pub const MIRV: i32 = 26;
pub const GUIDED_MIRV: i32 = 27;
/// They break up a second in, the same clock the steering's gain runs on
/// (`0x477b9d`).
pub const SPLIT_AT: f32 = 1.0;
/// Into ten (`0x477bc6`).
pub const SPLIT_INTO: usize = 10;
/// And leave a blast of 32 units and half a hit (`0x477c4a`).
pub const SPLIT_REACH: f32 = 32.0;
pub const SPLIT_DAMAGE: f32 = 0.5;

/// The Bion super weapon, the eight pieces' reward. It scorches as it goes:
/// every sub-step it damages everything within 16 units of where it is, a
/// quarter of its damage at a time (`0x477a19`).
pub const SUPER: i32 = 30;
pub const SUPER_REACH: f32 = 16.0;

/// What ended a missile's flight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Struck {
    Player,
    Object(usize),
    Ground,
}

impl Missile {
    /// Six seconds (`0x60000`, set at `0x4778f9`).
    pub const LIFE: f32 = 6.0;
    /// `0x400000`.
    pub const TOP_SPEED: f32 = 64.0;
    /// `0x100000` a second.
    pub const ACCELERATION: f32 = 16.0;
    /// `0x40000`, the steering gain once it has risen.
    pub const GAIN: f32 = 4.0;
    /// `0x4000`.
    pub const DAMAGE: f32 = 0.25;
    pub const SUBSTEPS: usize = 4;

    /// A SAM site's (`0x4074e0`).
    pub fn enemy(position: [f32; 3], heading: f32, pitch: f32, speed: f32) -> Missile {
        Missile {
            position,
            heading,
            pitch,
            speed,
            age: 0.0,
            damage: Missile::DAMAGE,
            kind: GUIDED,
            side: Side::Enemy,
            target: None,
            life: Missile::LIFE,
        }
    }

    pub fn alive(&self) -> bool {
        self.age <= self.life
    }

    /// Whether it is a MIRV that has flown its second and is about to break
    /// up.
    pub fn splitting(&self) -> bool {
        (self.kind == MIRV || self.kind == GUIDED_MIRV) && self.age > SPLIT_AT
    }

    /// The ten it becomes: from where it is, at half its speed, pitched and
    /// headed at random. A MIRV's are Dead-Ons with nothing to home on; a
    /// guided MIRV's are Vipers, one for each target offered.
    pub fn split(&self, rng: &mut Rng, targets: &[usize]) -> Vec<Missile> {
        let guided = self.kind == GUIDED_MIRV;
        (0..SPLIT_INTO)
            .map(|i| {
                let pitch = (rng.next() & 0x7fff) as f32 - 16384.0;
                let heading = (rng.next() & 0xffff) as f32;
                Missile {
                    position: self.position,
                    heading,
                    pitch,
                    speed: self.speed / 2.0,
                    age: 0.0,
                    damage: self.damage,
                    kind: if guided { 19 } else { 18 },
                    side: self.side,
                    target: guided.then(|| targets.get(i % targets.len().max(1)).copied()).flatten(),
                    life: Missile::LIFE,
                }
            })
            .collect()
    }

    /// One frame of an enemy's missile. `Some(true)` if it reached the
    /// player, `Some(false)` if it hit the ground, either of which ends it;
    /// `None` while it flies on.
    pub fn step(&mut self, dt: f32, player: Option<[f32; 3]>, solid: impl Fn([f32; 3]) -> bool) -> Option<bool> {
        let hit = |at: [f32; 3]| player.filter(|&p| crate::combat::player_is_hit(p, at)).map(|_| Struck::Player);
        match self.fly(dt, |_| player, hit, solid) {
            Some(Struck::Ground) => Some(false),
            Some(_) => Some(true),
            None => None,
        }
    }

    /// One frame of the player's missile. `aim` gives a placement's position
    /// while it is still a target; `hit` says which placement, if any, a
    /// point is inside.
    pub fn step_at(
        &mut self,
        dt: f32,
        aim: impl Fn(usize) -> Option<[f32; 3]>,
        hit: impl Fn([f32; 3]) -> Option<usize>,
        solid: impl Fn([f32; 3]) -> bool,
    ) -> Option<Struck> {
        let seek = |m: &mut Missile| {
            let at = m.target.and_then(&aim);
            if at.is_none() {
                m.target = None;
            }
            at
        };
        self.fly(dt, seek, |p| hit(p).map(Struck::Object), solid)
    }

    fn fly(
        &mut self,
        dt: f32,
        mut seek: impl FnMut(&mut Missile) -> Option<[f32; 3]>,
        hit: impl Fn([f32; 3]) -> Option<Struck>,
        solid: impl Fn([f32; 3]) -> bool,
    ) -> Option<Struck> {
        let step = dt / Missile::SUBSTEPS as f32;
        for _ in 0..Missile::SUBSTEPS {
            self.speed = (self.speed + Missile::ACCELERATION * step).min(Missile::TOP_SPEED);
            if let Some(target) = seek(self) {
                let d = [
                    wrapped(target[0] - self.position[0]),
                    target[1] - self.position[1],
                    wrapped(target[2] - self.position[2]),
                ];
                let turn = 65536.0 / std::f32::consts::TAU;
                let heading = d[0].atan2(d[2]) * turn;
                // Positive pitch is nose down, so climbing toward a target
                // above is a negative pitch (`0x4eeaf0` is the negative scale).
                let pitch = -(d[1].atan2((d[0] * d[0] + d[2] * d[2]).sqrt())) * turn;
                if self.age > 1.5 {
                    self.heading = heading.rem_euclid(65536.0);
                    self.pitch = pitch;
                } else {
                    let gain = Missile::GAIN * self.age.min(1.0);
                    self.pitch += (pitch - self.pitch) * gain * step;
                    self.heading =
                        (self.heading + angle_error(heading, self.heading) * gain * step).rem_euclid(65536.0);
                }
            }
            let dir = direction(self.heading, self.pitch);
            for k in 0..3 {
                self.position[k] += dir[k] * self.speed * step;
            }
            if let Some(struck) = hit(self.position) {
                return Some(struck);
            }
            if solid(self.position) {
                return Some(Struck::Ground);
            }
        }
        self.age += dt;
        None
    }
}
