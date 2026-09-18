---
title: The port's renderer
status: partial
covers: crates/hb-render
worklog: 13, 16, 17, 20, 21
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
- The three screen sizes the art is drawn for: 320x200, 320x400, 640x480.

## What is the renderer's own choice

Each of these is a decision taken because the engine's answer is not known. A
port that later reads the engine should revisit them.

| choice | why |
| --- | --- |
| Affine texture interpolation | The engine has a `perspectiveFlag` with three settings, so it switches on some threshold. The threshold is unknown and at a cell's size the difference is small. |
| Texture space scales, not wraps | A coordinate is a texel in a 256-unit space and the textures are 64 x 64. Scaling makes a corner-to-corner face one tile; wrapping makes it four, and puts a fine grid over everything. |
| Depth buffer | The engine's visibility scheme is not known. This sorts cells back to front by distance and settles the rest with a z-buffer. |
| Yaw 0 looks along +z | Nothing yet ties the engine's 16-bit heading to a world axis. |
| 90-degree field of view | Not established. |
| Draw distance of 220 units | Chosen so the fog ramp saturates before the edge. |
| The 118 colourless polygons are skipped | Four models have polygons with neither a material nor a flat colour before them, so nothing says what colour they are. |
| A chamber's first `.CL1` texture is the floor's | That is the order the terrain loader reads the two in. Nothing confirms which is which. |
| The sky wraps once around the horizon and once from horizon to zenith | The engine's projection is not known - a `skyTextureFlag` and a `"Sky clip overflow!"` diagnostic is all there is. This is the simplest thing that turns with the camera. |

## The units

World positions are 16.16 fixed point. One terrain cell is `0x80000`, which is
8.0 units, and the grid is 128 cells, so a level is 1024.0 units square and
wraps. A height query returns the altitude byte scaled by 2^15, so the terrain
spans 127.5 units from the bottom of its range to the top.

## What is drawn

The sky, ground triangles, both sets of ground boxes, chamber floors and
ceilings, the level's placed objects, and the cockpit over the top of it all.
Not yet: sprites, the HUD, or anything that moves.

A chamber's floor and ceiling are height fields that `heightAtGrid` accepts as
layers 2 and 3, so they use the same triangle split and the same corner heights
as the ground. Their two textures come from the cell's `.CL1` entry, floor
first - which is the order the loader reads them in and is not otherwise
confirmed. Because chambers sit below the ground they are invisible from above
and only appear once the eye is inside one; on `ROID` the whole playable volume
is one, which is what a level set in space should be.

An object comes from the instance list in the level's
[.DEF](../formats/level-text.md): a kind, a position, a heading and a scale.
Its mesh is normalised to -1.0..+1.0, so a vertex reaches the world as
`(vertex * scale) >> 14`. Positions are in the world's signed coordinates and
the terrain walk uses unwrapped indices around the eye, so a placement is
rebased onto the nearest copy of the wrapping world before it is projected -
otherwise an object at -300 units lands 1024 units from the ground it stands
on.

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

A box is drawn as four sides and a top. The bottom is skipped because it cannot
be seen from outside, and because which of the four side slots faces which way
is [not settled](../formats/terrain.md) - the sides use the slot that matches
their axis and take the first of the pair, which is right for the axis and may
be mirrored within it.

## Unknown

Everything about the original's rasteriser: its scan conversion, its clipping,
its visibility scheme, its use of the `perspectiveFlag`, `airShadowFlag`,
`ditherFlag`, `blendFlag` and `filterFlag` settings, and how the Direct3D path
differs from the software one.
