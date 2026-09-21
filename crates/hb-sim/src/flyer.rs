//! The flying enemies: classes 7 and 53, `0x4967b0` - 1,108 placements - and
//! class 55, `0x496060`, the mine layers.
//!
//! Read from the engine - the decisions:
//!
//! - Actor `+0x64` is a phase. 0 initialises and goes to 200.
//! - Two cone tests: `0x492750` puts the flyer in the player's frame and
//!   answers 1 if it is within 30 degrees (`0x4ef510`) of the player's nose
//!   both across and up, 2 if it is ahead but outside that, -2 if behind;
//!   `0x492940` asks the same with the roles swapped. A third test asks whether
//!   the two are flying the same way - headings and pitches within 30 degrees.
//! - From those, a situation (`0x496b74` onward): head-on (both in each
//!   other's sights) pulls away under 32 units; with the player on its tail and
//!   aiming, it breaks - to the side it is banked toward, 16 units beside the
//!   player, or 16 units above him when level - and flies to that point until
//!   it stops closing; with the player ahead but not in its sights it matches
//!   the player's speed.
//! - Phase 200 flies at the player. Within the range where it could turn in
//!   time - `sqrt((R + r + 2)^2 - r^2)`, R its radius and r its turning radius,
//!   speed over turn rate (`0x49685b`) - or within the type's retreat range
//!   (`!NewAtakRet`'s second value) it goes to 2000 or 201.
//! - Phases 2000 and 201 fly **away** from the player - the steering is told
//!   mode 1, which negates the direction to the target (`0x494853`) - at no
//!   less than twice the player's speed, 201 with an eighth of the turn rate,
//!   until beyond the attack range (`!NewAtakRet`'s first value), and then go
//!   back to 200. So a flyer makes passes: in, past, out, round, in.
//! - It fires through the turrets' own `0x407770` whenever the player is
//!   within the attack range and it is aimed, at twice its own speed
//!   (`0x496f86`).
//!
//! **Class 55, the mine layers** (`0x496060`) is the same routine with the
//! same two cone tests, the same situation ladder and the same phases, plus
//! one of its own. Three differences:
//!
//! - Phase 200 has no retreat test: where a fighter turns for home inside the
//!   type's retreat range, a layer keeps going round.
//! - Phase 201 breaks at half the turn rate (`0x49650c`), not an eighth.
//! - Situation 6 in phase 200 - the player ahead of it but off its nose -
//!   sends it to phase 2002 (`0x4964b5`), which flies to 24 units along the
//!   player's own nose, where he is about to be, and drops a mine when it
//!   stops closing on that point. Not over the player: `0x496666` refuses
//!   within eight units, measured flat, and `0x49661e` refuses over a wreck.
//!
//! What it drops goes into a pool of its own - see [`crate::mine::laid`].
//!
//! This port's own - the flying: the engine's steering, `0x4944c0`, is 1,250
//! instructions of x87 rigid-body integration not yet read. What is known of
//! it is used: the target's direction as a heading and a pitch, the -1 for
//! mode 1, the turn rate at 2 pi / 65,536 radians a second a unit, and the
//! ground check that lifts the target when the predicted position would be
//! below the ground plus a clearance. Here a flyer turns its heading and
//! pitch toward the target at the turn rate and flies along its nose; that it
//! is aimed means the player is within its 30-degree cone.

use crate::mine::laid::Field as Laid;
use crate::mine::laid::AHEAD;
use hb_formats::mrgl::Model;
use hb_formats::text::{EnemyDef, Placement};

use crate::combat::{position_of, wrapped};
use crate::turret::{Launch, Rng, Turret};

/// The engine's circle.
const TURN: f32 = 65536.0;
const CONE: f32 = 0.523_598_8;

/// What the flyer knows about the player each frame.
#[derive(Debug, Clone, Copy)]
pub struct Target {
    pub position: [f32; 3],
    /// The player's axes.
    pub right: [f32; 3],
    pub up: [f32; 3],
    pub forward: [f32; 3],
    /// Forward speed, units a second.
    pub speed: f32,
    /// Whether the player is still flying. A mine layer will not lay one
    /// over a wreck (`0x49661e`).
    pub alive: bool,
}

#[derive(Debug, Clone)]
pub struct Flyer {
    pub position: [f32; 3],
    /// The engine's circle, as floats so small turns accumulate.
    pub heading: f32,
    pub pitch: f32,
    pub roll: f32,
    pub phase: u32,
    /// Class 55 rather than 7 or 53: it lays mines, breaks at half the turn
    /// rate instead of an eighth, and does not turn for home on the retreat
    /// range.
    pub lays: bool,
    /// Where a break-off is headed (`+0xb4`), and the nearest it has come
    /// (`+0x154`).
    pub break_point: [f32; 3],
    pub closest: f32,
    /// The weapon's timer and barrel, shared with the turret code.
    pub gun: Turret,
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn radians(angle: f32) -> f32 {
    angle / TURN * std::f32::consts::TAU
}

fn circle(r: f32) -> f32 {
    (r / std::f32::consts::TAU * TURN).rem_euclid(TURN)
}

fn signed(a: f32) -> f32 {
    (a + 32768.0).rem_euclid(TURN) - 32768.0
}

/// The engine's cone answer for a point seen in a frame: 1 in the 30-degree
/// cone ahead, 2 ahead but outside it, -2 behind.
fn cone(local: [f32; 3]) -> i32 {
    if local[2] <= 0.0 {
        return -2;
    }
    let across = local[0].atan2(local[2]).abs();
    let up = local[1].atan2((local[0] * local[0] + local[2] * local[2]).sqrt()).abs();
    if across < CONE && up < CONE {
        1
    } else {
        2
    }
}

/// A heading, pitch and roll's axes: right, up, forward - as the camera and
/// the ship use them.
pub fn axes(heading: f32, pitch: f32, roll: f32) -> [[f32; 3]; 3] {
    let (sh, ch) = radians(heading).sin_cos();
    let (sp, cp) = radians(pitch).sin_cos();
    let (sr, cr) = radians(roll).sin_cos();
    let forward = [sh * cp, -sp, ch * cp];
    let up0 = [sh * sp, cp, ch * sp];
    let right0 = [ch, 0.0, -sh];
    let right = [right0[0] * cr + up0[0] * sr, right0[1] * cr + up0[1] * sr, right0[2] * cr + up0[2] * sr];
    let up = [up0[0] * cr - right0[0] * sr, up0[1] * cr - right0[1] * sr, up0[2] * cr - right0[2] * sr];
    [right, up, forward]
}

impl Flyer {
    pub fn new(p: &Placement) -> Flyer {
        Flyer {
            position: position_of(p),
            heading: p.heading as f32,
            pitch: 0.0,
            roll: 0.0,
            phase: 0,
            break_point: [0.0; 3],
            closest: 0.0,
            gun: Turret::new(p),
            lays: false,
        }
    }

    /// A mine layer, class 55 (`0x496060`). It is this routine with one more
    /// phase and a slower break; [`Flyer::step`] returns [`Launch::Mine`]
    /// when it drops one.
    pub fn layer(p: &Placement) -> Flyer {
        Flyer { lays: true, ..Flyer::new(p) }
    }

    fn forward(&self) -> [f32; 3] {
        axes(self.heading, self.pitch, 0.0)[2]
    }

    /// One frame of `0x4967b0`. `ground` answers the height of the solid
    /// surface at an x and z.
    #[allow(clippy::too_many_arguments)]
    pub fn step(
        &mut self,
        def: &EnemyDef,
        mesh: Option<&Model>,
        player: &Target,
        dt: f32,
        ground: impl Fn(f32, f32) -> f32,
        rng: &mut Rng,
    ) -> Option<Launch> {
        if self.phase == 0 {
            self.phase = 200;
            return None;
        }
        let d = [
            wrapped(player.position[0] - self.position[0]),
            player.position[1] - self.position[1],
            wrapped(player.position[2] - self.position[2]),
        ];
        let distance = dot(d, d).sqrt();
        let attack = def_range(def, 0);
        let retreat = def_range(def, 1);
        let mut speed = def.move_rate as f32 / 65536.0;
        let mut turn = def.turn_rate as f32 / 65536.0 * std::f32::consts::TAU;
        let radius = def.radius() as f32 / 65536.0;

        // Phase 200 turns to an attack pass once close enough that it could
        // not turn in time otherwise (`0x49685b`).
        if self.phase == 200 && turn > 0.0 {
            let r = speed / turn;
            let reach = ((radius + r + 2.0).powi(2) - r * r).sqrt();
            if reach >= distance {
                self.phase = 2000;
            }
        }

        // The situation.
        let [pr, pu, pf] = [player.right, player.up, player.forward];
        let me_in_his_sights = cone([dot(d, pr) * -1.0, dot(d, pu) * -1.0, dot(d, pf) * -1.0]);
        let [mr, mu, mf] = axes(self.heading, self.pitch, self.roll);
        let him_in_mine = cone([dot(d, mr), dot(d, mu), dot(d, mf)]);
        let player_heading = circle(pf[0].atan2(pf[2]));
        let player_pitch = circle((-pf[1]).clamp(-1.0, 1.0).asin());
        let aligned = radians(signed(player_heading - self.heading)).abs() < CONE
            && radians(signed(player_pitch - self.pitch)).abs() < CONE;
        let situation = if me_in_his_sights == 1 && him_in_mine == 1 {
            1
        } else if aligned && him_in_mine == 1 {
            2
        } else if aligned && me_in_his_sights == 1 {
            4
        } else if him_in_mine == -2 {
            5
        } else if him_in_mine == 2 {
            6
        } else if !aligned && me_in_his_sights != 1 && him_in_mine != 1 {
            3
        } else {
            0
        };
        let twice_player = 2.0 * player.speed;
        match situation {
            1 if distance < 32.0 => self.phase = 201,
            4 if self.phase != 2012 => {
                speed = speed.max(twice_player);
                // Level within 30 degrees (`0x4ef598`, 5,461 in the circle).
                self.phase = if signed(self.roll).abs() < 5461.333 {
                    2011
                } else if signed(self.roll) > 0.0 {
                    2008
                } else {
                    2009
                };
            }
            // The layer peels off to lay one the moment the player is ahead
            // of it but off its nose (`0x4964b5`). Everything else matches
            // his speed and keeps station.
            6 if self.lays && self.phase == 200 => self.phase = 2002,
            6 if attack > distance => speed = player.speed,
            _ => {}
        }

        let mut target = player.position;
        let mut away = false;
        match self.phase {
            200 => {
                // The layer has no such test: it goes round again rather
                // than turning for home (`0x496482` tests nothing else).
                if !self.lays && retreat > distance {
                    self.phase = 201;
                }
            }
            201 => {
                speed = speed.max(twice_player);
                // The layer breaks at half the turn rate (`0x49650c`), the
                // fighters at an eighth.
                turn /= if self.lays { 2.0 } else { 8.0 };
                away = true;
                if distance > attack {
                    self.phase = 200;
                }
            }
            900 => speed = 0.0,
            2000 => {
                speed = speed.max(twice_player);
                away = true;
                if distance > attack {
                    self.phase = 200;
                }
            }
            2008 | 2009 | 2011 => {
                let side = if self.phase == 2008 { -16.0 } else { 16.0 };
                let (sh, ch) = radians(player_heading).sin_cos();
                self.break_point = if self.phase == 2011 {
                    [player.position[0], player.position[1] + 16.0, player.position[2]]
                } else {
                    [player.position[0] + ch * side, player.position[1], player.position[2] - sh * side]
                };
                self.closest = 2048.0;
                self.phase = 2012;
            }
            // The layer's own phase: fly to where the player is about to be
            // and leave a mine there (`0x49658f`).
            2002 => {
                target = std::array::from_fn(|k| player.position[k] + player.forward[k] * AHEAD);
                let t = [
                    wrapped(target[0] - self.position[0]),
                    target[1] - self.position[1],
                    wrapped(target[2] - self.position[2]),
                ];
                let to_target = dot(t, t).sqrt();
                if self.closest > to_target {
                    self.closest = to_target;
                } else {
                    // Stopped closing: drop it here and break away.
                    self.closest = 2048.0;
                    self.phase = 201;
                    if player.alive && Laid::clear_of(self.position, player.position) {
                        self.fly(def, target, false, speed, turn, dt, &ground);
                        return Some(Launch::Mine {
                            at: self.position,
                            radius: retreat,
                            damage: def.shot_damage as f32 / 65536.0,
                        });
                    }
                }
            }
            2012 => {
                target = self.break_point;
                let t = [
                    wrapped(target[0] - self.position[0]),
                    target[1] - self.position[1],
                    wrapped(target[2] - self.position[2]),
                ];
                let to_target = dot(t, t).sqrt();
                if self.closest > to_target {
                    self.closest = to_target;
                } else {
                    self.phase = 201;
                }
            }
            _ => {}
        }

        self.fly(def, target, away, speed, turn, dt, &ground);

        // Fire within the attack range when aimed (`0x496f68`).
        if attack > distance && him_in_mine == 1 {
            return self.gun.trigger(def, mesh, self.position, player.position, dt, 2.0 * speed, rng);
        }
        None
    }

    fn fly(
        &mut self,
        def: &EnemyDef,
        target: [f32; 3],
        away: bool,
        speed: f32,
        turn: f32,
        dt: f32,
        ground: &impl Fn(f32, f32) -> f32,
    ) {
        let clearance = def.radius() as f32 / 65536.0 + 2.0;
        let mut t = [
            wrapped(target[0] - self.position[0]),
            target[1] - self.position[1],
            wrapped(target[2] - self.position[2]),
        ];
        // The ground check: if the next position would be below the ground
        // plus a clearance, aim at that height instead.
        let f = self.forward();
        let ahead = [self.position[0] + f[0] * speed * dt, self.position[1] + f[1] * speed * dt, self.position[2] + f[2] * speed * dt];
        let floor = ground(ahead[0], ahead[2]) + clearance;
        if ahead[1] < floor {
            t[1] = floor - self.position[1];
        }
        if away {
            t = [-t[0], -t[1], -t[2]];
            if ahead[1] < floor {
                t[1] = floor - self.position[1];
            }
        }
        let horizontal = (t[0] * t[0] + t[2] * t[2]).sqrt();
        let want_heading = circle(t[0].atan2(t[2]));
        let want_pitch = circle(-(t[1].atan2(horizontal)));
        let step = circle(turn * dt).min(32768.0);
        let heading_error = signed(want_heading - self.heading);
        let pitch_error = signed(want_pitch - self.pitch);
        self.heading = (self.heading + heading_error.clamp(-step, step)).rem_euclid(TURN);
        let pitch = signed(self.pitch + pitch_error.clamp(-step, step)).clamp(-14000.0, 14000.0);
        self.pitch = pitch.rem_euclid(TURN);
        // Bank into the turn: the port's own, for the look of it.
        let bank = (-heading_error * 2.0).clamp(-10923.0, 10923.0);
        let roll = signed(self.roll);
        self.roll = (roll + (bank - roll) * (4.0 * dt).min(1.0)).rem_euclid(TURN);

        let f = self.forward();
        for k in 0..3 {
            self.position[k] += f[k] * speed * dt;
        }
        let floor = ground(self.position[0], self.position[2]) + clearance * 0.5;
        if self.position[1] < floor {
            self.position[1] = floor;
        }
        let wrap = |v: f32| (v + 512.0).rem_euclid(1024.0) - 512.0;
        self.position[0] = wrap(self.position[0]);
        self.position[2] = wrap(self.position[2]);
    }
}

/// `!NewAtakRet`'s first two values - attack and retreat range, whole units
/// (type offsets 0x1f8 and 0x1fc, compared shifted up by 16).
fn def_range(def: &EnemyDef, which: usize) -> f32 {
    def.attack_retreat[which] as f32
}
