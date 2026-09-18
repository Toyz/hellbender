---
number: 30
title: The view is ninety degrees down too, and the sky is a ceiling
date: 2026-09-17
area: decomp,render,port
files: crates/hb-render/src/camera.rs,crates/hb-render/src/scene.rs,crates/hb-render/src/level.rs,crates/hb-fly/src/main.rs,docs/formats/sky.md
---

# 30. The view is ninety degrees down too, and the sky is a ceiling

Two of the renderer's remaining guesses - the field of view and the sky - read
out of the engine.

## The projection

The vertex transform `0x42a690` rotates into view space and then clips with
outcodes that compare `x` and `y` directly against `z` (`0x42a805`): the
frustum is `|x| <= z`, `|y| <= z`. It projects with `x * [0x5b3650] / z +
[0x5b3658]` and `y * [0x5b3654] / z + [0x5b365c]`.

Those four are written in five places. Four are special views - a 63x56 box
scaled from a 640x480 layout (`0x41f8b0`, an instrument), a quarter-screen
inset at three quarters across (`0x475440`, twice), a 64x64 render-to-texture
(`0x42fea3`), and a (0, 0, 10, 10) preview (`0x475290`). The fifth is
`setViewport(left, top, right, bottom)` at `0x485910`, 29 callers, which at
`0x485940` takes half the rectangle's width and height, each rounded down to
even, less one, as the two scales, and centres on the rectangle. The game loop
calls it with `(0, 0, W, H)` at `0x45a0f1`.

So the in-game view is 90 degrees across and **90 degrees down**, whatever the
screen's shape. At 320x200 the scales are 159 and 99. The port used
`(w/2) / tan(45)` for both - 160 and 160 - which is 64 degrees down: every
vertical distance on screen was 1.6 times the engine's. That is the other half
of "everything feels too big", alongside worklog 28's object scale.

A zoom factor `[0x512550]` divides the matrix rows in another view mode (the
zoom keys step it 0x1000 at a time, `0x40e983`); it is 1.0 in the cockpit.

The original put 320x200 on a 4:3 monitor, so a pixel was 1.2 times taller than
wide. `hb-fly` now opens a 4:3 window and minifb stretches into it. Together
with the engine's projection, the picture has the original's proportions -
which are a little flatter than square, since 90 by 90 over 4:3 is anamorphic.

`Camera::screen` carries the numbers and
`the_view_is_ninety_degrees_across_and_down_as_the_engine_sets_it` pins them.

## The sky

`"Sky clip overflow!"` belongs to a polygon clipper (`0x4141a0`) that cuts
ground polygons at altitude 128.0 - `[0x5055d4]`, the same constant the class-25
launcher and the death check use. The sky itself is `0x44fd70`, reached from
the background routine `0x451180` when `skyTextureFlag` (`[0x5125d0]`) is set:

- a quad at `(+-0x1fffff, 0, +-0x1fffff)` about `(0, 128.0, 0)`, with the
  camera's offset divided by 256 - which projects exactly like a quad 8,192
  units each way at altitude 128;
- corner coordinates `scroll +- 0x3fffffff`, so `u = scroll + 2x`, `v = scroll
  + 2z`: a 64 x 64 tile every 128 world units;
- the scroll advancing by a velocity times the frame time, the velocity being
  `.LVL` line 41 (parser `0x44beb0` into `0x6670d0`, `0x6670d4`) - 10.0 in
  eleven levels, 0 in fifteen: drifting clouds;
- full intensity, no fog.

`0x451180` picks by altitude with a cloud half-thickness of `[0x5055d8]` = 2.0:
below 126 the plane from below, inside 126-130 a cleared frame, above 130
`0x450d10`. The port draws the first case by casting each pixel's ray onto the
plane; it replaces a cylinder that was entirely its own invention. JURASIC is
now a red volcanic overcast and FLOAT a green one.

The sky is laid on the world's absolute coordinates, so it needs the wrapped
eye; the wrap test caught a one-row difference when it did not.

**Still unknown:** `0x450d10` (above the clouds), the space levels' sky
(`0x450500`, `0x4506e0`), the walls the sky routine adds when a level has no
boxes or chambers, and `.LVL` line 42.
