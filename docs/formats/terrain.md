---
title: The terrain grids
status: partial
covers: DATA\*.RAW, DATA\*.CLR, DATA\*.RA0-RA5, DATA\*.CL0-CL2
worklog: 4, 8, 10, 11, 12, 15, 20, 29
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
occupy -32,640 to 0. An empty cell of box set B has bottom and top both at
byte 0, -32,640 - in all 15,300 to 16,300 empty cells of each of `FLOAT`,
`HOTH`, `IOWAH`, `MORBOS`, `ROID` and `SHIP` - so a box is told by its bottom
and top differing, never by either being zero. Across
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

### How a texture lies on a cell

Both of the engine's texture-coordinate setups hand the permuting routine
`0x413940` a quad of four vertices `a b c d`, already holding coordinates:

```
corner   ground cell (0x413c20)   box face (0x414f60)   u, v
a        (x, z)                   see the box table     lo, hi
b        (x+1, z)                                       hi, hi
c        (x+1, z+1)                                     hi, lo
d        (x, z+1)                                       lo, lo
```

`lo` and `hi` are `0x20000` and `0xfe0000` - 2.0 and 254.0 in the 256-unit
texture space, half a texel in from the edges of a 64-texel texture - or
`0x4000` and `0xffc000` when `[0x5d29e0]` is 256. The ground's corner order is
read twice over: the four vertices are copied from the projected-grid array at
`0x526038`, 22 entries wide, at `(gx, gz)`, `(gx+1, gz)`, `(gx+1, gz+1)`,
`(gx, gz+1)` (`0x414bf0` to `0x414c8e`), and each takes the shade of the cell
at that corner (`0x414e05`).

So **u runs with x and v runs against z**: the texture's first row lies along
the cell's far z edge. Laying every untouched tile of a level that way makes
neighbours meet far better than any of the other seven ways a square can lie -
a mean texel difference across shared edges of 46.9 in `HOTH` against 77.2 for
the next best, 73.1 against 91.8 in `KREASH`, 53.7 against 78.6 in `JURASIC`
(`untouched_tiles_meet_best_the_engines_way_round`).

`0x413940` then applies the word's orientation code:

```
r = code >> 2     if non-zero, a b c d take the coordinates corners
                  r, r+1, r+2, r+3 (mod 4) had - a quarter turn per step,
                  since a b c d run round the square
code & 2          swaps a with b and d with c: mirrors u
code & 1          swaps a with d and b with c: mirrors v
```

The sixteen codes make the eight symmetries of a square, each twice. Ten appear
in the ground data: 0 through 6, 8, 10 and 12. `TextureRef::corner_uvs`
transcribes it and its tests check each case.

Whether the turned tiles meet their neighbours is a weaker check than it
sounds, because most of them are noise textures turned to break up repetition,
and any way round fits a noise texture about as well. Averaged over edge
segments, the code's own reading is first or second of the eight in 59 per cent
of 2,031 turned cells against a chance 25; code 4 agrees best, while codes 8
and 12 are often matched as well or better by a mirror. A hand check of a
transition tile - `KREASH`'s `KMDRIBDT` at (100, 72), grey along its first row
and brown along its last, with brown to its -x side and grey to its +x -
comes out right only under the code's reading. The disassembly is unambiguous,
and the port follows it.

## The shading database, DATA\*.LTE

A fifth file, opened by the terrain loader itself at `0x413460` with
`sprintf("%s.lte")` against the `data` directory. It is 114,688 bytes, which is
seven per cell, and the loader scatters those seven into the four cell arrays
in this order:

```
1  ground[+4]       the two together are one little-endian value
2  ground[+5]
3  box A[+0x10]
4  chamber[+8]      the floor's intensity
5  chamber[+9]      the ceiling's intensity
6  chamber[+10]     bit 0: the floor takes the ambient; bit 1: the ceiling
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

**The value belongs to a grid point, not a cell.** The ground drawer gives each
of a cell's four vertices the value of the cell whose origin that vertex is
(`0x414e05` to `0x414ea7`): `byte << 8`, or, when bit 8 is set, the constant at
`[0x525c5c]`. That constant is the level's ambient - the `.LVL`'s
[line 19](lvl.md), loaded to `0x666f40` and copied there at `0x419fda` - so bit
8 marks a point in shadow. The intensity's floor per level is the same ambient:
`HOTH`'s 16384 is its floor of 64, `FLOAT`'s 40960 its 160, `ROID`'s 24576 its
96. The ground is Gouraud shaded between the four.

**A box's byte is eight shadow bits.** The shading computation clears box A's
byte (`0x41c8cd`) and, for each of the box's corners, casts 48 units
(`0x300000`) toward the light and tests the segment against the terrain
(`0x413580`), setting the corner's bit if something is in the way. `15`, the
commonest value in `HOTH`, is the four bottom corners shadowed. Bit n is box
corner n, bottom corners 0-3 then top 4-7, each in the ground's order - the
drawer tests them in that order (`0x41572a` to `0x4157ed`) and asks for the
lamps' light at the corners in the same order (`0x415825`). A shadowed corner
gets the ambient. A lit one is marked, and each face gives its marked corners
its own light from the sun: `0x48a6a0` of the face's normal, the ambient plus
the rest of the way to full by how squarely the face looks into the light
(`0x416563` and five like it). So a box's faces are shaded by which way they
look, as a model's polygons are. The drawer also casts again for points that
`0x415120` adds to a face (`0x415e35`), which is not read.

**A chamber's three bytes are two intensities and two flags.** Read as one
number they span about 24,000 to 240,000, which is what a floor byte, a
ceiling byte and a flags byte of 0 to 3 give. The chamber drawers light each
corner from its own cell's record as the ground drawer does: the floor from
`+8`, or the ambient if bit 0 of `+10` is set (`0x418fe4`); the ceiling from
`+9`, or the ambient if bit 1 is (`0x41944c`). The computation lights the
chambers from a second direction and ambient, the `.LVL`'s lines 20 and 21
(`0x41d161`).

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

## The height anywhere, not just at a corner

`heightAtGrid` answers only at a cell corner. For a position inside a cell the
engine uses `groundTriangleInt`, which produces a point on the containing
triangle and the triangle's normal - and a point plus a normal is a plane,
which solves for the height at any position in that triangle.

A port can reach the same plane from the three corner altitudes directly, which
is shorter and avoids reproducing the engine's normalisation rounding. Either
way the triangle has to be the one the
[half test](#which-half-of-a-cell-a-point-is-in) picks, or the answer is taken
from the wrong plane on half the grid.

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

The box drawer draws each face from its own call into `0x414f60`, with the
slot's word from the 18-byte box cell, the face's normal for the back-face test
at `0x456650`, and the neighbouring box it checks for a face that cannot be
seen - the face is skipped if that neighbour's box spans it top to bottom:

```
slot  word  site      normal       neighbour   quad
 0    +4    0x415cc0  (0, 0, -1)   z - 1       0 1 5 4
 1    +6    0x416773  (0, 0, +1)   z + 1       2 3 7 6
 2    +8    0x417222  (+1, 0, 0)   x + 1       1 2 6 5
 3    +10   0x417cb6  (-1, 0, 0)   x - 1       3 0 4 7
 4    +12   0x4186de  (0, +1, 0)   -           12 13 14 15
 5    +14   0x4188a1  (0, -1, 0)   -           8 9 10 11
```

Box vertices 0-3 are the bottom corners and 4-7 the top, each in the ground's
order (x, z), (x+1, z), (x+1, z+1), (x, z+1); 8-15 are copies of 0-7 made by a
`rep movs` of 0x48 dwords after each face. So **slot 0 faces -z, 1 faces +z, 2
faces +x and 3 faces -x**, 4 is the top and 5 the bottom, and a side's texture
stands upright with its first row along the top edge.

That settles what a measurement had only half settled. Taking every box cell
across eight levels whose four side slots are three-of-a-kind plus one odd one
out, and that has exactly one boxless neighbour, had paired the slots with
axes:

```
odd slot 0    z neighbour exposed 160    x neighbour exposed 2
odd slot 1    z 129                      x 4
odd slot 2    x 145                      z 0
odd slot 3    x 141                      z 0
```

but split 81 against 77 and 82 against 63 on the signs. Slot 4 carries 80
distinct textures across `FLOAT`'s 1,142 boxes, against 18 for slot 5.

The ground drawer also skips a cell whose four corners all lie within the span
of that cell's box A (`0x414d43`): ground inside a box is never drawn.

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

The one remaining spare byte in each box cell (+17) and in each chamber cell
(+11). Why `0xABB9` is not `0xAAAB`. What the `[0x5d29e0] == 256` case that
narrows the texture inset is.
