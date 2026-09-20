---
number: 39
title: Shots are models
date: 2026-09-20
area: decomp,render,port
files: crates/hb-sim/src/weapons.rs,crates/hb-render/src/level.rs,crates/hb-render/tests/world.rs,crates/hb-fly/src/main.rs,docs/engine/simulation.md,docs/engine/rendering.md
---

# 39. Shots are models

Every shot in the port was a coloured point, because the port had nothing to
draw instead. The weapon table had the answer all along.

## A word I had mis-assigned

Worklog 37 worked out that a weapon row starts before its speed, and put the
model name 28 bytes back. The loader for the shot models (`0x476473`) walks
the table from `0x50e780` and reads an integer and then the name, which puts
the row's start at 32 bytes back, not 28. That integer says how the shot is
drawn: 0 a laser, 1 a missile, 2 nothing. It is 2 for the Valkyrie Cannon, for
the afterburner, the smart bomb, the mine and the cloak, and 0 or 1 for
everything that flies.

## Which way it faces

`0x4769cf` dispatches on the kind. Most are drawn along their own angles;
kinds 2, 4 and 9 to 16 - the dispersion cannon's, the fireball, and the eight
boss weapons - are turned by the view's angles instead, the same way the
powerups are, because they are round things that should look the same from
anywhere. Kinds 18 to 22 reach the "should not need default case" arm: the
missiles are drawn by the missile system, not this one.

The Valkyrie Cannon is its own case. It has no shot model; it cycles
`muzzle.bin`, `muzzle2.bin` and `muzzle3.bin`, advancing one every time a shot
is drawn - so a burst of them flickers through three shapes rather than
drawing one shape three times.

The laser models turn out to be the sprite family from worklog 35: a quad,
per-vertex texels, an indexed polygon, and a texture whose index 0 is a hole.
They needed nothing new to draw. The missiles are ordinary solid models -
`rocket4.bin` is a finned rocket, 42 polygons.

## In the port

`hb-render` loads a model for every weapon row that has one, and the three
muzzle flashes, all at size 1.0; `hb-fly` puts each shot and missile in the
scene as a placement turned along its velocity, or to the eye for the kinds
that want that, and keeps the old coloured point for anything whose model did
not load. One test checks that every weapon the port fires has its model, that
the Valkyrie has its flashes, and that a shot in front of the eye is drawn.
