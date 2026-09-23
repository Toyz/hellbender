---
number: 123
title: The demo plays back as the engine plays it
date: 2026-09-22
area: decomp, port
files: crates/hb-formats/src/demo.rs, crates/hb-formats/tests/demo.rs, crates/hb-sim/src/flight.rs, crates/hb-fly/src/main.rs, crates/hb-fly/src/battle.rs, docs/formats/demo.md
resolves: 25
---

# 123. The demo plays back as the engine plays it

The demo was broken in play: it opened on the arrival and the opening camera,
it jerked, and it never fired. The port had written its own player when 25
read the file format, before any of the engine's was read: the camera at
`pose_at(wall clock since the program started % length)`, positions
interpolated, angles not.

## The engine's

"Demo Play Done" leads to `0x44d180`, called each frame from `0x44d470`,
which then adds the frame time to the demo's clock `0x505130`. The clock is
zeroed at the level's start (`0x44cec0`). Each frame the player finds the
first pose past the clock and the pose before it, and interpolates every
value in integers - including the three angles, the short way round with
`shl 16; sar 16`. It writes the result into the player's pose at `0x5b3830`
and rebuilds the ship's orientation (`0x464800`). Then it clears the key
array at `0x5b36c0` and, if the later pose's fourth value is set, holds the
key at `fireKey`'s scan code: the fourth value 25 could only call "0 or 1"
is the fire button. Key records are pressed a pose late - when the pose it
plays toward moves on, the ones between the two before it.

And the attract mode never shows a demo's introduction. `0x4558e0` loads
`demo1.dmo` - always that one - sets the attract flag `0x512630` and runs the
level; the level start skips the briefing, the arrival and the opening camera
when that flag is set (`0x4815e1`, `0x481693`).

## What was wrong in the port, against that

- The clock ran from the program's start, so by the time the movies and the
  briefing and the opening camera were over, the recording was well under
  way, and looping on the wall clock.
- Angles were not interpolated at all; each frame took the angles of the pose
  before, so the view turned in steps at the recording's rate.
- The fire button and the key records were ignored, and the guns were off.
- The battle was stepped with the player a million units away, so nothing
  flew at the recorded ship.

`hb_formats::demo::Player` is `0x44d180` and `0x44d470`; `pose_at` is the same
interpolation, for `hb demo`. In `hb-fly` a demo skips the introduction,
plays from the level's start on the frame time, fires and selects weapons
with the recorded trigger and keys, runs the whole fight - the recorded
flight takes no damage, which is the port's choice since it cannot dodge -
and plays again when "Demo Play Done". The level-start bookkeeping that was
written out twice in `hb-fly` is one function now, which is where the demo's
skip lives.

`DEMO1` at sixty frames a second never turns more than 505 circle units
between frames.

**Still unknown:** what the attract screen does after "Demo Play Done";
whether the engine lets a demo flight be killed.
