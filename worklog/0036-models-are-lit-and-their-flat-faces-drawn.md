---
number: 36
title: Models are lit, and their flat faces drawn
date: 2026-09-18
area: decomp,format,render
files: crates/hb-formats/src/mrgl.rs,crates/hb-formats/src/anim.rs,crates/hb-formats/tests/against_the_game.rs,crates/hb-render/src/scene.rs,crates/hb-render/src/level.rs,docs/engine/rendering.md,docs/formats/mrgl.md
---

# 36. Models are lit, and their flat faces drawn

Worklog 35 found `0x19`, a flat polygon in 34 level models that the port never
drew. Reading its draw handler to fill it in turned up something larger.

## Every model polygon is lit

`0x19` takes its light from `0x48a6a0`, and so does the textured polygon
handler every model uses (`0x458500` for 0x18). The function is short: the
dot product of the polygon's normal with the light in model space, negated
and clamped to 0..1, scaled into the gap between the ambient and full. The
light and ambient are the level's, the same `.LVL` lines 18 and 19 the ground
is shaded with - the level loader sets both at `0x44c105`. The port had drawn
every model at full light, so buildings were flat cut-outs against a shaded
ground. Now a face away from the light sits at the ambient - 0.625 of full in
`FLOAT`, 0.25 in `HOTH` - and a face into it at full.

`0x48a6a0`'s 28 callers settle which polygon kinds are lit: 0x0e, 0x18, 0x1e
and 0x22, and 0x19; not 0x0f (the powerup sprites) or 0x11. The animated
`.TXT` models draw through the 0x18 handler as well (`0x4584df`), so they are
lit. A per-object addition to the ambient, `0x48b550`, reads a light grid
around the object - most likely explosions or the ship's headlight - and is
not modelled.

## The flat faces

A `0x19` polygon is the indexed layout from worklog 35 filled with one colour.
The colour comes from a `0x0a` record: 0 and up picks a band of the palette
from two tables (`0x50c4e8` low, `0x50c528` high) and the polygon's light
chooses within it; below 0 is a palette index. All 215 in the game follow a
colour of 0, the band from index 0 to 31, so they are dark faces set into
textured models such as `FMBUNK.BIN` and `FDTRAINX.BIN`.

## What the port reads as flat colour is not

The port has coloured the seven untextured models by the `0x17` record since
worklog 16. The engine's handler for `0x17` does not touch the colour at all -
it advances a counter - and the value is always in a model's first record and
looks like a count. The engine's flat colour is `0x0a`'s, and with textures on
a 0x18 polygon with no material before it draws with whatever texture was
current. None of the seven is placed in a level, so this is left as a known
wrong guess on the format page.

## Tests

The format tests keep the 0x0e-family counts they had and gain one for the
flat polygons: 215 in 34 models, all after colour 0, all with unit normals,
and the band arithmetic at its ends.
