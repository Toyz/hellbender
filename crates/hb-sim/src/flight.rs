//! The player's ship: `HELLBEND.EXE`'s flight model, `0x463aa0`.
//!
//! Read from the engine, per frame:
//!
//! - Each steering key ramps an input: held, it rises by twice the frame time
//!   to 1.0; released, it falls by four times the frame time to 0
//!   (`0x464a8e`). The INI binds up, down, left, right, roll left and roll
//!   right (`upKey=72` and so on; the loader at `0x42d2b3` puts them at
//!   `0x512760`..`0x512774`).
//! - The throttle, 0 to 1.0 (`0x512588`), moves by the frame time while the
//!   throttle keys are held (`0x464367`) - one second from stop to full.
//! - The three turn rates and the local velocity are **halved** (`0x463aba`),
//!   then the inputs are added, each scaled by 1/7 (`0x2492`): pitch by up
//!   minus down, yaw by right minus left, and roll by right minus left roll
//!   plus 0.73 of the yaw input (`0xbb80`) - a turn banks the ship.
//! - With `autoLevel=1` the roll rate is also pulled toward level: by the roll
//!   angle over seven, over the frame time, clamped to `0xc30`, scaled by
//!   `1 - sin^2(pitch)` (`0x4641f7`). Past a quarter turn it levels upside
//!   down.
//! - Forward speed gains eight units times the throttle (`0x4644a4`), or a
//!   flat 24 on the afterburner (`0x464475`).
//! - The rates, times the frame time, turn the ship about its own axes
//!   (`0x4631f0`, angles in the engine's 16-bit circle), and the local
//!   velocity, turned into the world, moves it (`0x464633`).
//!
//! Halving every frame and adding the same every frame settles at twice the
//! addition whatever the frame rate, so: 16 units a second at full throttle,
//! 48 on the afterburner, and 2/7 of a turn a second - 103 degrees - on a
//! held key. The recorded demo flies at a median 16.5 units a second with a
//! 90th percentile of 49, and turns at a 90th percentile of 18,002 units a
//! second against the 18,724 this predicts.
//!
//! The engine's model has no strafe and no vertical thrust: the side and
//! vertical velocities are only ever halved.

use hb_formats::fixed::{circle, radians, to_units, TURN};
use hb_formats::vector::{add, along, axes, cross, dot, from_frame, normalise, scale};

/// `0x2492`, the input scale.
const SEVENTH: f32 = to_units(9362);
/// Where the forward speed settles at full throttle, and on the afterburner,
/// units a second: twice the eight and 24 added each frame (`0x4644a4`,
/// `0x464475`).
const FULL_THROTTLE: f32 = 16.0;
const BURNER: f32 = 48.0;
/// `0xbb80`: how much of the yaw input banks the ship.
const BANK: f32 = to_units(48000);
/// `0xc30`, the auto-level's largest pull a frame, in turns a second.
const LEVEL_CLAMP: f32 = 3120.0 / TURN;

/// What is held down this frame.
#[derive(Debug, Clone, Copy, Default)]
pub struct Controls {
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
    pub roll_left: bool,
    pub roll_right: bool,
    pub throttle_up: bool,
    pub throttle_down: bool,
    /// The afterburner. In the game it is a selectable weapon (row 22 of the
    /// weapon table, `0x612220`); here it is held.
    pub afterburner: bool,
    /// A stick's deflection in place of the six key ramps: pitch, turn and
    /// roll, each -1 to 1, with pitch positive where [`Controls::up`] is
    /// held. The ramps exist so a key feels like a stick being pushed, so a
    /// stick sets them rather than nudging them - which is what the engine
    /// does with an axis at `0x463df3`, though its own curve, dead zone and
    /// `xStickMin`/`xStickMax` calibration are not transcribed here.
    pub stick: Option<[f32; 3]>,
    /// A throttle lever, 0 to 1, in place of the throttle keys.
    pub lever: Option<f32>,
}

#[derive(Debug, Clone)]
pub struct Ship {
    /// World units.
    pub position: [f32; 3],
    /// The ship's own axes in the world: right, up, forward.
    pub right: [f32; 3],
    pub up: [f32; 3],
    pub forward: [f32; 3],
    /// Pitch, roll and yaw rates, in turns a second (`0x50cc54`, `0x50cc58`,
    /// `0x50cc5c`).
    pub rates: [f32; 3],
    /// Right, up and forward velocity in the ship's frame, units a second
    /// (`0x50cc48`, `0x50cc4c`, `0x50cc50`).
    pub velocity: [f32; 3],
    /// The ramped key inputs: up, down, left, right, roll left, roll right.
    pub keys: [f32; 6],
    /// 0 to 1.
    pub throttle: f32,
    /// `autoLevel`, 1 in the shipped INI.
    pub auto_level: bool,
}

impl Ship {
    /// A ship at a position, level, facing a heading in the engine's circle.
    pub fn new(position: [f32; 3], heading: f32) -> Ship {
        let [right, up, forward] = axes(heading, 0.0, 0.0);
        Ship {
            position,
            right,
            up,
            forward,
            rates: [0.0; 3],
            velocity: [0.0; 3],
            keys: [0.0; 6],
            throttle: 0.0,
            auto_level: true,
        }
    }

    /// Pitch, roll and heading in the engine's 16-bit circle, as the pose
    /// stores them: positive pitch is nose down, heading 0 is +z increasing
    /// toward +x, and positive roll is the left wing down.
    pub fn angles(&self) -> [f32; 3] {
        let f = self.forward;
        let pitch = (-f[1]).clamp(-1.0, 1.0).asin();
        let heading = f[0].atan2(f[2]);
        let roll = self.right[1].atan2(self.up[1]);
        [circle(pitch), circle(roll), circle(heading)]
    }

    /// Put the ship where a recorded demo has it: a position, pitch, roll
    /// and heading in the circle, moving at `velocity` in the world.
    /// `0x44d180` writes the pose and `0x464800` rebuilds the ship's
    /// orientation from the angles.
    pub fn set_pose(&mut self, position: [f32; 3], [pitch, roll, heading]: [f32; 3], velocity: [f32; 3]) {
        let frame = axes(heading, pitch, roll);
        [self.right, self.up, self.forward] = frame;
        self.position = position;
        self.velocity = along(velocity, &frame);
    }

    /// Turn the ship about its own right axis, in turns. Positive is nose
    /// down, as everywhere else here.
    ///
    /// The jump-out at `0x45a2a0` drives the ship's pitch itself rather than
    /// through the controls, and this is what it needs.
    pub fn pitch_by(&mut self, turns: f32) {
        let (sp, cp) = (turns * std::f32::consts::TAU).sin_cos();
        let (f, u) = (self.forward, self.up);
        self.forward = normalise(add(scale(f, cp), scale(u, -sp)));
        self.up = normalise(add(scale(u, cp), scale(f, sp)));
    }

    /// Forward speed, units a second.
    pub fn speed(&self) -> f32 {
        self.velocity[2]
    }

    /// The ship's velocity in the world.
    pub fn world_velocity(&self) -> [f32; 3] {
        from_frame(self.velocity, &[self.right, self.up, self.forward])
    }

    /// The controls into the ship's inputs: the six key ramps, a stick in
    /// their place, and the throttle. The first part of [`Ship::step`].
    pub fn take_input(&mut self, controls: &Controls, dt: f32) {
        let held = [

            controls.up,
            controls.down,
            controls.left,
            controls.right,
            controls.roll_left,
            controls.roll_right,
        ];
        for (key, on) in self.keys.iter_mut().zip(held) {
            *key = if on { (*key + 2.0 * dt).min(1.0) } else { (*key - 4.0 * dt).max(0.0) };
        }
        // A stick is already where the ramp would have climbed to, so it
        // stands in for the ramp rather than adding to it - but only where it
        // is pushed further than the keys are. Whichever is asking for more
        // wins, so a stick never takes the keyboard away.
        if let Some([pitch, turn, roll]) = controls.stick {
            let split = |v: f32| (v.clamp(0.0, 1.0), (-v).clamp(0.0, 1.0));
            let (up, down) = split(pitch);
            let (right, left) = split(turn);
            let (roll_right, roll_left) = split(roll);
            for (key, axis) in
                self.keys.iter_mut().zip([up, down, left, right, roll_left, roll_right])
            {
                *key = key.max(axis);
            }
        }
        if controls.throttle_up {
            self.throttle += dt;
        }
        if controls.throttle_down {
            self.throttle -= dt;
        }
        if let Some(lever) = controls.lever {
            self.throttle = lever;
        }
        self.throttle = self.throttle.clamp(0.0, 1.0);
    }

    /// The port's own, for a recorded demo, which holds the ship's pose and
    /// nothing of what the pilot held: controls that would have flown the
    /// ship the way it moved since `before`, `dt` ago, so the cockpit's hand
    /// and gauges move with it. The stick is the pitch and yaw rates as a
    /// fraction of a held key's 2/7 of a turn a second - the 1/7 input
    /// settled at twice - and the throttle is the forward speed over the 16
    /// units a second full throttle settles at; past 32, midway to the
    /// afterburner's 48, the afterburner is taken to be lit. The engine's
    /// demo sets none of this (`0x44d180`).
    pub fn controls_for(&self, before: [[f32; 3]; 3], dt: f32) -> Controls {
        let [right, up, _] = before;
        let turned = |across: [f32; 3]| dot(self.forward, across).clamp(-1.0, 1.0).asin() / std::f32::consts::TAU / dt;
        let full = 2.0 * SEVENTH;
        let deflection = |rate: f32| (rate / full).clamp(-1.0, 1.0);
        // Nose down is a positive pitch, which the up key gives.
        let stick = [deflection(-turned(up)), deflection(turned(right)), 0.0];
        let speed = self.speed();
        Controls {
            stick: Some(stick),
            lever: Some((speed / FULL_THROTTLE).clamp(0.0, 1.0)),
            afterburner: speed > (FULL_THROTTLE + BURNER) / 2.0,
            ..Controls::default()
        }
    }

    /// One frame of `0x463aa0`.
    pub fn step(&mut self, controls: &Controls, dt: f32) {
        if dt <= 0.0 {
            return;
        }
        self.take_input(controls, dt);

        for r in &mut self.rates {
            *r *= 0.5;
        }
        for v in &mut self.velocity {
            *v *= 0.5;
        }

        let [up, down, left, right, roll_left, roll_right] = self.keys;
        let pitch_in = (up - down) * SEVENTH;
        let yaw_in = (right - left) * SEVENTH;
        let roll_in = (roll_right - roll_left) * SEVENTH;
        self.rates[0] += pitch_in;
        self.rates[2] += yaw_in;
        self.rates[1] -= BANK * yaw_in + roll_in;

        if self.auto_level {
            let [pitch, roll, _] = self.angles();
            let s = radians(pitch).sin();
            let weight = 1.0 - s * s;
            // The roll as a signed 16-bit angle, in turns. Past a quarter turn
            // the engine subtracts half a turn without wrapping again
            // (`0x4642da`), so rolled left past 90 degrees it levels upside
            // down, but rolled right past 90 it rolls back toward upright -
            // the pull is clamped either way.
            let mut r = (roll / TURN + 0.5).rem_euclid(1.0) - 0.5;
            if r <= -0.25 || r >= 0.25 {
                r -= 0.5;
            }
            let pull = (r * SEVENTH / dt).clamp(-LEVEL_CLAMP, LEVEL_CLAMP);
            self.rates[1] -= pull * weight;
        }

        self.velocity[2] += if controls.afterburner { 24.0 } else { 8.0 * self.throttle };

        // Turn about the ship's own axes. Positive pitch is nose down,
        // positive yaw turns right, positive roll lowers the left wing.
        let angle = |rate: f32| rate * dt * std::f32::consts::TAU;
        let (sp, cp) = angle(self.rates[0]).sin_cos();
        let (f, u) = (self.forward, self.up);
        self.forward = add(scale(f, cp), scale(u, -sp));
        self.up = add(scale(u, cp), scale(f, sp));
        let (sy, cy) = angle(self.rates[2]).sin_cos();
        let (f, r) = (self.forward, self.right);
        self.forward = add(scale(f, cy), scale(r, sy));
        self.right = add(scale(r, cy), scale(f, -sy));
        let (sr, cr) = angle(self.rates[1]).sin_cos();
        let (r, u) = (self.right, self.up);
        self.right = add(scale(r, cr), scale(u, sr));
        self.up = add(scale(u, cr), scale(r, -sr));
        // Keep the axes square as they accumulate.
        self.forward = normalise(self.forward);
        self.right = normalise(cross(self.up, self.forward));
        self.up = cross(self.forward, self.right);

        self.position = add(self.position, scale(self.world_velocity(), dt));
    }
}
