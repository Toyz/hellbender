//! The steering, `0x4944c0`: how everything that flies moves - the fighters,
//! the mine layers, the hover craft, the transports. Each of those routines
//! decides where to go and how fast; this flies there.
//!
//! It is a rigid body. The actor keeps a velocity in its own frame (`+0x15c`,
//! `+0x160`, `+0x164`: right, up, forward) and three angular rates (`+0x168`
//! roll, `+0x16c` pitch, `+0x170` yaw), and each frame:
//!
//! - **Where it wants to point.** The target's direction as a heading,
//!   `atan2(dx, dz)`, and a pitch, `-atan2(dy, flat)` - away from it in mode
//!   1 (`0x494853`), at the player's lead point in modes 3 and 4 (`0x4922a0`).
//!   With the target more than a unit off its nose and moving forward at a
//!   unit a second or more (`0x494921`), it also banks: the roll that puts
//!   the target overhead, `-atan2(across, |up|)` in its own frame, and its
//!   pitch and yaw rates are held to the turn rate split by that bank.
//!   Otherwise the roll it wants is level.
//! - **Turning.** A spring and a damper on each angle: 4 times the pitch
//!   and heading errors and 5 times the roll error, less 2.5 times the rate
//!   (`0x494ee7`). A rate already at the turn rate is not pushed further
//!   (`0x494f3d`). Rates, then angles, step by the frame time.
//! - **Thrust.** Forward at 8 units a second squared times the caller's
//!   factor - 1 for a fighter, 2 when it is chasing at twice the player's
//!   speed, a half for the transports - and backward in modes 2 and 4. Once
//!   the forward speed reaches the speed asked for, the thrust becomes minus
//!   the forward speed, so it settles on it (`0x49525b`). Sideways and
//!   vertical speed decay by a tenth a second (`0x4952b7`).
//! - **Moving** along its velocity turned into the world by last frame's
//!   axes (`0x4953aa`), and then held between the floor and ceiling by class
//!   (`0x495528`).
//!
//! Before any of that, all but six classes look ahead: where the frame's
//! motion would put it, and if the target is below the floor there plus the
//! type's clearance (`+0x98`), they aim at that height instead (`0x4947b0`).
//! The engine takes the motion from a stack slot this routine only fills at
//! its end (`0x4953b2`), so the look-ahead uses whatever velocity the last
//! call left there - usually another actor's. The port uses the actor's own.
//! It then asks `0x4279b0` whether the segment to that point meets a surface
//! and moves the target to where it does; that is not ported.

use hb_formats::fixed::{from_units, over_the_top, signed, to_units, RADIANS_PER_UNIT as K, TURN};
use hb_formats::text::Placement;
use hb_formats::vector::{add, along, angles_of, axes, direction, from_frame, in_world, length, offset, scale};
use hb_world::Grid;


/// What the steering is asked to do, its last argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Toward the target.
    Toward = 0,
    /// Away from it (`0x494853` negates the direction).
    Away = 1,
    /// Toward it, thrusting backward (`0x495217`).
    Backing = 2,
    /// Facing the player's lead point (`0x494a4c`).
    Facing = 3,
    /// Both.
    BackingFacing = 4,
}

/// The six classes that do not look ahead for the ground (`0x494702`).
pub const NO_LOOKAHEAD: [i32; 6] = [60, 43, 44, 61, 59, 50];

/// Kept above the floor plus the clearance, and never below the clearance
/// itself: the flyers (`0x49553f`).
pub const ABOVE_GROUND: [i32; 10] = [7, 38, 39, 53, 54, 55, 56, 57, 58, 64];

/// Kept above the floor plus the clearance, underground too (`0x495564`).
pub const ANYWHERE: [i32; 6] = [43, 44, 45, 59, 60, 61];

/// The forward thrust at a factor of one, units a second squared (`0x49000000`
/// at `0x4944d2`, 8.0 in 16.16).
pub const THRUST: f32 = 8.0;

/// The surfaces the steering keeps between, in units.
pub trait Surfaces {
    /// `0x41c300`: what a point would come down on.
    fn floor(&self, at: [f32; 3]) -> f32;
    /// `0x41c4d0`: what is over it.
    fn ceiling(&self, at: [f32; 3]) -> f32;
}

impl Surfaces for Grid<'_> {
    fn floor(&self, at: [f32; 3]) -> f32 {
        let [x, y, z] = at.map(hb_formats::fixed::from_units);
        to_units(self.floor_under(x, y, z))
    }

    fn ceiling(&self, at: [f32; 3]) -> f32 {
        let [x, y, z] = at.map(hb_formats::fixed::from_units);
        to_units(self.ceiling_over(x, y, z))
    }
}

/// Flat ground at a height and nothing overhead, for tests and for callers
/// without a level.
#[derive(Debug, Clone, Copy)]
pub struct Flat(pub f32);

impl Surfaces for Flat {
    fn floor(&self, _: [f32; 3]) -> f32 {
        self.0
    }

    fn ceiling(&self, _: [f32; 3]) -> f32 {
        to_units(hb_world::grid::NO_CEILING)
    }
}

/// An actor's pose and motion as the steering keeps them.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Body {
    pub position: [f32; 3],
    /// The engine's circle, 0 to 65,536, as floats so small turns add up.
    pub heading: f32,
    pub pitch: f32,
    pub roll: f32,
    /// In its own frame - right, up, forward - units a second.
    pub velocity: [f32; 3],
    /// Roll, pitch and yaw rates, circle units a second.
    pub spin: [f32; 3],
    /// Pushes from outside, used up by the next frame: a linear one in its
    /// own frame, units a second squared (`+0x178`), and pitch, yaw and roll
    /// ones in tenths of a circle unit a second squared (`+0x184`).
    pub push: [f32; 3],
    pub twist: [f32; 3],
    /// How far the target was, last frame (`+0x174`).
    pub distance: f32,
    /// Its velocity in the world, last frame - see the module's note on the
    /// look-ahead.
    pub moving: [f32; 3],
}

/// One frame's request.
#[derive(Debug, Clone, Copy)]
pub struct Order {
    pub target: [f32; 3],
    pub mode: Mode,
    /// Units a second.
    pub speed: f32,
    /// Circle units a second: the type's `+0x68` plus the actor's bias.
    pub turn: f32,
    /// The thrust factor, the fifth argument.
    pub thrust: f32,
    /// The type's class and clearance (`+0x98`, units).
    pub class: i32,
    pub clearance: f32,
    /// Whether the actor is in phase 2000, which does not bank within eight
    /// units of its target (`0x494939`).
    pub breaking: bool,
    /// For modes 3 and 4, the heading and pitch of the player's lead point,
    /// circle units (`0x4922a0`, the actor's `+0x60` and `+0x58`).
    pub aim: Option<(f32, f32)>,
}

impl Body {
    pub fn new(position: [f32; 3], heading: f32) -> Body {
        Body { position, heading, ..Body::default() }
    }

    /// Where it is, into the placement the renderer and the fight read.
    pub fn write_to(&self, placed: &mut Placement) {
        [placed.x, placed.y, placed.z] = self.position.map(from_units);
        placed.heading = self.heading as u16;
        placed.pitch = self.pitch as i32;
        placed.roll = self.roll as i32;
    }

    /// Its axes - right, up, forward - as the actor's matrix holds them
    /// (`0x40b1c0`).
    pub fn axes(&self) -> [[f32; 3]; 3] {
        axes(self.heading, self.pitch, self.roll)
    }

    /// One frame of `0x4944c0`. Returns the target as the look-ahead left
    /// it: the engine writes the lifted height back into the caller's
    /// point.
    pub fn steer(&mut self, order: &Order, world: &impl Surfaces, dt: f32) -> [f32; 3] {
        let p = self.position;
        let mut target = order.target;
        let mut d = offset(p, target);
        let distance = length(d);

        if !NO_LOOKAHEAD.contains(&order.class) {
            let ahead = add(p, scale(self.moving, dt));
            let floor = world.floor(ahead) + order.clearance;
            if target[1] < floor {
                target[1] = floor;
                d[1] = floor - p[1];
            }
        }
        if order.mode == Mode::Away {
            d = d.map(|c| -c);
        }

        // Where it wants to point, and how fast it may turn to get there.
        let turn = order.turn * K;
        let axes = self.axes();
        let local = along(d, &axes);
        let across = (local[0] * local[0] + local[1] * local[1]).sqrt();
        let (cos, sin) = if across == 0.0 { (1.0, 0.0) } else { (local[1] / across, local[0] / across) };
        let banks =
            across >= 1.0 && self.velocity[2] >= 1.0 && !(order.breaking && distance < 8.0);
        let (mut yaw, mut climb) = angles_of(d);
        if let (Mode::Facing | Mode::BackingFacing, Some(aim)) = (order.mode, order.aim) {
            (yaw, climb) = aim;
        }
        let (yaw, climb) = (yaw * K, climb * K);
        let (bank, pitch_cap, yaw_cap) = if banks {
            (-local[0].atan2(local[1].abs()), -turn * cos, turn * sin)
        } else {
            (0.0, turn, turn)
        };

        // A spring and a damper on each angle.
        let [roll, pitch, heading] = [self.roll, self.pitch, self.heading].map(|a| signed(a) * K);
        let [w_roll, w_pitch, w_yaw] = self.spin.map(|w| w * K);
        let mut a_pitch = (climb - pitch) * 4.0 - 2.5 * w_pitch;
        let mut a_yaw = around(around(yaw) - around(heading)) * 4.0 - 2.5 * w_yaw;
        let a_roll = around(around(bank) - around(roll)) * 5.0 - 2.5 * w_roll;
        if pitch_cap <= w_pitch {
            if a_pitch > 0.0 {
                a_pitch = 0.0;
            }
        } else if -pitch_cap >= w_pitch && a_pitch < 0.0 {
            a_pitch = 0.0;
        }
        if w_yaw >= yaw_cap {
            if a_yaw > 0.0 {
                a_yaw = 0.0;
            }
        } else if -yaw_cap >= w_yaw && a_yaw < 0.0 {
            a_yaw = 0.0;
        }
        let kick = self.twist.map(|t| t * K * 10.0);
        let w_pitch = w_pitch + (a_pitch + kick[0]) * dt;
        let w_yaw = w_yaw + (a_yaw + kick[1]) * dt;
        let w_roll = w_roll + (a_roll + kick[2]) * dt;
        let pitch = around(pitch + w_pitch * dt);
        let heading = around(heading + w_yaw * dt);
        let roll = around(roll + w_roll * dt);

        // Thrust along the nose, settling on the speed asked for.
        let backing = matches!(order.mode, Mode::Backing | Mode::BackingFacing);
        let mut speed = order.speed;
        let mut thrust = order.thrust * THRUST;
        if backing {
            if speed > 0.0 {
                speed = -speed;
            }
            thrust = -thrust;
        }
        let v = self.velocity;
        if v[2] >= speed && thrust > 0.0 {
            thrust = -v[2];
        }
        if v[2] <= speed && thrust < 0.0 {
            thrust = -v[2];
        }
        let a = [-0.1 * v[0] + self.push[0], -0.1 * v[1] + self.push[1], thrust + self.push[2]];
        let v = add(v, scale(a, dt));

        // Moved by last frame's axes, which is the matrix the actor still has.
        let moving = from_frame(v, &axes);
        let mut at = add(p, scale(moving, dt));
        at[1] = hold(at, order, world);

        self.position = in_world(at);
        self.velocity = v;
        self.moving = moving;
        self.spin = [w_roll, w_pitch, w_yaw].map(|w| w / K);
        [self.roll, self.pitch, self.heading] = [roll, pitch, heading].map(|a| (a / K).rem_euclid(TURN));
        self.distance = distance;
        self.push = [0.0; 3];
        self.twist = [0.0; 3];
        target
    }
}

impl Body {
    /// `0x407960`, the simpler way some routines fly. The heading and pitch
    /// move toward the wanted ones by `rate` - the turn rate as a fraction a
    /// second, the type's `+0x68` over 65,536 - times the error, so they close
    /// by that fraction a second; the roll becomes minus half the heading's
    /// step; and it moves along the nose it had at the start of the frame at
    /// `speed`, then is held above the floor plus the clearance and under the
    /// ceiling less it. The rigid body's velocity and rates are left alone.
    pub fn glide(&mut self, wanted: (f32, f32), rate: f32, speed: f32, clearance: f32, world: &impl Surfaces, dt: f32) {
        let forward = direction(self.heading, self.pitch);
        let step = rate * dt;
        let turned = signed(wanted.0 - self.heading) * step;
        self.pitch = (signed(self.pitch) + (wanted.1 - signed(self.pitch)) * step).rem_euclid(TURN);
        self.roll = (-turned / 2.0).rem_euclid(TURN);
        self.heading = (self.heading + turned).rem_euclid(TURN);
        let mut at = add(self.position, scale(forward, speed * dt));
        at[1] = at[1].max(world.floor(at) + clearance);
        at[1] = at[1].min(world.ceiling(at) - clearance);
        self.position = in_world(at);
    }
}

/// The height after the class's floor and ceiling (`0x495528` onward). The
/// flyers keep above the floor plus their clearance and never below the
/// clearance itself; the tunnel classes above the floor plus clearance;
/// the rest only under the ceiling less the clearance. Where the two cross
/// the floor is looked for again just under the ceiling, and a height
/// caught between them goes to the middle.
fn hold(at: [f32; 3], order: &Order, world: &impl Surfaces) -> f32 {
    let clearance = order.clearance;
    let y = at[1];
    let floor = |y: f32| {
        let f = world.floor([at[0], y, at[2]]) + clearance;
        if ABOVE_GROUND.contains(&order.class) {
            f.max(clearance)
        } else if ANYWHERE.contains(&order.class) {
            f
        } else {
            y
        }
    };
    let mut lo = floor(y);
    let hi = world.ceiling(at) - clearance;
    if hi < lo {
        lo = floor(hi - to_units(1));
        if !ABOVE_GROUND.contains(&order.class) && !ANYWHERE.contains(&order.class) {
            lo = y;
        }
    }
    let middle = (lo + hi) * 0.5;
    if y < lo {
        if lo < hi {
            lo
        } else {
            middle
        }
    } else if y > hi {
        if hi > lo {
            hi
        } else {
            middle
        }
    } else {
        y
    }
}

/// Into -pi..pi, the way the engine's loops bring an angle round.
fn around(a: f32) -> f32 {
    use std::f32::consts::{PI, TAU};
    let mut a = a;
    while a > PI {
        a -= TAU;
    }
    while a < -PI {
        a += TAU;
    }
    a
}

/// `0x4922a0`: where to point to hit the player - the heading and pitch, in
/// circle units, of his position plus his velocity times the time a shot at
/// `shot_speed` would take to reach him, allowing for how fast he is drawing
/// away. A type with no shot speed points at him. Written to the actor's
/// `+0x60` and `+0x58`, which modes 3 and 4 steer by.
pub fn lead(from: [f32; 3], player: [f32; 3], velocity: [f32; 3], shot_speed: f32, dt: f32) -> (f32, f32) {
    let d = offset(from, player);
    let now = length(d);
    let receding = (length(offset(from, add(player, scale(velocity, dt)))) - now) / dt;
    let t = if shot_speed == 0.0 { 0.0 } else { now / (shot_speed - receding) };
    let (heading, pitch) = angles_of(add(d, scale(velocity, t)));
    let (heading, pitch) = over_the_top(heading, pitch);
    (heading.rem_euclid(TURN), pitch)
}
