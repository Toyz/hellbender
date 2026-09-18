---
title: The terrain grids
status: partial
covers: DATA\*.RAW, DATA\*.CLR, DATA\*.RA0-RA5, DATA\*.CL0-CL2
worklog: 4, 8, 10, 11, 12, 15
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
  +4  u8   shade[2]          .LTE  one per triangle half

box set A 0x00675fa0  18 bytes per cell
  +0  i16  bottom altitude   .RA0  byte << 7
  +2  i16  top altitude      .RA1  byte << 7
  +4  u16  texture[6]        .CL0
  +16 u8   shade             .LTE
  +17 u8   unknown

chambers  0x006bdfb0  12 bytes per cell
  +0  i16  floor altitude    .RA2  (byte - 255) << 7
  +2  i16  ceiling altitude  .RA3  (byte - 255) << 7
  +4  u16  texture[2]        .CL1
  +8  u8   shade[3]          .LTE
  +11 u8   unknown

box set B 0x006edfc0  18 bytes per cell
  +0  i16  bottom altitude   .RA4  (byte - 255) << 7
  +2  i16  top altitude      .RA5  (byte - 255) << 7
  +4  u16  texture[6]        .CL2
  +16 u8   shade             .LTE
  +17 u8   unknown
```

The box cell's 18 bytes are 2 + 2 + 12 for the altitudes and textures, which
fixes the texture count at six, and two more that the shading pass fills. The
chamber's 12 are 2 + 2 + 4, which fixes its texture count at two, and four more
for the same reason.

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

The origin is in the **middle** of that square, not at a corner: every course
point in every level lies between -511.3 and +511.8 units in x and z - see
[.CRS](courses.md). A cell index is the middle seven bits of a signed
coordinate, so cells 0 to 63 are the positive half and 64 to 127 the negative
one, and the wrap is what a two's-complement coordinate masked to seven bits
does on its own.

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

## The texture words are twelve bits of index and four of orientation

Every texture word in the terrain - the ground's `.CLR` value, a box's six
`.CL0`/`.CL2` slots, a chamber's two `.CL1` slots - has the same shape:

```
bits 0-11    index into the level's .TEX list
bits 12-15   UV orientation code
```

`0x413c20` resolves one as `textureTable[word & 0xfff]`, where the table is
1,024 entries of 24 bytes at `0x753cd0` - the same table the `.ANI` animated
textures register into, which is why an animated texture is addressed exactly
like a static one.

The data agrees and is the stronger evidence. Across all 26 levels and 425,984
ground cells, `word & 0xfff` is always a valid index into that level's texture
list. The raw `u16` is out of range 3,658 times, and those 3,658 are exactly
the cells with a non-zero orientation code.

The orientation code is consumed in one place, `0x413940`, which takes the
cell's four corners and permutes their texture coordinates in a table at
`0x59d36c`:

```
code >> 2      tested first, a two-bit selector
code & 2       swaps corner 0 with 1, and corner 3 with 2
code & 1       a further swap of the same shape
```

So it is a mirror-and-rotate on the cell's UVs. 231 texture indices appear with
more than one code, which is what you would expect of a tile placed at several
orientations, and the code does not correlate with cell parity, so it is
authored rather than derived. Ten of the sixteen codes appear in the ground
data: 0 through 6, 8, 10 and 12.

## The shading database, DATA\*.LTE

A fifth file, opened by the terrain loader itself at `0x413460` with
`sprintf("%s.lte")` against the `data` directory. It is 114,688 bytes, which is
seven per cell, and the loader scatters those seven into the four cell arrays
in this order:

```
1  ground[+4]       the two together are one little-endian value
2  ground[+5]
3  box A[+0x10]
4  chamber[+8]      the three together are one 24-bit value
5  chamber[+9]
6  chamber[+10]
7  box B[+0x10]
```

The ground's pair is **not** one byte per triangle half. Read as a
little-endian `u16` it spans 0 to 511 in every level, so it is nine bits: an
intensity in the low byte and one flag in bit 8. The intensity's ceiling is 255
everywhere and its floor is per level - 64 in `HOTH`, 96 in `ROID`, 160 in
`FLOAT` - which reads as that level's ambient minimum.

Rendering settles it. Treating the two bytes as separate per-half shades
produces a checkerboard of black over the whole map, because the second byte is
only ever 0 or 1 and a ramp index taken from it is always row 0 while one taken
from the first byte is usually row 15. Treating the pair as an intensity and
indexing the ramp at `(255 - intensity) >> 4` produces hillsides lit from one
direction, which is what a heightmap should look like.

It is a **cache**, not source data. When the file is absent the engine prints
`No .LTE file.  Shading database for the last time during loading.  Phew!` and
computes the same values at `0x41c5d0`, walking the grid with a light direction
held in four globals at `0x666f34`. All 26 levels ship one.

This is not the same format as `FOG\<stem>.LTE`, which is 4,096 bytes and is
the 16-row [colour ramp](colour-tables.md). The two files share an extension
and nothing else: line 17 of the [.LVL](lvl.md) names the `FOG\` one, and the
terrain loader opens the `DATA\` one without asking the manifest. 114,688 is a
multiple of 256, so a ramp parser that checks only for that reads the shading
database as a 448-row ramp and says nothing - which is what the old note in
`colour-tables.md` about "448 rows" was.

## Which half of a cell a point is in

`0x41b0b0` answers it with two comparisons on the position's fractions across
its cell. The fraction is `(p & 0x7ffff) * 0x10000 / 0x80000`, the low 19 bits
scaled to 16.

```
parity 0   fz < fx          {origin, +x, +x+z}   else {origin, +z, +x+z}
parity 1   fx + fz < 1.0    {origin, +x, +z}     else {+x, +x+z, +z}
```

The first test is the line `fx == fz`, the main diagonal; the second is
`fx + fz == 1`, the anti-diagonal. That is the same split the sample points in
`groundTriangleMidpoint` imply, arrived at by a different routine, so the
triangulation is transcribed twice over rather than inferred once.

## The surface normal

`groundTriangleInt` at `0x428ac0` calls `groundTriangleMidpoint` for a point on
the triangle, then dispatches on the layer to one of three routines - ground
`0x41b0b0`, chamber floor `0x41b3c0`, chamber ceiling `0x41b6d0` - for the
normal. Each builds a vector from two corner-altitude differences and hands it
to the normaliser at `0x42bb80`:

```
n = normalise( h(a) - h(b), 0x80000, h(c) - h(d) )
```

where `(a, b)` is the triangle's x-aligned edge and `(c, d)` its z-aligned one.
Every triangle in this scheme has exactly one of each; the diagonal edge is the
one left out. The two readable cases spell out:

```
parity 0, fz < fx    ( h(x,z) - h(x+1,z),  0x80000,  h(x+1,z) - h(x+1,z+1) )
parity 1, fx+fz < 1  ( h(x,z) - h(x+1,z),  0x80000,  h(x,z)   - h(x,z+1)   )
```

The y component is the cell size, so the vector is a height change per cell and
**+y is up**.

Between them, `groundTriangleMidpoint` and `groundTriangleInt` give a point and
a normal, which is a plane, which is the height anywhere in the triangle. That
is how the engine does terrain height and collision, and it is the whole of the
ground query.

## The layer parameter

`heightAtGrid`, `groundTriangleInt` and `groundTriangleMidpoint` all take a
layer and all reject anything but 0, 2 and 3. There is no layer 1.

```
0  ground            0x0073bcc0 + index * 6,  +0
2  chamber floor     0x006bdfb0 + index * 12, +0
3  chamber ceiling   0x006bdfb0 + index * 12, +2
```

The box sets are not reachable through these queries. `intersectingBoxSurface`
at `0x004294c0` is the one that takes them, and it accepts layers 1 and 4 and
nothing else:

```
1  box set A    0x00675fa0 + index * 18
4  box set B    0x006edfc0 + index * 18
```

It reads the bottom at +0 and the top at +2, shifts both left 8 like
`heightAtGrid` does, and builds the box's corners from `index << 19` and
`(index + 1) << 19`. So a ground box is an axis-aligned column spanning exactly
one cell, and those two altitudes fully determine it. The complete layer
numbering across the engine is therefore:

```
0  ground          heightAtGrid, groundTriangleInt, groundTriangleMidpoint
1  box set A       intersectingBoxSurface
2  chamber floor   heightAtGrid, groundTriangleInt, groundTriangleMidpoint
3  chamber ceiling heightAtGrid, groundTriangleInt, groundTriangleMidpoint
4  box set B       intersectingBoxSurface
```

## Which box texture is on which face

Measured, not transcribed. Take every box cell across eight levels whose four
side slots are three-of-a-kind plus one odd one out, and that has exactly one
boxless neighbour:

```
odd slot 0    z neighbour exposed 160    x neighbour exposed 2
odd slot 1    z 129                      x 4
odd slot 2    x 145                      z 0
odd slot 3    x 141                      z 0
```

So slots 0 and 1 are the two z-facing sides and 2 and 3 the two x-facing sides.
Slot 4 is the top - 80 distinct textures across `FLOAT`'s 1,142 boxes, against
18 for slot 5, the bottom, which a player rarely sees.

Which member of each pair faces which way is **not** settled. The same test
splits 81 against 77 for the z pair and 82 against 63 for the x pair, which is
noise. A stricter test over isolated two-cell runs gives 7 against 1 and 5
against 2 - suggestive, too small to rely on, and it disagrees with the one
case read by hand. It needs the terrain renderer, or a rendered level compared
against a screenshot.

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

What bit 8 of the ground shading word is. What the chamber's 24-bit shading
value decomposes into - it spans about 24,000 to 240,000, so it is not the same
shape as the ground's.

Which of the four orientation bits is which mirror and which rotation. The
corner argument order at `0x413940` would say, and it has not been pinned.

The one remaining spare byte in each box cell (+17) and in each chamber cell
(+11). Which member of each box side pair faces which way. Why `0xABB9` is not
`0xAAAB`. What sets the light direction at `0x666f34`, which is in
uninitialised data.
