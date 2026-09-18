---
number: 20
title: Height anywhere, collision, and the chambers turn out to be underneath
date: 2026-09-17
area: world, render, port
files: crates/hb-world/src/grid.rs, crates/hb-render/src/scene.rs, crates/hb-fly/src/main.rs
---

# 20. Height anywhere, collision, and the chambers turn out to be underneath

Three things that follow from the ground query being closed in
[10](0010-the-ground-query-closes-and-a-level-that-looks-like-its-own-n.md), and
which between them make the thing feel less like a viewer.

## The height at a position, not at a corner

`heightAtGrid` answers only at a cell corner. `groundTriangleInt` is what
answers anywhere: it produces a point on the containing triangle and that
triangle's normal, and a point plus a normal is a plane.

`Grid::height_at` reaches the same plane from the three corner altitudes
instead - barycentric weights over the triangle the half test picks. Shorter,
and it avoids reproducing the engine's normalisation rounding in a place where
that rounding does not matter. What does matter is picking the same triangle:
take the wrong half and the answer comes off the wrong plane on every other
cell of the grid.

The test is on synthetic terrain rather than the game's, which is the right
shape for this: a ramp whose altitude byte equals the cell's x index, where the
answer at a corner must equal `heightAtGrid` there and the answer halfway
between two corners must be halfway between their heights. Both hold, and 500
scattered positions all return a height inside the terrain's range and land in
the cell the engine's own test picks.

## Collision

`hb-fly` now keeps the eye above the ground and above anything standing on it -
`ceiling_of_solid` takes the interpolated ground height and the top of either
box set. `c` toggles it.

This is not the engine's collision. `intersectingBoxSurface` and the ground
triangle queries are pieces of a real system that has not been read. It is the
height query used honestly, and it is enough that you can fly along a valley
floor instead of through it.

The telemetry line prints the cell, the eye's height, the ground under it and
the speed, which is the readout that made everything below testable by flying
rather than by rendering a still.

## The chambers are underneath

Drawing a chamber is free once the ground is drawn: its floor and ceiling are
height fields that `heightAtGrid` accepts as layers 2 and 3, so the same
triangle split and the same corner heights apply. The two textures come from
the cell's `.CL1`, floor first, which is the order the loader reads them and is
the one thing here that is a guess.

The first render drew 2,072 chamber triangles and changed not one pixel. That
is correct and it took a moment to see why: chambers are **below** the ground.
Their altitudes use the downward scaling from
[8](0008-half-the-world-was-in-the-wrong-hemisphere-and-the-diagonal-a.md) and
run from -32,640 to 0 while the ground runs from 0 up. A chamber is a void
carved under the terrain, so from above it is hidden by the surface, exactly as
it should be.

Putting the eye below ground shows them. On `ROID` the result is a ceiling
above and a floor far below with rock between - and `ROID`'s chamber floor is
byte 0 in all 16,384 cells, meaning -32,640 everywhere, with the ceiling
varying. The whole playable volume of a level set in an asteroid field is one
enormous chamber. `HOTH` is the other extreme, with a chamber in 5,414 of its
cells tracing the canyon network the
[top-down map](0012-a-texture-word-is-twelve-bits-and-four-and-the-map-draws-itse.md)
showed.

So "chamber" is the engine's word for the space the player flies through where
the terrain is not simply open sky above a surface. That reading is consistent
with every level and was not obvious from the byte layout.

**Still unknown:** which of a chamber's two `.CL1` textures is the floor's. The
engine's actual collision. Sprites, the sky and the cockpit are still not
drawn.
