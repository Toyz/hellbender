---
number: 73
title: A joystick, eight bytes at a time
date: 2026-09-20
area: port, flight
files: crates/hb-fly/src/stick.rs, crates/hb-fly/src/main.rs, crates/hb-sim/src/flight.rs, crates/hb-sim/tests/flight.rs, docs/port/plan.md
---

# 73. A joystick, eight bytes at a time

Hellbender was made for a stick. `HELLBEND.INI` gives one a whole block -
`joystickActive`, `xStickMin`, `xStickMax`, `yStickMin`, `yStickMax`,
`throttleMin`, `throttleMax`, `rudderActive`, `rudderMin`, `rudderMax`,
`useJoystickDeadZone`, `joystickDeadZonePercent` - and a binding each for
`buttonFire`, `buttonWeapon`, `buttonThrottleUp`, `buttonThrottleDown` and
`buttonFunction0` to `buttonFunction3`. There is a calibration screen behind
it: centre the stick, sweep it, set the throttle's ends, sweep the rudder.

So the axes are named, which settles what a stick is for: x and y steer, the
rudder is its own axis, the throttle is its own axis. In the flight model
`0x463df3` takes that branch when `joystickActive` is above 1, and multiplies
the reading by a pair of scales before it reaches the same place the keys
reach.

## Where an axis goes

The port's flight model already had the shape for this without meaning to.
The engine ramps each steering key from 0 to 1 over half a second
(`0x463b4a`, ported in [[24]]), which is a key pretending to be a stick being
pushed. So a stick does not nudge the ramp - it *is* the ramp, and
`Controls::stick` sets all six of them outright. `Controls::lever` does the
same for the throttle, which the keys walk and a lever simply is.

Three tests say it: a stick fully over turns further than a key held the same
time, by about the half second the key spent climbing; half over turns half
as far, which a key cannot ask for; and a lever puts the throttle where it
is rather than walking it there.

## Reading the device

No library. Linux's `js` device is eight bytes an event - a millisecond
stamp, an `i16` value, a kind byte with an init flag, and which axis or
button it is - so `stick.rs` opens `/dev/input/js0` non-blocking and drains
it once a frame. The init burst the driver sends on open is worth keeping
rather than skipping: it is how a throttle that has not moved reports where
it is resting.

Four tests drive the decoder through `/dev/null`, which opens and never says
anything, with the events fed in by hand.

Which axis is which is the device's business and no two agree, so
`HB_JOY_X`, `HB_JOY_Y`, `HB_JOY_RUDDER` and `HB_JOY_THROTTLE` move them,
`HB_JOYSTICK` names another device, and `HB_JOY_INVERT_Y=0` stops the usual
inversion. The throttle is ignored unless it is named: a pad's third axis is
a thumbstick, not a lever, and it would sit the ship at half power forever.

Every other platform, and a machine with no device, gets what it had before.

**Still unknown:** the engine's own response. `xStickMin` and `xStickMax` are
a calibration this does not have - the device reports its own range instead -
and `joystickDeadZonePercent` is a flat 8% here. What `0x463df3` does with
the reading between `0x51257c` and `0x59d124`, which is two multiplies and a
comparison against a pair of limits, has not been read, so a stick here is
linear where the game's may not be. Also not wired: `buttonFunction0` to
`buttonFunction3`, which the INI binds and nothing here asks for.
