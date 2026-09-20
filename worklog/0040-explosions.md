---
number: 40
title: Explosions
date: 2026-09-20
area: decomp,engine,render,port
files: crates/hb-sim/src/explosion.rs,crates/hb-sim/tests/explosion.rs,crates/hb-render/src/scene.rs,crates/hb-render/src/level.rs,crates/hb-render/tests/world.rs,crates/hb-fly/src/battle.rs,crates/hb-fly/src/main.rs,crates/hb/src/main.rs,docs/engine/simulation.md
---

# 40. Explosions

Things died quietly in the port: the model swapped for its wreck, a sound
played, and that was all.

## Eleven puffs

`0x47f3f0` takes a position and a size and makes eleven puffs: ten of twice
the size scattered within it, and one of four times at the centre. Each goes
into the effect pool at `0x612230` - sixteen slots of 32 bytes, first free
slot, or over the first when there is none, so a big explosion can cut an
older one short.

A puff is a position, a size, a clock and a rate. The rate is drawn at random
between a quarter and three quarters, so the puffs of one explosion run at
different speeds. The clock advances by the frame time times the rate, and the
frame is the clock shifted down twelve bits: sixteen frames of a sixteenth of
a second, `blast1.raw` to `blast16.raw`. The two-second life in the record is
never reached - the sixteenth frame ends it first, between one and four
seconds depending on the rate.

The draw (`0x4771e0`) is a square facing the eye, laid out corner to corner
across the texture with the engine's usual 4-to-251 texel inset, its corners a
size out from the middle. Whose size? The mesh it builds passes a unit of 256
and corners of `size >> 8`, which comes to the size in world units - so the
ten puffs of a radius-4 object are eight units across and the middle one
sixteen.

## What blows up

The player's death is one of these at two units, followed by the loadout being
reset (`0x426e90`) - the three death paths are being shot down, flying into the
ground, and flying into a ceiling. A destroyed actor goes elsewhere: `0x40cb3c`
spawns an actor of a type looked up by name, which is a mechanism I have not
read. The port uses the puffs there as well, sized by the type's radius, and
says so.

## In the port

`hb_sim::explosion` is the pool and the clock; `hb-render` gains
`draw_sprite`, a camera-facing textured square with texel 0 left clear, and
loads the sixteen frames; `hb-fly` bursts one wherever something is destroyed
and where the player dies, and resets the stores on death as the engine does.
`hb look <level> <n> <out> --blast N` draws one frame for a look at it.

Three tests cover the shape of a burst, a puff's sixteen frames, and the
pool's sixteen slots; one more in `hb-render` checks a puff actually puts
pixels on the screen.
