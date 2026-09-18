---
number: 9
title: The polygon node, and a ship that looks like a ship
date: 2026-09-17
area: format, render, port
files: crates/hb-formats/src/mrgl.rs, crates/hb/src/view.rs, docs/formats/mrgl.md
---

# 9. The polygon node, and a ship that looks like a ship

[5](0005-mrgl-the-node-stream-inside-a-model-and-its-size-table-as-a.md)
left the polygon node with 16 unread bytes of header and a guess about its
texture coordinates. Both are now settled, and the guess was wrong.

```
+0x00  u32  type                     0x0e, 0x11, 0x18, 0x1e or 0x22
+0x04  i32  corner count             always 3 or 4
+0x08  i32  normal.x                 16.16, unit length
+0x0c  i32  normal.y
+0x10  i32  normal.z
+0x14  i32  plane constant           16.16, signed
+0x18  per corner, 12 bytes:
         i32  vertex index
         i32  u                      texel << 16, 0 to 255
         i32  v                      texel << 16, 0 to 255
```

`CUBE.BIN`'s first face is the whole format in one record:

```
+0x08  0, 0, -65536      normal (0, 0, -1)
+0x14  170120976         2595.84, against a face at z = -2595
+0x18  2, 65536, 65536            vertex 2 at texel (1, 1)
+0x24  3, 16711680, 65536         vertex 3 at texel (255, 1)
+0x30  7, 16711680, 16711680      vertex 7 at texel (255, 255)
+0x3c  6, 65536, 16711680         vertex 6 at texel (1, 255)
```

Its six faces carry exactly the six axis normals, which is what a cube should
have and is a good sign that the offsets are right.

## The texture coordinates are texels, not fractions

Entry 5 read them as 16.16 in the 0.0-to-1.0 sense, because the cube's first
corner is 65536 and that looks like 1.0. It is not: it is texel 1.

Across both archives, all 111,166 corner coordinates are multiples of 65536 and
all of them lie between 0 and 16711680, which is 255 << 16. There are 256
distinct values and no others. The commonest pairs are the four corners of a
quad - (255,255), (0,255), (255,0), (0,0) - with (128, ...) next.

So the value is a whole texel in a 256-unit texture space. The textures are
64 x 64, so either the space repeats four times or the renderer scales; that is
not answered yet. A face mapped corner to corner uses 1 and 255 rather than 0
and 255, insetting by a texel, which is the usual dodge against sampling off
the edge.

## Normals, planes and winding

Measured over all 33,728 polygons in both archives:

```
normal is unit length, |n| - 1 < 0.01          33,648    80 are not
plane constant within 0.2% of dot(n, centroid) 32,373    the rest are
                                                          non-planar quads or
                                                          near zero
cross(v1-v0, v2-v1) points along the normal    33,639    2 do not
first three corners collinear, no winding          87
```

The two that disagree are in `KBOUT.BIN` and `KBOUT2.BIN` with cosines of -0.49
and -0.63, so both are far from planar and their stored normal is barely
meaningful. The winding rule has no real exceptions: a renderer can take
`cross(v1 - v0, v2 - v1)` as outward and cull on the stored normal.

Getting to the counts needed the test to be honest about degenerate cases.
The first version asserted zero reversed polygons and found 89, because a
strict `dot > 0` counts a zero cross product as reversed. Separating the 87
degenerate ones leaves 2, which is the real answer.

## What 0x18 is

`0x456571` walks a model expecting a specific node order, skipping one optional
node:

```
0x02  vertex list
0x17  optional
0x0d  material
0x18  polygon
```

So 0x18 is the polygon node that follows a material, which is what every leaf
model actually emits - 33,484 of them against 0x0e's 244. Its payload is
identical to 0x0e's on every measurement above. What distinguishes the two is
still unknown. The binary has `- Flat shading model` and
`- Gouraud shading model` strings, which is a plausible reason and not evidence
for one, so it stays in the Unknown section.

## Looking at it

`crates/hb/src/view.rs` is a debug view, not the port's renderer: perspective
projection, back-face culling on the stored normal, painter's algorithm, flat
Lambert shading.

```
hb view startup:ship.bin out.png   360 vertices, 490 polygons, 228 drawn, 262 back-facing
hb view krtank.bin out.png         122 vertices, 109 polygons,  51 drawn,  58 back-facing
hb view cube.bin out.png             8 vertices,   6 polygons,   3 drawn,   3 back-facing
```

The ship renders as a ship - fuselage, swept wings, engine nacelles - and the
tank as a tracked hull with two barrels. A cube shows exactly three of its six
faces. That is the check a table of offsets cannot give you: if the vertex
indices, the corner stride or the normal sign were wrong, the silhouette would
be noise rather than an aircraft.

It took two goes. The first render was a black frame: `world()` divides
coordinates by the model's half-extent and the projection was still using the
camera distance in model units, so the perspective divide cancelled the scale
and collapsed everything to a point. 228 faces reported drawn, zero pixels
changed - a reminder that a count of drawn polygons is not evidence of output.

**Still unknown:** what separates 0x18 from 0x0e. Whether the 256-unit texture
space repeats or scales over a 64 x 64 texture. The thirteen node types that
still have no semantics.
