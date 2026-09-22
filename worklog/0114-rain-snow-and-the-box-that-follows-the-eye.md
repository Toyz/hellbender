---
number: 114
title: Rain, snow, and the box that follows the eye
date: 2026-09-22
area: render, port
files: crates/hb-sim/src/weather.rs, crates/hb-sim/tests/weather.rs, crates/hb-render/src/scene.rs, crates/hb-render/tests/weather.rs, crates/hb-fly/src/main.rs, docs/engine/weather.md
---

# 114. Rain, snow, and the box that follows the eye

"We're missing the rain and all." The `.LVL`'s weather line was named long ago
(worklog 48) and nothing drew it.

## Finding it

The level writer (`0x44c830`, dead, but it prints every field with its
comment) puts the weather after `{ Weather (...)` from level struct `+0x41c`,
which is `0x6670cc`. `tools/funcs.py xref` gives four readers, all together at
`0x49c9b0`..`0x49d290`: snow, rain, the lightning spawner the `.LVL` page
already knew, and the strike. The frame at `0x4811e0` calls the first two
between the world and the cockpit.

## What they are

The same machine twice. A pool - 300 flakes, 200 drops - scattered through a
16-unit cube around the eye when the level starts, falling at 4.88 or 9.77
units a second with a drift too small to see. Every frame each moves, and any
that is now more than eight units from the eye on an axis jumps sixteen back
across. The cube goes where the eye goes and is always full. Nothing moves or
draws below the ground, above the sky layer, or under an overhanging box
(`0x41c4d0` answering anything but "nothing above").

A flake is a single 320x200 pixel (`draw320x200SizeDot`, 2x2 at 640x480) in
palette index 14. A drop is a line of 0.305 units in index 5, pointing up plus
however far the eye moved this frame - `0x62d6d4`, which the flight step
writes as the camera less its copy from before the step - so the rain leans
into the ship's travel. Both colours are `0x4566c0`'s ramp 1, the same ramp
tables the MRGL polygons use.

## Lightning, read but not drawn

`0x49d290` is also read: a strike lands at the sky layer within 40 units,
plays `lghtng.wav`, lifts the models' light floor to full for half a second
(`0x48a640`, `0x51138c`), swaps the sky's shading table, and sends `thun-c.wav`
after a delay of the distance at 20.6 units a second. The bolt itself is drawn
by a callback not read yet, so the port has no lightning. `weather.md` has the
lot.

## The port

`hb_sim::weather` in 16.16, and two draw helpers in `hb-render` that write
straight into the frame as the engine does. A render test draws a frame of
`IOWAH` and one of `HOTH` from real positions: 241 pixels of rain, 46 of snow -
most flakes vanish against `HOTH`'s own snow, which the original's did too.

**Still unknown:** the bolt, the sky tables, and how the `0x606a20` remap is
built; the port takes the remap as the identity.
