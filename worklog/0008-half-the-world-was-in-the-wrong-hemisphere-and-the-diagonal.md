---
number: 8
title: Half the world was in the wrong hemisphere, and the diagonal alternates
date: 2026-09-17
area: world, decomp, port
files: crates/hb-world, crates/hb-formats/src/terrain.rs, docs/formats/terrain.md
---

# 8. Half the world was in the wrong hemisphere, and the diagonal alternates

[4](0004-the-terrain-is-seven-128x128-grids-and-the-loader-names-them.md)
recorded that every altitude pass in the terrain loader does `shl ax, 7` on the
byte it read. That is true of two layers and wrong about four. Reading the rest
of the loader, from `0x413030` to `0x4133c0`:

```
0x412f47  shl ax, 7                 .ra0  box A bottom
0x412fba  shl ax, 7                 .ra1  box A top
0x413104  sub ax, 0xff ; shl ax, 7  .ra2  chamber floor
0x41317b  sub ax, 0xff ; shl ax, 7  .ra3  chamber ceiling
0x4132c4  sub ax, 0xff ; shl ax, 7  .ra4  box B bottom
0x41333b  sub ax, 0xff ; shl ax, 7  .ra5  box B top
```

`sub ax, 0xff` before the shift makes the value `(byte - 255) << 7`, which
spans -32,640 to 0. So the ground and box set A live at or above zero and the
chambers and box set B live at or below it. Byte 0 is "nothing here" for the
upward layers and byte 255 is "nothing here" for the downward ones; both mean
altitude zero.

The data agrees without exception. Across `FLOAT`, `HOTH`, `ROID` and `SHIP`,
no downward layer yields a positive value and no upward layer a negative one,
and `every_level_loads_its_thirteen_terrain_grids` now asserts that over all 26.

This was invisible from the files. Six identical 16,384-byte arrays of bytes,
and three of them mean the opposite of the other three. `ROID`'s chamber floor
is byte 0 in all 16,384 cells - under the wrong scaling that reads as a floor
at zero everywhere, and under the right one it is a floor at -32,640, which is
what an asteroid field with nothing underneath should look like.

## Four arrays, not two

Entry 4 found two. There are four, laid end to end with sixteen bytes between
them:

```
ground    0x0073bcc0    6 bytes per cell
box set A 0x00675fa0   18 bytes per cell
chambers  0x006bdfb0   12 bytes per cell
box set B 0x006edfc0   18 bytes per cell
```

`0x675fa0 + 16384 * 18 = 0x6bdfa0` and the chambers begin at `0x6bdfb0`;
`0x6bdfb0 + 16384 * 12 = 0x6edfb0` and box set B begins at `0x6edfc0`. The
chamber cell is 12 bytes, which is 2 + 2 for the altitudes, 4 for its two
textures, and four bytes left over that nothing in the loader writes.

## A cell is 8.0 units and the world wraps

`groundTriangleMidpoint` at `0x428900` converts a cell index to a world
coordinate with

```asm
shl esi, 25
sar esi, 6
```

which is `(index & 0x7f) << 19`. Two facts fall out at once. A cell is
`0x80000` world units, 8.0 in 16.16, so the world is 1024.0 units square. And
the mask means the grid **wraps** - `heightAtGrid` does the same thing with
`and eax, 0x7f`. A Hellbender level is a torus and has no edge.

That makes the other fixed-point numbers legible. The `.NAV` position
`14942208,491520,9699328` is (228.0, 7.5, 148.0), comfortably inside a 1024
square. `heightAtGrid` returns the stored altitude shifted left by 8, so a
height query returns half a world unit per altitude step and 127.5 units from
the bottom of the range to the top.

## The diagonal alternates like a checkerboard

`groundTriangleMidpoint` computes the sample point of one half of one cell. It
tests `(x ^ z) & 1`, and the four cases add different fractions of the cell
size to the two axes:

```
parity 1, half 0    (0xABB9, 0xABB9)
parity 1, half 1    (0x5555, 0x5555)
parity 0, half 0    (0x5555, 0xABB9)
parity 0, half 1    (0xABB9, 0x5555)
```

Those are centroids, and a centroid names its triangle. (1/3, 1/3) and
(2/3, 2/3) are the two halves either side of the line from (1,0) to (0,1);
(1/3, 2/3) and (2/3, 1/3) are the halves either side of the line from (0,0) to
(1,1). So:

```
(x ^ z) & 1 == 1    anti-diagonal, joining (1,0) to (0,1)
(x ^ z) & 1 == 0    main diagonal, joining (0,0) to (1,1)
```

The split alternates across the grid like a checkerboard and no two
edge-sharing cells split the same way. Nothing in any file records it - it is
computed from the cell indices - so a port that picks one uniform diagonal
gets half the world's triangles wrong, and gets them wrong in a way that only
shows up as height queries being slightly off in half the cells.

I had the parity backwards on the first pass and the test caught it: I wrote
the corner sets down from the sample points, then asserted that each corner
set's centroid is the sample point the engine computes, and half of them were
not. The corner sets in `hb-world` are a derivation from the constants, not a
transcription, and `every_sample_point_lands_inside_its_own_triangle` is what
holds the derivation honest.

## 0xABB9 is not two thirds

`0x5555` is 0.333328, a third to within a rounding. `0xABB9` is 0.670791. Two
thirds is `0xAAAB`. The difference is 0.4% of a cell, which is enough to change
which triangle a query on the diagonal lands in, so `hb-world` reproduces the
constant rather than correcting it. Why it is that value is unknown - it is not
a rounding of 2/3 in any obvious precision, and it is not 1 minus `0x5555`,
which would be `0xAAAB` as well.

## The layer parameter has a hole

`heightAtGrid`, `groundTriangleInt` and `groundTriangleMidpoint` all take a
layer and all three switch on 0, 2 and 3 and fall through to a fatal error
otherwise. There is no layer 1:

```
0  ground           0x0073bcc0 + index * 6,  +0
2  chamber floor    0x006bdfb0 + index * 12, +0
3  chamber ceiling  0x006bdfb0 + index * 12, +2
```

The box sets are not reachable through these queries at all. Whatever reads
them is `intersectingBoxSurface` at `0x0042953c`, which takes its own layer
argument and has not been read yet.

## The port

`crates/hb-world` is new and holds the cell geometry: wrapping indices, the
cell-to-world conversion, the parity rule, the sample points and the corner
sets, with five tests that need no game data because they are about arithmetic
transcribed from the binary. `hb-formats::terrain` grew a `Scale` enum so an
altitude layer carries which direction it was read in, and `Altitudes` is now
`i16` rather than `u16`, which the compiler made unavoidable the moment the
downward layers went in.

**Still unknown:** `intersectingBoxSurface`, which would say which of the six
box textures is which face. The ground cell's third `u16` and the chamber
cell's four spare bytes. The `.CLR` high byte.
