---
number: 97
title: What the ship can fly through
date: 2026-09-21
area: decomp, port, test
files: crates/hb-sim/src/collide.rs, crates/hb-sim/src/combat.rs, crates/hb-fly/src/main.rs, crates/hb-render/tests/solids.rs, docs/engine/simulation.md, docs/port/plan.md
---

# 97. What the ship can fly through

Flying through the radar dishes is the report, again. This is the third time
object collision has come up and the first two answers were both half right, so
this time I measured every step of it rather than reading one routine.

## The port's collision does work

A scratch test over three levels says so. Every one of the 476 placements in
`hoth` has a hit volume, every one of them answers yes to
`combat::object_at` at its own position, and every terrain box under a dish or
a tower pushes a point at its middle back out - 38 of 38 tried. So neither the
volumes nor `push_out` is the problem.

What the boxes *are* is the problem. The box under `mobdish.txt` at
(-342.5, 45.0, 336.2) runs from y 35 to y 45, and the dish sits at y 45. The
cells hold the plinth the dish stands on, and nothing above it. Across
`morbos`, `hoth` and `float` the scenery is 60% boxed that way and 40% not
boxed at all.

## Which is what the engine does

`0x40d650`, the routine the player's frame calls every frame with its own
position, walks the live actors and skips two classes outright -
`cmp eax, 0` and `cmp eax, 9` at `0x40d6ad` and `0x40d6af`. Class 0 is the
scenery and class 9 is the bunkers: in `hoth` that is `hmradar.bin`,
`hdnucry1.bin`, `reac1.txt`, `hdcore.bin` and 78 `wbunker.bin`. And the ship's
own collision (`0x427280`) collects nothing but heightfields - `0x428790`
per cell into `0x4294c0`, `0x4290f0` and `0x428ac0`, all of them
`[x * 128 + z] * 18` lookups into the terrain arrays.

So the original really does let you fly through a radar dish, and it does not
even scratch you for it. Which also corrects a line on the simulation page: the
example it used for grinding damage was a bunker, and a bunker is exactly one
of the two classes that never takes any.

## The port stops you anyway

Three times asked is a decision. `hb_sim::collide::solid_of` turns a hit volume
by its placement's heading, squares it off and hands `push_out` a box, and
`hb-fly` builds one for every placement whose class the engine's ram test
skips. Nothing else changes: a tank still grinds you down rather than stopping
you, because that is what the engine does and it is visible.

It is the port's only deliberate departure, and `docs/port/plan.md` now has a
section that says so.

Three tests hold it down: every solid pushes a point at its own middle out,
no level's start is inside one, and the biggest box on any of the three levels
is 45.7 units across - a reactor - so nothing has walled off a map.

**Still unknown:** whether the levels' designers meant the plinth boxes to be
the whole of a building's collision, or whether the missing 40% is data that
was never authored. And what `0x5b36a4`, which exempts the player from ram
damage, is set by.
