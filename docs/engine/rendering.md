---
title: The port's renderer
status: partial
covers: crates/hb-render
worklog: 13, 16, 17, 20, 21, 22, 25, 28, 29
---

# The port's renderer

`hb-render` draws Hellbender's world in perspective, in the 8-bit indexed
colour space the original uses. It is **not** a transcription of the original's
rasteriser - those inner loops have not been read. What it does reproduce is
everything the data dictates, so that its output is in the same colour space
and the same world units as the game's.

This page is about the port. What the original does is under
[docs/formats/](../README.md) and in the worklog.

## What comes from the data

- The palette, from the level's [.ACT](../formats/act.md).
- Shading and fog through the level's [ramps](../formats/colour-tables.md), at
  the per-cell intensity from the [shading database](../formats/terrain.md).
- Cell geometry: the 8.0-unit cell, the wrapping grid, the parity-dependent
  triangle split, the corner altitudes - all from
  [hb-world](../formats/terrain.md).
- Texture selection: the 12-bit index out of a `.CLR`, `.CL0`, `.CL1` or
  `.CL2` word, resolved against the level's `.TEX` list.
- How a texture lies on a cell or a box face: u along x, v against z, then the
  word's orientation code's turn and mirrors, as `0x413c20`, `0x414f60` and
  `0x413940` do it - see [terrain](../formats/terrain.md).
- Light: each ground vertex takes its own grid point's shade, or the level's
  ambient when the point is in shadow, and the ground is Gouraud shaded between
  them. A box corner is full light or ambient by its shadow bit. Box faces are
  the engine's, sign and all, and a side hidden by its neighbour's box is not
  drawn; nor is ground inside a box.
- The three screen sizes the art is drawn for: 320x200, 320x400, 640x480.

## What is the renderer's own choice

Each of these is a decision taken because the engine's answer is not known. A
port that later reads the engine should revisit them.

| choice | why |
| --- | --- |
| Perspective-correct texture, light and depth | The engine has a `perspectiveFlag` with three settings, so it switches between affine and corrected on some threshold, which is unknown. The port first chose affine and it bent textures badly on the large ground triangles near the eye, so it now always corrects. |
| Texture space scales, not wraps | A coordinate is a texel in a 256-unit space and the textures are 64 x 64. Scaling makes a corner-to-corner face one tile; wrapping makes it four, and puts a fine grid over everything. |
| Depth buffer | The engine's visibility scheme is not known. This sorts cells back to front by distance and settles the rest with a z-buffer. |
| Heading 0 looks along +z and increases toward +x | No longer a choice: the recorded demo flight measures it to a median 0.8 degrees. The port had the direction backwards until then. |
| 90-degree field of view | Not established. |
| Draw distance of 220 units | Chosen so the fog ramp saturates before the edge. |
| The 118 colourless polygons are skipped | Four models have polygons with neither a material nor a flat colour before them, so nothing says what colour they are. |
| A chamber is lit flat by the low byte of its 24-bit shade | The value is not decomposed. |
| Past the draw distance is the fog colour | The fog ramp's last row sends every colour to one index; the frame is filled with it before the sky, so the band between the last cell and the horizon is fog rather than a hole. |
| The sky wraps once around the horizon and once from horizon to zenith | The engine's projection is not known - a `skyTextureFlag` and a `"Sky clip overflow!"` diagnostic is all there is. This is the simplest thing that turns with the camera. |

## The units

World positions are 16.16 fixed point. One terrain cell is `0x80000`, which is
8.0 units, and the grid is 128 cells, so a level is 1024.0 units square and
wraps. A height query returns the altitude byte scaled by 2^15, so the terrain
spans 127.5 units from the bottom of its range to the top.

## What is drawn

The sky, ground triangles, both sets of ground boxes, chamber floors and
ceilings, the level's placed objects, shots and missiles as points, and the
cockpit over the top of it all.
Terrain textures animate on the wall clock where the level's
[`.ANI`](../formats/level-text.md) says they should. Not yet: sprites, the HUD,
or anything that moves through the world.

Animation is kept out of the rasteriser: `Level::texture_frames` returns, for a
given instant, which texture slot each slot resolves to - the identity except
where a cycle is running - and the scene walk looks every index up through it.
So the drawing code never knows a texture moves.

A chamber's floor and ceiling are height fields that `heightAtGrid` accepts as
layers 2 and 3, so they use the same triangle split and the same corner heights
as the ground. Their two textures come from the cell's `.CL1` entry, floor
first: the engine draws them through the ground's own cell routine, from
`0x41911a` with the word at chamber `+4` and from `0x41957a` with the word at
`+6` and its flip flag set. Because chambers sit below the ground they are invisible from above
and only appear once the eye is inside one; on `ROID` the whole playable volume
is one, which is what a level set in space should be.

An object comes from the instance list in the level's
[.DEF](../formats/level-text.md): a kind, a position and a heading. Its size is
its **type's radius**, field 2 of the type record's first line, which is what
the engine's actor draw passes (`0x40da00`). Its mesh is normalised to
-1.0..+1.0, so a vertex reaches the world as `(vertex * radius) >> 14`. A wreck
is drawn at the radius of the type it replaced.

Until worklog 28 the port drew objects at the placement's second field, which
turned out to be hit points. That drew them at a median of one twentieth of
their size - buildings a few units across in a world of 8-unit cells - and is
the likeliest reason the first playtest found everything "too big": the
terrain was right and the things on it were tiny.

Positions are in the world's signed coordinates. The terrain walk starts from
the eye's cell **before wrapping** - `camera.x >> 19`, no mask - and each
cell's data is looked up through the wrapped index, so the ground is drawn in
the camera's own frame. A placement is rebased onto the copy of the wrapping
world nearest the eye before it is projected. Until worklog 28 the walk started
from the wrapped cell, which put the ground 1,024 units away whenever the
camera stood at a negative coordinate - half the world rendered as empty sky.
`a_view_is_the_same_from_either_side_of_the_wrap` renders one place from both
sides of the wrap and requires the two frames to match.

The sky is drawn first and writes no depth, so everything covers it. Its
texture is brought into the level's palette through the level's `.MAP` - see
[the sky](../formats/sky.md) - because a sky palette shares nothing with its
level's.

The cockpit is blitted last, with index 0 transparent. It needs no remap: it is
drawn in `VGA.ACT` and a level's own palette agrees with `VGA.ACT` on 240 of
its 256 entries.

Each polygon is drawn with the material that precedes it in the node stream, or
with its flat colour when it has no material - see [MRGL](../formats/mrgl.md).

Objects whose kind names a `.TXT` model are skipped: that is the animated model
format and it has no parser yet. `HOTH` places 476 objects of which 465 have a
`.BIN` mesh; `FLOAT` places 293 of which 269 do.

A box is drawn as four sides and a top, each from the slot the engine uses for
that face: slot 0 faces -z, 1 +z, 2 +x, 3 -x, 4 is the top. The bottom is not
drawn from outside.

## Unknown

Everything about the original's rasteriser: its scan conversion, its clipping,
its visibility scheme, its use of the `perspectiveFlag`, `airShadowFlag`,
`ditherFlag`, `blendFlag` and `filterFlag` settings, and how the Direct3D path
differs from the software one.
