---
number: 13
title: A perspective frame, and two convention bugs the picture found
date: 2026-09-17
area: render, port, world
files: crates/hb-render, docs/engine/rendering.md
---

# 13. A perspective frame, and two convention bugs the picture found

`hb-render` draws the world in perspective, in the same 8-bit indexed colour
space the game uses. `hb fly hoth out.png 40 100 12288 60 4096` puts the eye 60
units above cell (40, 100) looking down 22.5 degrees, and what comes out is
snowfields either side of an ice canyon, shaded, in a 320x200 frame.

It is not a transcription. The original's rasteriser has not been read, and
`docs/engine/rendering.md` lists every place where this renderer had to choose
something the engine's answer is not known for - affine interpolation, a depth
buffer, a field of view, a heading convention. What is reproduced is everything
the data dictates: the palette, the ramps, the per-cell shade, the cell
geometry, the texture indexing, the three screen sizes.

## The picture found two bugs that parsed fine

**A box is absent when its bottom equals its top, not when its top is zero.**
`has_box_b` tested the top against zero, which is correct for box set A, whose
altitudes scale upward from a "nothing here" byte of 0. Box set B scales
downward, so its "nothing here" byte of 255 becomes an altitude of zero and its
byte 0 becomes -32,640 - and testing the top against zero calls almost every
cell a box. `FLOAT` reported 16,124 box B cells where it has 1,076. The first
frame was a wall of buildings in every direction.

The right test is `bottom != top`, and the data agrees: across `FLOAT` the
bytes are equal in 15,242 cells for set A and 15,308 for set B, which is the
right order of magnitude for a level with a few hundred structures.

This is the third time the two altitude scalings have caused a bug - see
[8](0008-half-the-world-was-in-the-wrong-hemisphere-and-the-diagonal-a.md) for
the first. `Scale` being on the type has stopped the parse going wrong; it does
not stop a predicate being written for the wrong hemisphere.

**Texture space scales, it does not wrap.** A texture coordinate is a texel in
a 256-unit space and the textures are 64 x 64, so the obvious reading is that a
corner-to-corner face repeats the texture four times. That puts a fine grid over
every surface, which is what the second frame looked like. Scaling by
`size / 256` makes a face one tile, which is what the render should look like
and what the model format implies as well - a polygon node maps its texture
corner to corner with 1 and 255, and no artist draws a ship panel meaning it to
tile four times.

Neither bug is catchable by a parser. Both produce values in range, both
round-trip, and both are only wrong in the way the world looks.

## A convention I asserted and had backwards

The first camera test asserted that yaw 0 looks along +x. It looks along +z -
the rotation is `rx = dx*cos - dz*sin`, so at yaw 0 the view axis is z. The
test failed immediately, which is the right outcome, and the fix was to write
down the convention rather than change the code, because nothing yet ties the
engine's 16-bit heading to a world axis. The `.LVL` headings and the model
format's `angleList` are in the same circle; neither has been tied to an axis.
Until one is, the port's heading zero is arbitrary and says so.

## What the frames show

```
hoth:    2325 ground, 536 box triangles, 2515 clipped
jurasic: 2247 ground,   0 box triangles, 2381 clipped
float:   2337 ground, 2902 box triangles, 3905 clipped
```

`HOTH` is snow and ice; `JURASIC` is black rock with red lava-lit cliffs in the
distance and metal plating underfoot; `FLOAT` is platforms with a lot of
structure on them. Each matches its own top-down map from
[12](0012-a-texture-word-is-twelve-bits-and-four-and-the-map-draws-itse.md),
which is the check that the perspective path and the orthographic one agree.

`JURASIC` drawing zero box triangles from the middle of the map is consistent
with its map: its structures are around the edges.

**Still unknown:** chambers, models, sprites, the sky and the cockpit are not
drawn. Fog is wired up but has not been checked against anything. Which of a
box's four side slots faces which way is still open, and the sides are drawn
with the slot that matches their axis, which is right for the axis and may be
mirrored within it.
