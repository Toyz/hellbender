---
number: 74
title: A touchscreen is not a joystick
date: 2026-09-20
area: port, flight
resolves: 73
files: crates/hb-fly/src/stick.rs, crates/hb-fly/src/main.rs, crates/hb-sim/src/flight.rs, crates/hb-sim/tests/flight.rs, docs/port/plan.md
---

# 74. A touchscreen is not a joystick

[[73]] shipped a joystick and took the keyboard away with it.

`/dev/input/js0` is not "the joystick". It is whatever the kernel numbered
first that has axes and buttons, and on this machine that is

```
N: Name="ILITEK       ILITEK-TOUCH Mouse"
H: Handlers=event14 js0 mouse3
```

a touchscreen. Its axes do not spring back to the middle - they sit wherever
they were last touched - and the driver reports that resting place in the
burst of events it sends when the device is opened. Read as a stick, that is
one held hard over and never let go, and since the port made an axis *replace*
the key ramp rather than add to it, nothing the keyboard did could be seen.

Two things were wrong and both are worth fixing on their own.

## An axis has to earn its place

The opening burst has a bit set on every event to say it is the current state
rather than a change. The port was taking those at face value, because on a
real stick they are the truth - it is how a throttle that has not moved
reports where its lever is. On a device that is not a stick they are a lie,
and there is no way to tell from the value which one you have.

So an axis now counts for nothing until an event *without* that bit moves it,
which only something with a hand on it will ever send. A button held down at
open is treated the same way. `hb-fly` says which it has:

```
joystick: open, waiting for an axis to move
joystick: an axis moved - flying with it
```

The right long answer is to ask the device what it is - the `js` driver has
ioctls for its name and its axis count - but nothing in this workspace does
ioctls, and "has anything actually moved" costs one bit and answers the same
question for every device rather than a list of known-bad ones.

## A stick should never be able to silence a key

The second half is the one that matters whatever the device is. An axis was
replacing the six key ramps outright. Now it takes the larger of the two per
direction, so a stick adds to the keys and can never take one away - a stick
pushed left does not cancel the right arrow, it just loses to it.

Three tests: a centred stick turns the ship exactly as far as the key alone
does, a stick pushed the other way does not stop the key, and the opening
burst leaves everything centred with the buttons up until one real event
arrives.

**Still unknown:** unchanged from [[73]] - the engine's own response curve at
`0x463df3`, and its `xStickMin`/`xStickMax` calibration, which this replaces
with whatever range the device reports.
