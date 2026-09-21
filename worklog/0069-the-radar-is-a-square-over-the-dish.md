---
number: 69
title: The radar is a square over the dish
date: 2026-09-20
area: decomp, ui, port
files: crates/hb-render/src/hud.rs, crates/hb-fly/src/main.rs, crates/hb/src/main.rs, docs/engine/hud.md
---

# 69. The radar is a square over the dish

[[68]] left the radar as a still unknown: none of the three HUD routines
touches it, and it is not in the cockpit art either, because the art only has
the dish and the dish is empty in a fresh frame. The way in was the one
string the image has about it - `Radar destroyed.` - which led to the flag
`0x512744`, which is read in three places, and one of them is `0x474834`.

## The square

`0x474a70` builds a rectangle out of the view's size: 507, 6, 101 by 101 in
the 640x480 everything on the HUD is authored in. Rendered at 320x200 that
is x 253 to 303 and y 2 to 44, which lands exactly on the dish in
`ckpt200.raw` - so the dish is a frame someone drew around a box the engine
fills. With the cockpit off and the view zoomed, the top of the square moves
to the top of the view instead.

The same four globals are the rect for the objective arrow, and `0x474af0`
is a second setter that makes them the whole view. So there is one small
viewport that two things take turns drawing into.

## What it plots

`0x437148` walks the object table once a frame, skipping the player, anything
with hit points at or below zero, and anything with bit 1 of its flags set.
For each survivor it wraps the offset at the world's edge - the `shl 6`,
`sar 6` pair - turns it by the heading, and scales it:

    offset (16.16) >> 13, times the box's width, >> 11

which is 256 world units across the box, so the radar reaches about 128
units - an eighth of the world. Then it clips to the box and to three pixels
inside the view, and draws.

A blip is not a dot. It is a short string, six pixels to the right of the
point, taken from a field in the object's own record at `0x667128 + n * 112`.
A loader at `0x448298` fills that field from a table of strings by index from
0x1393, and that table is not in the executable's string resources - I walked
them to check. So the port plots a two-pixel mark, and what the mark should
say is written down rather than guessed.

## Also half read

`0x475290` is the objective arrow, and it is not a sprite: it sets a camera,
a rotation and a shade, and draws the model at `0x6210c0` into the rect
above. When the objective is behind you - heading between 0x7800 and 0x8800 -
it plots a small pattern of pixels in index 138 instead.

## In the port

`hud::radar` takes the blips already wrapped and turned, as offsets in world
units right and forward, and plots them the engine's way. `hb-fly` fills it
from the live objects, and `hb fly <level> <out.png>` fills it from the
level's placements, which is how I checked it: render, diff against the same
frame without blips, and look at where the difference is. It is inside the
square, and it moves with the heading.

**Still unknown:** the string table at 0x1393, which is what a blip says; the
reticle, which is in none of the HUD routines; and the colour a blip is drawn
in, which comes through the remap at `0x606a20` - the glyph blit indexes it
by the font's pixel value, so a blip is `table[1]` on `table[0]`, and what
that table holds in flight has not been read. The port draws blips in the
readout's own index 47.
