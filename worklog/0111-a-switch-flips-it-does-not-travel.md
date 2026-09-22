---
number: 111
title: A switch flips, it does not travel
date: 2026-09-21
area: decomp, world, port
files: crates/hb-sim/src/quake.rs, crates/hb-sim/tests/quake.rs, crates/hb-fly/src/main.rs, crates/hb-render/tests/solids.rs, docs/formats/scenery.md
---

# 111. A switch flips, it does not travel

"Shooting switches isn't working - they're not changing texture or anything."
Reported while flying, and every part of it looked right in isolation, which is
what made it worth measuring rather than reading.

## What was already right

A test against `MORBOS` said so: all 35 of its shot-triggered entries start from
a hit in their own cell at their own height, and all 35 of those cells hold a
box, so the `.QKE`'s cell numbering lines up with the world. And a trace in
`hb-fly` showed real shots landing on a switch and alternating "1 door started"
and "0 door started" - started, then found already going, which is correct.

So the shot was arriving and the switch was starting. Nothing swapped a
texture.

## What was wrong

A switch's own box has `rest` and `target` both **zero**. It has thickness -
1,280 to 3,840 words of it - but it goes nowhere. It flips.

The port woke a box's watchers only when that box *moved*
(`0x410da0`'s reading), so a switch that never moves never woke the door it
points at, the door never left rest, and the switch was never lit: `on` in the
lighting test is "something watching my id is away from rest", and nothing ever
was. Hence no texture, ever, on any switch in the game.

Waking belongs on the trigger, not on the motion. `Quakes::start` now wakes the
watchers of whatever it starts, which is also the shape of the engine's own
`0x410d00` - the switch goes looking for the door when it is thrown.

With that, twenty seconds of `MORBOS` produces 42 texture swaps where it
produced none.

## And one texture the level does not carry

41 of those 42 name a texture in the level's `.TEX` list. The other names
`SUPRSWT1.RAW`, which is in `GAME.POD` but not in `MORBOS`'s list, so the port
had nothing to swap to and left the switch on one face. It now loads a name it
does not know and gives it a slot.

A test that asserted the watcher was still in `About` after two frames now
asserts it is not resting after the shot, which is the same thing said about
the corrected timing.

**Still unknown:** whether the engine adds an unlisted switch texture to the
level's own list, as this does, or resolves it some other way.
