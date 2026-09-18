---
title: The terrain grids
status: partial
covers: DATA\*.RAW, DATA\*.CLR, DATA\*.RA0-RA5, DATA\*.CL0-CL2
worklog: 4, 8
---

# The terrain grids

A Hellbender level's world is a 128 x 128 grid of cells. Thirteen files under
`DATA\` describe it: one base surface, two sets of extruded boxes, and one set
of chambers. Every file is a bare array with no header, in row-major order,
128 x 128 entries.

## The files

| file | bytes | per cell | what |
| --- | ---: | --- | --- |
| `.RAW` | 16,384 | `u8` | ground altitude |
| `.CLR` | 32,768 | `u16` | ground colour / texture |
| `.RA0` | 16,384 | `u8` | box set A, bottom altitude |
| `.RA1` | 16,384 | `u8` | box set A, top altitude |
| `.CL0` | 196,608 | `u16[6]` | box set A textures |
| `.RA2` | 16,384 | `u8` | chamber floor altitude |
| `.RA3` | 16,384 | `u8` | chamber ceiling altitude |
| `.CL1` | 65,536 | `u16[2]` | chamber textures |
| `.RA4` | 16,384 | `u8` | box set B, bottom altitude |
| `.RA5` | 16,384 | `u8` | box set B, top altitude |
| `.CL2` | 196,608 | `u16[6]` | box set B textures |

All 26 levels ship all thirteen at exactly these sizes.

A *ground box* is an extruded cell: a block standing between a bottom and a top
altitude, with six texture slots. A *chamber* is its inverse, a roofed void
between a floor and a ceiling, with two texture slots. Two box sets plus one
chamber set per cell is what lets a level stack structures and tunnel under
them.

## How the loader reads them

`HELLBEND.EXE:0x00412c00`. It builds each name with `sprintf` from the level's
stem, and the format string sits immediately before the diagnostic that fires
if the open fails - which is how each extension was matched to its role:

```
0x005013bc  "%s.ra0"   "Unable to read ground box layer 0"
0x005013f4  "%s.ra1"   "Unable to read ground box layer 1"
0x00501434  "%s.cl0"   "Unable to read ground box texture"
0x0050146c  "%s.ra2"   "Unable to read chamber ground layer"
0x005014a4  "%s.ra3"   "Unable to read chamber ceiling layer"
0x005014e0  "%s.cl1"   "Unable to read chamber texture"
0x0050151c  "%s.ra4"   "Unable to read ground box layer 0"
0x00501554  "%s.ra5"   "Unable to read ground box layer 1"
0x0050158c  "%s.cl2"   "Unable to read ground box texture"
```

## The in-memory cells

The loops give the strides and the base addresses directly. There are four
arrays, not two.

```
ground   0x0073bcc0   6 bytes per cell
  +0  i16  altitude          .RAW  byte << 7
  +2  u16  colour            .CLR  u16, unscaled
  +4  u16  unknown

box set A 0x00675fa0  18 bytes per cell
  +0  i16  bottom altitude   .RA0  byte << 7
  +2  i16  top altitude      .RA1  byte << 7
  +4  u16  texture[6]        .CL0

chambers  0x006bdfb0  12 bytes per cell
  +0  i16  floor altitude    .RA2  (byte - 255) << 7
  +2  i16  ceiling altitude  .RA3  (byte - 255) << 7
  +4  u16  texture[2]        .CL1
  +8  4 bytes unknown

box set B 0x006edfc0  18 bytes per cell
  +0  i16  bottom altitude   .RA4  (byte - 255) << 7
  +2  i16  top altitude      .RA5  (byte - 255) << 7
  +4  u16  texture[6]        .CL2
```

18 is 2 + 2 + 12, which is what fixes the box texture count at six; 12 is
2 + 2 + 4 + 4, which fixes the chamber count at two and leaves four bytes over.

The arrays sit end to end with 16 bytes between them:
`0x675fa0 + 16384 * 18 = 0x6bdfa0`, and the chambers start at `0x6bdfb0`;
`0x6bdfb0 + 16384 * 12 = 0x6edfb0`, and box set B starts at `0x6edfc0`.

## Altitudes are bytes scaled by 128, in two directions

Every altitude pass shifts the byte left by 7, so a level has 256 heights and a
vertical step of 128 world units. But two of the six layers first subtract 255:

```
0x412f47  shl ax, 7                 .ra0, box A bottom
0x412fba  shl ax, 7                 .ra1, box A top
0x413104  sub ax, 0xff ; shl ax, 7  .ra2, chamber floor
0x41317b  sub ax, 0xff ; shl ax, 7  .ra3, chamber ceiling
0x4132c4  sub ax, 0xff ; shl ax, 7  .ra4, box B bottom
0x41333b  sub ax, 0xff ; shl ax, 7  .ra5, box B top
```

So the ground and box set A occupy 0 to 32,640 and the chambers and box set B
occupy -32,640 to 0. The byte that means "nothing here" is 0 for the upward
layers and 255 for the downward ones, and both resolve to altitude zero. Across
`FLOAT`, `HOTH`, `ROID` and `SHIP` no downward layer produces a positive value
and no upward layer a negative one.

Reading a downward layer with the upward scaling puts chambers and half the
world's structures in the wrong hemisphere, and the file itself gives no hint -
it is a plain array of bytes either way.

## A cell is 8.0 units, and the world wraps

`groundTriangleMidpoint` at `0x428900` turns a cell index into a world
coordinate with

```asm
shl esi, 25
sar esi, 6
```

which is `(index & 0x7f) << 19`. So a cell is `0x80000` world units - 8.0 in
16.16 - and the grid **wraps** at 128 rather than clamping. `heightAtGrid`
masks the same way, with `and eax, 0x7f`. The world is 1024.0 units square and
a level has no edge.

`heightAtGrid` returns the stored altitude shifted left by 8, so a query
returns the byte scaled by 2^15: half a world unit per step, 127.5 units from
the bottom of the range to the top.

## A cell is two triangles, and the diagonal alternates

`groundTriangleMidpoint` computes a sample point for one half of one cell. It
tests `(x ^ z) & 1` and the caller's half, and adds one of two fractions of the
cell size to each axis:

```
(x ^ z) & 1 == 1, half 0     (0xABB9, 0xABB9)      approx (2/3, 2/3)
(x ^ z) & 1 == 1, half 1     (0x5555, 0x5555)      approx (1/3, 1/3)
(x ^ z) & 1 == 0, half 0     (0x5555, 0xABB9)      approx (1/3, 2/3)
(x ^ z) & 1 == 0, half 1     (0xABB9, 0x5555)      approx (2/3, 1/3)
```

Each fraction is multiplied by `0x80000` and shifted right 16.

Those are triangle centroids, and they say which way the diagonal runs:

- `(x ^ z) & 1 == 1`: centroids at (1/3, 1/3) and (2/3, 2/3), so the diagonal
  joins (1,0) to (0,1) - the anti-diagonal.
- `(x ^ z) & 1 == 0`: centroids at (1/3, 2/3) and (2/3, 1/3), so the diagonal
  joins (0,0) to (1,1) - the main diagonal.

The split therefore alternates across the grid like a checkerboard, and no two
edge-sharing cells split the same way. Nothing in any file records this - it is
derived from the cell indices alone, so a port that picks one uniform diagonal
gets every other cell wrong.

`0x5555` is 0.333328, a third to within a rounding. `0xABB9` is 0.670791, which
is **not** two thirds - that would be `0xAAAB`. The 0.4%-of-a-cell difference
changes which triangle a borderline query lands in, so it is reproduced rather
than corrected.

## The layer parameter

`heightAtGrid`, `groundTriangleInt` and `groundTriangleMidpoint` all take a
layer and all reject anything but 0, 2 and 3. There is no layer 1.

```
0  ground            0x0073bcc0 + index * 6,  +0
2  chamber floor     0x006bdfb0 + index * 12, +0
3  chamber ceiling   0x006bdfb0 + index * 12, +2
```

The box sets are not reachable through these queries;
`intersectingBoxSurface` at `0x0042953c` is the one that takes them, and it has
not been read yet.

## Both colour and texture files have a narrow and a wide form

The loader branches on file length:

- `.CLR`: if the file is 0x4000 (16,384) it reads one byte per cell, otherwise
  two. Every shipped `.CLR` is 32,768, so the `u16` path is the live one.
- `.CL0`: if the file is 0x18000 (98,304) it reads six bytes per cell,
  otherwise six `u16`. Every shipped `.CL0` is 196,608, so again the wide path.

The narrow forms are supported and unused. A port needs them only to load
third-party data.

## Notes

The `.CLR` values in `FLOAT.CLR` - `0x40df`, `0x004d`, `0x004e`, `0x004f`,
`0x0050`, `0xc0cf` - have a small, often-zero high byte and a byte-sized low
part. That reads as an index into the level's [texture list](lvl.md) plus flags
in the high bits, not as a colour. It is not confirmed.

## Unknown

The third `u16` of the ground cell, and the four spare bytes of the chamber
cell. The meaning of the `.CLR` high byte. Which of the six box textures is
which face - `intersectingBoxSurface` is the routine that would say. Why
`0xABB9` is not `0xAAAB`.
