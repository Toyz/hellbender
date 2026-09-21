//! A joystick, read straight from the kernel.
//!
//! The game has a whole joystick to itself - `HELLBEND.INI` carries
//! `joystickActive`, `xStickMin`, `xStickMax`, `yStickMin`, `yStickMax`,
//! `throttleMin`, `throttleMax`, `rudderActive`, `rudderMin`, `rudderMax`,
//! `useJoystickDeadZone`, `joystickDeadZonePercent`, and a binding each for
//! `buttonFire`, `buttonWeapon`, `buttonThrottleUp`, `buttonThrottleDown` and
//! `buttonFunction0` to `buttonFunction3` - and the calibration screen walks
//! you through centring the stick, sweeping it, setting the throttle's ends
//! and the rudder's.
//!
//! None of that calibration is transcribed. What is here is the four axes and
//! the buttons, read from Linux's `js` device, which needs no library: an
//! event is eight bytes and the device reports its own centre. Anything else
//! - another platform, no device, no permission - leaves the keyboard alone.
//!
//! The axis numbers a device reports are its own business, so
//! `HB_JOY_X`, `HB_JOY_Y`, `HB_JOY_THROTTLE` and `HB_JOY_RUDDER` override
//! them, `HB_JOY_INVERT_Y=0` stops the usual inversion, and `HB_JOYSTICK`
//! names the device.

#[cfg(target_os = "linux")]
mod linux {
    use std::fs::File;
    use std::io::Read;
    use std::os::unix::fs::OpenOptionsExt;

    /// `O_NONBLOCK`, so a frame with no events costs one failed read.
    const NONBLOCK: i32 = 0o4000;

    /// One `js_event`: a millisecond stamp, the value, the kind and which
    /// axis or button it is.
    const EVENT: usize = 8;
    const BUTTON: u8 = 0x01;
    const AXIS: u8 = 0x02;
    /// Set on the burst of events the driver sends when the device opens,
    /// which report the current state rather than a change.
    ///
    /// They are not to be believed. `/dev/input/js0` is whatever the kernel
    /// numbered first, and on a machine with a touchscreen and no stick that
    /// is the touchscreen, whose axes sit wherever they were last touched -
    /// which arrives as a stick held hard over and takes the keyboard away.
    /// So an axis counts for nothing until an event without this bit moves
    /// it, which only something a hand is on will send.
    const INIT: u8 = 0x80;

    /// How far from centre an axis has to be before it counts. The engine
    /// has `joystickDeadZonePercent` for the same job.
    const DEAD_ZONE: f32 = 0.08;

    pub struct Stick {
        device: File,
        axes: [f32; 16],
        /// Which axes have moved since the device was opened. Until one has,
        /// it reads centred whatever the device says.
        live: [bool; 16],
        buttons: [bool; 32],
        /// What the buttons were before this frame's events, so a press can
        /// be told from a hold.
        was: [bool; 32],
        map: Map,
    }

    struct Map {
        x: usize,
        y: usize,
        throttle: usize,
        rudder: usize,
        invert_y: bool,
    }

    fn number(name: &str, default: usize) -> usize {
        std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
    }

    impl Stick {
        /// Open the device, or nothing if there is not one to open.
        pub fn open() -> Option<Stick> {
            let path = std::env::var("HB_JOYSTICK").unwrap_or_else(|_| "/dev/input/js0".into());
            let device =
                std::fs::OpenOptions::new().read(true).custom_flags(NONBLOCK).open(&path).ok()?;
            let map = Map {
                x: number("HB_JOY_X", 0),
                y: number("HB_JOY_Y", 1),
                throttle: number("HB_JOY_THROTTLE", 2),
                rudder: number("HB_JOY_RUDDER", 3),
                invert_y: std::env::var("HB_JOY_INVERT_Y").as_deref() != Ok("0"),
            };
            let mut stick = Stick {
                device,
                axes: [0.0; 16],
                live: [false; 16],
                buttons: [false; 32],
                was: [false; 32],
                map,
            };
            stick.poll();
            Some(stick)
        }

        /// Take whatever the device has said since the last frame. A device
        /// that has gone away stops reporting and everything reads centred.
        pub fn poll(&mut self) {
            self.was = self.buttons;
            let mut event = [0u8; EVENT];
            while let Ok(EVENT) = self.device.read(&mut event) {
                self.apply(event);
            }
        }

        /// One event: a millisecond stamp, an `i16` value, the kind with the
        /// init bit, and which axis or button it belongs to.
        fn apply(&mut self, event: [u8; EVENT]) {
            let value = i16::from_le_bytes([event[4], event[5]]);
            let initial = event[6] & INIT != 0;
            let kind = event[6] & !INIT;
            let which = event[7] as usize;
            match kind {
                AXIS if which < self.axes.len() => {
                    self.axes[which] = value as f32 / 32767.0;
                    self.live[which] |= !initial;
                }
                BUTTON if which < self.buttons.len() => {
                    self.buttons[which] = value != 0;
                    // A button held down at open is the same kind of lie.
                    if initial {
                        self.buttons[which] = false;
                    }
                }
                _ => {}
            }
        }

        fn axis(&self, which: usize) -> f32 {
            if !self.live.get(which).copied().unwrap_or(false) {
                return 0.0;
            }
            let v = self.axes.get(which).copied().unwrap_or(0.0);
            if v.abs() < DEAD_ZONE {
                return 0.0;
            }
            // Take the dead zone out of the travel rather than off it, so the
            // first degree past it is small and not a step.
            let scaled = (v.abs() - DEAD_ZONE) / (1.0 - DEAD_ZONE);
            scaled.min(1.0) * v.signum()
        }

        /// Pitch, turn and roll, in the order [`hb_sim::flight::Controls`]
        /// wants them. Pushing the stick forward pitches like the up key,
        /// which is why the y axis is inverted by default.
        pub fn stick(&self) -> [f32; 3] {
            let pitch = if self.map.invert_y { -self.axis(self.map.y) } else { self.axis(self.map.y) };
            [pitch, self.axis(self.map.rudder), self.axis(self.map.x)]
        }

        /// The throttle lever, 0 to 1, or nothing when the device has no such
        /// axis to report. A lever that has never moved reads at its resting
        /// place, which the driver sends on open.
        pub fn lever(&self) -> Option<f32> {
            if !self.live.get(self.map.throttle).copied().unwrap_or(false) {
                return None;
            }
            let raw = *self.axes.get(self.map.throttle)?;
            // Levers sit at -1 shut and +1 open, the opposite way round on
            // some, so this only says how far along the travel it is.
            Some((1.0 - raw) / 2.0)
        }

        pub fn button(&self, which: usize) -> bool {
            self.buttons.get(which).copied().unwrap_or(false)
        }

        /// Down this frame and up the last.
        pub fn pressed(&self, which: usize) -> bool {
            self.button(which) && !self.was.get(which).copied().unwrap_or(false)
        }

        /// One event, as if the device had sent it. The tests use this; the
        /// running game gets its events from [`Stick::poll`].
        #[cfg(test)]
        pub fn feed(&mut self, event: [u8; EVENT]) {
            self.apply(event);
        }

        /// Whether the device has a throttle axis worth listening to. A pad
        /// that reports four axes still has no lever, so the axis has to be
        /// named before anything is read from it.
        pub fn has_lever(&self) -> bool {
            std::env::var("HB_JOY_THROTTLE").is_ok()
        }

        /// Whether anything on the device has actually moved yet. Until it
        /// has, the port says nothing about it, because the device may well
        /// not be a joystick at all.
        pub fn woken(&self) -> bool {
            self.live.iter().any(|&l| l)
        }
    }
}

#[cfg(target_os = "linux")]
pub use linux::Stick;

/// The buttons this port listens to, in the engine's own names
/// (`buttonFire`, `buttonWeapon`, `buttonThrottleUp`, `buttonThrottleDown`).
/// Which button on the device is which is the device's business; these are
/// the usual order and `HB_JOY_FIRE` and friends move them.
pub const FIRE: usize = 0;
pub const AFTERBURNER: usize = 1;
pub const WEAPON: usize = 2;
pub const THROTTLE_UP: usize = 3;
pub const THROTTLE_DOWN: usize = 4;

#[cfg(not(target_os = "linux"))]
pub struct Stick;

#[cfg(not(target_os = "linux"))]
impl Stick {
    pub fn open() -> Option<Stick> {
        None
    }
    pub fn poll(&mut self) {}
    pub fn stick(&self) -> [f32; 3] {
        [0.0; 3]
    }
    pub fn lever(&self) -> Option<f32> {
        None
    }
    pub fn button(&self, _which: usize) -> bool {
        false
    }
    pub fn pressed(&self, _which: usize) -> bool {
        false
    }
    pub fn has_lever(&self) -> bool {
        false
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    /// The device is opened, so these run only where one can be. `/dev/null`
    /// serves: it never reports anything and the events go in by hand.
    fn stick() -> Option<Stick> {
        std::env::set_var("HB_JOYSTICK", "/dev/null");
        Stick::open()
    }

    fn event(kind: u8, which: u8, value: i16) -> [u8; 8] {
        let v = value.to_le_bytes();
        [0, 0, 0, 0, v[0], v[1], kind, which]
    }

    #[test]
    fn an_axis_moves_once_a_hand_has_moved_it() {
        let Some(mut s) = stick() else { return };
        s.feed(event(0x02, 1, -32767));
        // Pushed forward is nose up, which is the up key.
        assert!((s.stick()[0] - 1.0).abs() < 0.01, "{:?}", s.stick());
        s.feed(event(0x02, 1, 0));
        assert_eq!(s.stick()[0], 0.0);
    }

    /// The whole reason this exists: `/dev/input/js0` on a machine with a
    /// touchscreen and no stick is the touchscreen, and its axes sit wherever
    /// they were last touched. Believed, that is a stick held hard over, and
    /// it takes the keyboard away.
    #[test]
    fn the_opening_burst_is_not_believed() {
        let Some(mut s) = stick() else { return };
        s.feed(event(0x02 | 0x80, 0, -32767));
        s.feed(event(0x02 | 0x80, 1, 32767));
        s.feed(event(0x01 | 0x80, FIRE as u8, 1));
        assert_eq!(s.stick(), [0.0; 3], "a device that has not moved is centred");
        assert!(!s.button(FIRE), "and its buttons are up");
        assert!(!s.woken());
        // One real event and that axis is worth listening to. The other is
        // still where the burst left it, and still ignored.
        s.feed(event(0x02, 0, 32767));
        assert!((s.stick()[2] - 1.0).abs() < 0.01, "{:?}", s.stick());
        assert_eq!(s.stick()[0], 0.0);
        assert!(s.woken());
    }

    #[test]
    fn a_small_deflection_is_the_dead_zone_and_a_big_one_is_not() {
        let Some(mut s) = stick() else { return };
        s.feed(event(0x02, 0, 1000));
        assert_eq!(s.stick()[2], 0.0, "inside the dead zone");
        s.feed(event(0x02, 0, 32767));
        assert!((s.stick()[2] - 1.0).abs() < 0.01);
    }

    #[test]
    fn a_button_is_held_once_and_pressed_once() {
        let Some(mut s) = stick() else { return };
        s.feed(event(0x01, FIRE as u8, 1));
        assert!(s.button(FIRE) && s.pressed(FIRE));
        s.poll();
        assert!(s.button(FIRE) && !s.pressed(FIRE), "still down, no longer a press");
    }

    #[test]
    fn the_lever_runs_from_shut_to_open() {
        let Some(mut s) = stick() else { return };
        std::env::set_var("HB_JOY_THROTTLE", "2");
        s.feed(event(0x02, 2, -32767));
        assert!(s.lever().unwrap() > 0.99, "{:?}", s.lever());
        s.feed(event(0x02, 2, 32767));
        assert!(s.lever().unwrap() < 0.01, "{:?}", s.lever());
        std::env::remove_var("HB_JOY_THROTTLE");
    }
}
