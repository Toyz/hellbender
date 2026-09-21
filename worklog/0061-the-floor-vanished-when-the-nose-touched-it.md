---
number: 61
title: The floor vanished when the nose touched it
date: 2026-09-20
area: render, port
files: crates/hb-render/src/scene.rs, crates/hb-render/tests/world.rs
---

# 61. The floor vanished when the nose touched it

Reported from playing: fly low enough that the ship's nose meets the ground
and the ground goes with it - the bottom of the frame turns black and stays
black until the camera lifts.

The renderer had no near-plane clipping. `project_onto` returned `None` for a
corner closer than 0.8 units in front of the eye, and every face that used it
did the same thing with that `None`: throw the whole face away.

```rust
let points: Option<Vec<Vertex>> = tri.corners.iter().map(...).collect();
let Some(points) = points else { drawn.clipped += 1; continue };
```

That is fine for a face entirely behind the eye and wrong for one that
straddles the plane, which is exactly the cell the camera is standing in. Its
corners are all around the eye, so as the camera drops, the near corners cross
the plane one by one and the cell disappears - and the cells beyond it next.
Looking straight down from a third of a unit above `FLOAT`'s ground, 234
ground triangles were drawn and two thirds of the frame was black.

The engine clips. Its polygon clipper is `0x4141a0`, and `"Sky clip
overflow!"` is that routine running out of room rather than anything to do
with the sky. So this clips too: a face is kept in view space until it has
been cut against `z = 0.8`, and only what is left is projected.

`clipped_face` is the whole of it - Sutherland-Hodgman against one plane,
interpolating the texture coordinates and the light along the cut edge with
the position, returning 0, 3 or 4 corners. The four faces that used to drop
now fan whatever comes back: the ground's two halves, a chamber's two, a
box's five, and a model's polygons, which had the same bug and would pop out
of existence when the camera reached them.

The same frame after: 436 ground triangles and no black. Across a whole level
it draws more of everything - `HOTH` 906 terrain triangles a frame before,
1,040 after - because faces at the edge of view were being dropped whole as
well. The benchmark did not notice: 276 frames a second before, 291 after.

`the_ground_survives_a_camera_that_touches_it` in `hb-render`'s world tests
puts the camera a third of a unit over the ground, points it down, and fails
if fewer than 300 ground triangles are drawn or more than a twentieth of the
lower half of the frame is black. Without the clip it fails on the first
count at 234.

One thing that looks like the same bug and is not: the black band on the
horizon in `JURASIC`. That is the fog fill, and `JURASIC.FOG`'s last row
sends every colour to index 255, which in `JURASIC.ACT` is `(0, 0, 0)`. The
engine draws the same black band under the same red sky.

**Still unknown:** whether the engine clips against the same 0.8 units - the
value here is the port's own, chosen as a tenth of a cell - and whether it
clips against the side and top planes too, which this port leaves to the
rasteriser's scissor.
