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
//! The flying itself is [`crate::steer`], `0x4944c0`, which every one of these
//! calls once a frame with a target, a mode, a speed, the type's turn rate and
//! a thrust factor: 1, or 2 for a fighter chasing at twice the player's speed
//! (`0x496c30`). A fighter in phase 200 with the player ahead of it but off its
//! nose faces the player's lead point (mode 3, `0x496ce4`).

use crate::mine::laid::Field as Laid;
use crate::mine::laid::AHEAD;
use hb_formats::fixed::{circle, radians, signed, RADIANS_PER_UNIT, UNITS_PER_RADIAN};
use hb_formats::mrgl::Model;
use hb_formats::text::{EnemyDef, Placement};
use hb_formats::vector::{self, add, along, flat_length, length, offset, scale};

use crate::combat::position_of;
use crate::steer::{self, Body, Mode, Order, Surfaces};
use crate::turret::{Launch, Rng, Turret};

const CONE: f32 = 0.523_598_8;

/// What the flyer knows about the player each frame.
#[derive(Debug, Clone, Copy)]
pub struct Target {
    pub position: [f32; 3],
    /// The player's axes.
    pub right: [f32; 3],
    pub up: [f32; 3],
    pub forward: [f32; 3],
    /// Forward speed, units a second (`0x50cc50`).
    pub speed: f32,
    /// Velocity in the world, units a second (`0x5b3a00`), for the lead
    /// point.
    pub velocity: [f32; 3],
    /// Whether the player is still flying. A mine layer will not lay one
    /// over a wreck (`0x49661e`).
    pub alive: bool,
}

#[derive(Debug, Clone)]
pub struct Flyer {
    /// Its pose and motion, as the steering keeps them.
    pub body: Body,
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

/// The engine's cone answer for a point seen in a frame: 1 in the 30-degree
/// cone ahead, 2 ahead but outside it, -2 behind.
fn cone(local: [f32; 3]) -> i32 {
    if local[2] <= 0.0 {
        return -2;
    }
    let across = local[0].atan2(local[2]).abs();
    let up = local[1].atan2(flat_length(local)).abs();
    if across < CONE && up < CONE {
        1
    } else {
        2
    }
}

impl Flyer {
    pub fn new(p: &Placement) -> Flyer {
        Flyer {
            body: Body::new(position_of(p), p.heading as f32),
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

    /// One frame of `0x4967b0`.
    #[allow(clippy::too_many_arguments)]
    pub fn step(
        &mut self,
        def: &EnemyDef,
        mesh: Option<&Model>,
        player: &Target,
        dt: f32,
        world: &impl Surfaces,
        rng: &mut Rng,
    ) -> Option<Launch> {
        if self.phase == 0 {
            self.phase = 200;
            return None;
        }
        let at = self.body.position;
        let d = offset(at, player.position);
        let distance = length(d);
        let attack = def_range(def, 0);
        let retreat = def_range(def, 1);
        let mut speed = def.move_rate as f32 / 65536.0;
        let mut turn = def.turn_rate as f32 * RADIANS_PER_UNIT;
        let mut thrust = 1.0;
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
        let me_in_his_sights = cone(along(scale(d, -1.0), &[pr, pu, pf]));
        let him_in_mine = cone(along(d, &self.body.axes()));
        let player_heading = circle(pf[0].atan2(pf[2]));
        let player_pitch = circle((-pf[1]).clamp(-1.0, 1.0).asin());
        let aligned = radians(signed(player_heading - self.body.heading)).abs() < CONE
            && radians(signed(player_pitch - self.body.pitch)).abs() < CONE;
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
                if twice_player > speed {
                    speed = twice_player;
                    // The fighter's own: it pushes twice as hard to get there.
                    if !self.lays {
                        thrust = 2.0;
                    }
                }
                // Level within 30 degrees (`0x4ef598`, 5,461 in the circle).
                let roll = signed(self.body.roll);
                self.phase = if roll.abs() < 5461.333 {
                    2011
                } else if roll > 0.0 {
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
        let mut mode = Mode::Toward;
        match self.phase {
            200 => {
                if situation == 6 && !self.lays {
                    mode = Mode::Facing;
                }
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
                mode = Mode::Away;
                if distance > attack {
                    self.phase = 200;
                }
            }
            900 => {
                speed = 0.0;
                mode = Mode::Facing;
            }
            2000 => {
                speed = speed.max(twice_player);
                mode = Mode::Away;
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
                target = add(player.position, scale(player.forward, AHEAD));
                let to_target = vector::distance(at, target);
                if self.closest > to_target {
                    self.closest = to_target;
                } else {
                    // Stopped closing: drop it here and break away.
                    self.closest = 2048.0;
                    self.phase = 201;
                    if player.alive && Laid::clear_of(at, player.position) {
                        let breaking = self.phase == 2000;
                        fly(&mut self.body, breaking, def, player, target, mode, speed, turn, thrust, dt, world);
                        return Some(Launch::Mine {
                            at: self.body.position,
                            radius: retreat,
                            damage: def.shot_damage as f32 / 65536.0,
                        });
                    }
                }
            }
            2012 => {
                target = self.break_point;
                let to_target = vector::distance(at, target);
                if self.closest > to_target {
                    self.closest = to_target;
                } else {
                    self.phase = 201;
                }
            }
            _ => {}
        }

        let breaking = self.phase == 2000;
        fly(&mut self.body, breaking, def, player, target, mode, speed, turn, thrust, dt, world);

        // Fire within the attack range when aimed (`0x496f68`).
        if attack > distance && him_in_mine == 1 {
            let at = self.body.position;
            return self.gun.trigger(def, mesh, at, player.position, dt, 2.0 * speed, rng);
        }
        None
    }
}

/// One frame of the steering toward `target`: `turn` in radians a second,
/// `speed` in units a second. `breaking` is whether the actor is in phase
/// 2000. The lead point for modes 3 and 4 is worked out here from the
/// type's shot speed (`0x4922a0`).
#[allow(clippy::too_many_arguments)]
pub fn fly(
    body: &mut Body,
    breaking: bool,
    def: &EnemyDef,
    player: &Target,
    target: [f32; 3],
    mode: Mode,
    speed: f32,
    turn: f32,
    thrust: f32,
    dt: f32,
    world: &impl Surfaces,
    ) {
    let aim = matches!(mode, Mode::Facing | Mode::BackingFacing).then(|| {
        let shot = def.shot_speed() as f32 / 65536.0;
        steer::lead(body.position, player.position, player.velocity, shot, dt)
    });
    let order = Order {
        target,
        mode,
        speed,
        turn: turn * UNITS_PER_RADIAN,
        thrust,
        class: def.class() as i32,
        clearance: def.radius() as f32 / 65536.0,
        breaking,
        aim,
    };
    body.steer(&order, world, dt);
}

/// `!NewAtakRet`'s first two values - attack and retreat range, whole units
/// (type offsets 0x1f8 and 0x1fc, compared shifted up by 16).
fn def_range(def: &EnemyDef, which: usize) -> f32 {
    def.attack_retreat[which] as f32
}

/// The hover craft, class 58 (`0x4976d0`) - the SPINE 17 in the three MORBOS
/// levels, three types.
///
/// It is not a fighter. It has a post - the position it was placed at, kept
/// at the actor's `+0xb4`, `+0xb8` and `+0xbc` - and it will not chase the
/// player further from that post than the type's attack range. The
/// difference shows in every phase:
///
/// - **2006** is where it starts and where it returns. Speed zero, steering
///   mode 3, target the player: it sits on its post and watches. Inside the
///   attack range it goes to 200.
/// - **200** chases (mode 0). Closer than the type's attack range it could
///   not turn in time, so it goes to 2000 - the same
///   `sqrt((R + r + 2)^2 - r^2)` test the fighters do. Inside the retreat
///   range it goes to 2002.
/// - **2000** flies away (mode 1) until it is more than eight units off, then
///   201.
/// - **2002** stays on the player, and inside the retreat range switches to
///   steering mode 2 - backing off - at no less than the player's own speed
///   (`0x4979fa`).
/// - **201** flies home (mode 0). Within eight units of the post it goes back
///   to 2006.
///
/// Dragged off its post - further from home than the attack range - it stops
/// choosing any of that: whatever the phase, if the player is behind it
/// (`0x492750` answers -2) it turns for home.
///
/// It fires after it has moved, whenever the player is in its sights, the
/// same test and the same gun as the fighters.
#[derive(Debug, Clone)]
pub struct Hover {
    /// Its pose and motion, and its gun, the fighters'.
    pub body: Body,
    pub gun: Turret,
    /// Where it was placed, which it will not stray far from.
    pub post: [f32; 3],
    pub phase: u32,
    /// The phase last frame, so the engine's "just arrived" test can be made
    /// (`0x497844`): a phase does nothing on the frame it is entered.
    was: u32,
}

/// Within this of its post, a hover craft is home (`0x4ef5a8`, 8.0).
pub const HOME: f32 = 8.0;

/// And this far from the player, phase 2000 has flown far enough
/// (`0x49797d`).
pub const OFF: f32 = 8.0;

impl Hover {
    pub fn new(p: &Placement) -> Hover {
        let body = Body::new(position_of(p), p.heading as f32);
        Hover { post: body.position, body, gun: Turret::new(p), phase: 2006, was: 2006 }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn step(
        &mut self,
        def: &EnemyDef,
        mesh: Option<&Model>,
        player: &Target,
        dt: f32,
        world: &impl Surfaces,
        rng: &mut Rng,
    ) -> Option<Launch> {
        let d = offset(self.body.position, player.position);
        let distance = length(d);
        let attack = def_range(def, 0);
        let retreat = def_range(def, 1);
        let mut speed = def.move_rate as f32 / 65536.0;
        let turn = def.turn_rate as f32 * RADIANS_PER_UNIT;
        let radius = def.radius() as f32 / 65536.0;

        // How far it has been drawn off its post, flat (`0x49778c`).
        let home = flat_length(offset(self.body.position, self.post));
        // Beyond that, the engine hands the steering a speed of zero
        // (`0x4977da`) - it stops chasing where it stands. Only phase 201,
        // which is the way home, puts a speed back.
        let tethered = attack >= home;
        if !tethered {
            speed = 0.0;
        }

        // Entered this frame? A phase does nothing on its first frame.
        let arrived = self.phase != self.was;
        self.was = self.phase;

        // The turn-in-time test, only while on the tether and in 200.
        if self.phase == 200 && tethered && turn > 0.0 {
            let r = speed / turn;
            let reach = ((radius + r + 2.0).powi(2) - r * r).sqrt();
            if reach >= distance {
                self.phase = 2000;
            }
        }

        // Where it is in the player's frame, which is the only thing it asks
        // once it is off its tether.
        let behind = || {
            let [pr, pu, pf] = [player.right, player.up, player.forward];
            cone(along(scale(d, -1.0), &[pr, pu, pf])) < 0
        };

        let mut target = player.position;
        let mut mode = Mode::Toward;
        match self.phase {
            200 | 2002 if !tethered => {
                if behind() {
                    self.phase = 201;
                }
            }
            200 => {
                if !arrived && retreat > distance {
                    self.phase = 2002;
                }
            }
            2002 => {
                if !arrived && retreat > distance {
                    // Backing off, at least as fast as he comes.
                    mode = Mode::Backing;
                    speed = speed.max(player.speed);
                }
            }
            2000 => {
                mode = Mode::Away;
                if !arrived && distance > OFF {
                    self.phase = 201;
                }
            }
            201 => {
                target = self.post;
                // It goes home at the distance it has to cover (`0x49794e`
                // hands the steering the distance itself as the speed), so
                // it rushes back and eases in.
                speed = home;
                if home < HOME {
                    self.phase = 2006;
                }
            }
            // On station: still, facing him, until he is close enough.
            _ => {
                speed = 0.0;
                mode = Mode::Facing;
                if !arrived && distance < attack {
                    self.phase = 200;
                }
            }
        }

        let breaking = self.phase == 2000;
        fly(&mut self.body, breaking, def, player, target, mode, speed, turn, 1.0, dt, world);

        // And then it shoots, if he is in front of it (`0x497a7b`).
        if cone(along(d, &self.body.axes())) == 1 {
            return self.gun.trigger(
                def,
                mesh,
                self.body.position,
                player.position,
                dt,
                2.0 * speed.max(def.move_rate as f32 / 65536.0),
                rng,
            );
        }
        None
    }
}
