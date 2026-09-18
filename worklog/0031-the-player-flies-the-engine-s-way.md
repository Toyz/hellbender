---
number: 31
title: The player flies the engine's way
date: 2026-09-17
area: decomp,engine,port
files: crates/hb-sim/src/flight.rs,crates/hb-sim/tests/flight.rs,crates/hb-render/src/camera.rs,crates/hb-fly/src/main.rs,docs/engine/simulation.md
---

# 31. The player flies the engine's way

`hb-fly`'s flight was the port's own from the start: a view you pushed around
at up to 90 units a second, with strafe and climb keys. The engine's is
`0x463aa0`, found by following the writers of the player velocity
`0x5b3a00` - a `sub esp,0x2b4` function whose one caller is `0x464c8e`.

## What it does each frame

- **Keys ramp.** `0x464a8e` gives each steering key an input that rises by
  twice the frame time while held, to 1.0, and falls by four times it when
  released. The INI loader (`0x42d2b3` on) puts `leftKey`, `rightKey`,
  `upKey`, `downKey`, `rollLeftKey`, `rollRightKey`, `throttleUpKey` and
  `throttleDownKey` at `0x512760` to `0x51277c`, which is how the inputs got
  their names: left feeds `0x62d6d0`, right `0x62d670`, up `0x62d668`, down
  `0x62d638`, roll left `0x62d674`, roll right `0x62d66c`.
- **Everything is halved.** The first thing the function does is `sar 1` on
  the three turn rates (`0x50cc54`, `0x50cc58`, `0x50cc5c`) and the three local
  velocities (`0x50cc48`, `0x50cc4c`, `0x50cc50`).
- **Inputs add, at a seventh.** Pitch rate gains (up - down) x `0x2492`, yaw
  rate (right - left) x `0x2492`, and roll rate loses (roll right - roll left)
  x `0x2492` plus 0.73 (`0xbb80`) of the yaw input - turning banks the ship. A
  joystick and a mouse feed the same sums with dead zones.
- **Auto-level** (`autoLevel=1`, `0x5126c0`): the roll rate loses the roll
  angle x 1/7 over the frame time, clamped to `0xc30`, weighted by
  `1 - sin^2(pitch)`. Past a quarter turn it subtracts `0x8000` without
  re-wrapping, which levels a leftward roll upside down but brings a rightward
  one back upright.
- **Thrust.** The forward velocity gains `8.0 x throttle`, or a flat 24.0 with
  weapon 22 selected (the afterburner), or 72.0 with a further flag. The
  throttle moves by the frame time while X or Z is held.
- **Turn and move.** The rates times the frame time go to `0x4631f0` as angles
  in the 16-bit circle (its scale is `2 pi / 65536`, at `0x4ee988`), which
  builds a rotation that is composed with the orientation; the angles are
  read back out, the pose matrix rebuilt, and position moves by the matrix
  times the local velocity times the frame time.

Halving and adding every frame settles at twice what is added, at any frame
rate. So full throttle is **16 units a second**, the afterburner **48**, and a
held key **2/7 of a turn a second** - 18,724 in the engine's circle. Against
the recorded demo: median speed 16.5 and 90th percentile 49; 90th percentile
heading rate 18,002, 99th percentile pitch rate 18,734. The demo's right turns
hold a negative roll 77 per cent of the time and its left turns a positive one
87 per cent, which is the coupled bank and fixes roll's sign: positive roll is
the left wing down.

Nothing drives the side or vertical velocity - only a reset writes them - so
the engine has no strafe and no climb thrust. The port's keys for them are
gone.

One sign is argued rather than read: that a positive pitch rate raises the
pitch angle, which is nose down, so **up dives**. It is the sign under which
the same composition makes "right" turn right, and it is the usual flight-sim
convention; decoding `0x4631f0`'s x87 sequence would settle it outright.

## In the port

`hb_sim::flight::Ship` keeps the ship's axes as vectors and turns them about
themselves; its tests hold 16 and 48 units a second, independence from the
frame rate (15 to 144 fps), 18,724 a second on a held key, the signs, and
auto-level. The camera gained roll, which also lets the demo replay show the
recorded bank. `hb-fly` binds the INI's keys - arrows, X and Z, Home and PgUp -
plus W/S and A/D, and Shift for the afterburner, which in the game is a weapon
you select.

**Still unknown:** `0x4631f0`'s exact composition order; the ship's collision
with the ground and boxes; what the 72.0 thrust flag (`0x5126b4`) is; what the
mode flag `0x5b39e8` changes (it swaps the pitch input's source and drops the
bank coupling).
