---
number: 118
title: Missiles smoke, and the smoke thins
date: 2026-09-22
area: render, port
files: crates/hb-sim/src/smoke.rs, crates/hb-sim/tests/smoke.rs, crates/hb-sim/src/turret.rs, crates/hb-sim/src/weapons.rs, crates/hb-fly/src/battle.rs, crates/hb-fly/src/main.rs, crates/hb-render/src/level.rs, crates/hb-render/src/scene.rs, crates/hb-render/tests/weather.rs, docs/engine/simulation.md
---

# 118. Missiles smoke, and the smoke thins

The audit in 112 listed `0x479040` - 1,073 bytes, reached only as an operand,
naming `puff4.raw` - among the live routines nobody had read. `simulation.md`
already said a SAM missile leaves "a smoke puff every sixteenth of a second
(`0x478e35`)". Reading the code around that address says otherwise.

## The timer and the kinds

`0x478d90` runs over the whole missile pool every frame, the player's missiles
with the SAM sites', and counts a timer at `+0x3c` down from 3/16 of a second.
When it runs out a byte table picks what happens by kind. The Dead-On, the two
MIRVs and kind 21 go to `0x478e35`, which calls `0x4010e0` and winds the timer
back a sixteenth; the super weapon does the same through `0x401460`. The
Viper - which is also the SAM's missile, kind 19 - the cruise missile and the
cluster missile go to `0x478e4b`, which calls `0x478f00` and does **not** wind
the timer back. So those three lay smoke every frame once they start, and the
line in `simulation.md` described the other kinds' clock.

## The smoke

`0x478f00` puts a segment from the missile's last position to its current one
into a pool of 100 at `0x612430`, round robin, with a two-second life, an
eighth of a second before it shows, and a width of 0.38 units. `0x479040`
draws it as a triangular tube a hundredth longer than the segment - the
cross-section's corner up, two below - with the radius the width times the
life left. A trail is thickest behind the missile and gone two seconds back.

The texture is the one surprise. `puff4.raw` indexes entries 0 to 21 of its own
`puff4.act`, a ramp of greys that shares one entry with `VGA.ACT`. The
engine's texture loader (`0x489d80`) opens `<name>.act` beside every texture
and converts through it when it finds one, and through the level palette
otherwise - which is also why the blast frames, with no `.act` of their own,
come out right as they are. The port converts `puff4` through the level's
`.MAP`, the way it already does the sky.

## In the port

`hb_sim::smoke` is the pool and each missile's `Trail`; the battle lays and
ages it; `hb-render` draws the tube. A render test lays forty segments across a
view of `FLOAT` and finds a thin grey trail - thin because 0.38 units is thin
from twelve away.

**Still unknown:** what `0x4010e0` and `0x401460` draw for the other missiles
on their sixteenth-second clock.
