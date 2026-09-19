---
title: The .BIN model and its MRGL nodes
status: partial
covers: MODELS\*.BIN
worklog: 5, 9, 16, 17, 35, 36
---

# The .BIN model and its MRGL nodes

A `.BIN` model is a flat stream of variable-length typed records - the engine
calls their type an MRGL type - terminated by a record of type 0. There is no
file header: the first record starts at byte 0.

Every record begins with a `u32` type. Everything is little-endian, and
integers are 32-bit signed unless stated.

## Loading

`HELLBEND.EXE:0x00473d20` reads the whole file into one allocation and does not
parse it:

```
length = podFileLength("models", name)
file   = open("models", name, "rb")
buffer = malloc(length)
fread(buffer, 1, length, file)     -> must return length, else "Model read hose"
fclose(file)
if (buffer[0] != 0x14 && buffer[0] != 0x20) fatal("Bad model!")
return buffer
```

The file *is* the memory image. Before any of that, at `0x00473d5a`, the loader
upper-cases the second character of the extension and compares it with `'T'`:
a `.TXT` model goes to a different parser at `0x0046cd60` and becomes a type
0x26 record instead.

## The size table

Records are variable length, so stepping over an unknown one needs a size
function. `0x00474360` is it: a bounds check against 0x26 and a jump through
the table at `0x0047442c`. Transcribed, with `n[k]` meaning the `u32` at byte
offset `k` of the record:

| type | size in bytes | name |
| --- | --- | --- |
| 0x00 | 4 | end of stream |
| 0x01 | 16 | |
| 0x02 | 12 + 12 * n[8] | vertex list |
| 0x03 | 12 + 4 * n[8] | |
| 0x04 | 12 + 8 * n[8] | vertex texels |
| 0x05 0x06 0x07 0x08 0x0f 0x15 0x19 0x1a 0x1b 0x21 | 24 + 4 * n[4] | indexed polygon (0x0f textured, 0x19 flat) |
| 0x09 | 32 | |
| 0x0a 0x0b | 8 | 0x0a sets the flat colour |
| 0x0c | 28 | |
| 0x0d | 24 | material |
| 0x0e 0x11 0x18 0x1e 0x22 | 24 + 12 * n[4] | polygon |
| 0x10 | 20 | |
| 0x12 0x14 | 8 | 0x14 starts a mesh; n[4] is its unit |
| 0x16 | 8 + 4 * n[4] | |
| 0x17 | 12 | |
| 0x1d | 28 + 32 * n[8] | flipbook material |
| 0x1f | 12 + 4 * n[8] | |
| 0x20 | 344 | group |
| 0x26 | 15,712 | animated model, built from a `.TXT` |

Types 0x13, 0x1c, 0x23, 0x24 and 0x25 reach the `"Bad MRGL type"` arm: holes in
the numbering, not records.

This table walks all 342 `.BIN` models in both archives so that the last record
ends exactly at end of file - `tools/mrgl.py check`.

## Records that are understood

### 0x14, mesh start

`+4` is the model's unit: model coordinates to one world unit. The bounds
routine `0x473ee0` scales every vertex by `2 * 0x7fffffff / unit` in 16.16
(`0x473ff4`), which is `vertex / unit` world units, and keeps the min and max
on each axis, the vertex average, and a radius - the length of the largest
absolute extent on each axis (`+0x24` of its 52-byte result). A placed actor
is drawn at its type's radius instead; the powerups, which have no type, are
drawn and picked up at the unit. The `f6*` powerups have a unit of 8,912, so
their 16,384-unit quad is 1.84 units to a side's half.

### 0x04, vertex texels

`+4` the first vertex, `+8` a count, then that many `(u, v)` pairs in 16.16
texels. The draw handler (`0x4567c0`) writes them beside the vertices in the
transformed-vertex array (`0x59d36c`, 36 bytes a vertex), so the indexed
polygons that follow take their texture coordinates from their vertices.

### 0x05-0x08, 0x0f, 0x15, 0x19-0x1b, 0x21, indexed polygons

One shape: `+4` the corner count, `+8` a 16.16 normal, `+0x14` the plane
constant, and from `+0x18` the vertex indices. Every draw handler starts with
the same back-face test against the eye in model space (`0x5b37d8`), skipped
when the normal is zero; they differ in how they fill.

- **0x0f** is textured from the vertex texels with a span routine
  (`0x4a5b1a`) that does not write texel 0 - the powerups and the other
  sprite-like models are a picture with holes on one quad. 52 polygons in 45
  models, all with a normal.
- **0x19** is flat: the normal's light (`0x48a6a0`) picks a shade in the
  colour set by the last 0x0a record - a ramp index into the tables at
  `0x50c4e8` (low) and `0x50c528` (high), or a palette index when negative
  (`0x458acd`). 215 polygons in 34 models, `FMBUNK.BIN` and `FDTRAINX.BIN`
  among them, every one after a colour of 0: the palette's first band, 0 to
  31. The port drew none of them until worklog 36.

### 0x1d, flipbook material

A material that changes with time (`0x458fd0`): `+8` the frame count, `+0xc`
the current frame, `+0x10` seconds a frame in 16.16, `+0x14` the time run so
far, `+0x18` a changed flag; then 32 bytes a frame, a texture name and at
`+0x10` a sound played when that frame comes round. The frame is
`time / period mod count`. The 40 in the archives are the powerups' spinning
pictures; `F6DAM1.BIN`'s is eight frames at 0.244 seconds each.

### 0x0a, flat colour

`+4` is the colour the flat polygons use (`0x457850` stores it at
`0x5b3888`). The bands, low and high index by colour 0 to 15:

```
 0  0x00-0x1f    4  0x60-0x7f    8  0xb0-0xbf   12  0xf0
 1  0x00-0x0f    5  0x80-0x8f    9  0xc0-0xcf   13  0xff
 2  0x20-0x3f    6  0x90-0x9f   10  0xd0-0xdf   14  0x00
 3  0x40-0x5f    7  0xa0-0xaf   11  0xe0-0xef   15  0xd8
```

The same colour fills the textured kinds when textures are turned off
(`0x5126e4`, `0x4585f9`).

The 0x17 record the port reads colours from does not set it - its handler
only advances a counter at `0x504280` - and its value is always the first
record's and looks like a count (`0x80` in `BMBEETLE.BIN`, `0x32` in
`FMBUNK.BIN`). Which makes the port's flat colour for the seven untextured
models a guess that is probably wrong.

### 0x02, vertex list

```
+0x00  u32  type = 0x02
+0x04  i32  ?
+0x08  i32  count
+0x0c  i32[3] * count      x, y, z per vertex
```

**Model space is normalised.** 235 of the 238 `.BIN` models in GAME.POD have a
maximum absolute vertex component of exactly 16,383 or 16,384, and none exceeds
16,384. So a coordinate is 2.14 fixed point spanning -1.0 to +1.0, and the
`scale` on a [placement](level-text.md) is the object's half-extent in world
units: a vertex becomes a 16.16 world offset as `(vertex * scale) >> 14`.

The three exceptions are two empty models and `CUBE.BIN`, whose components are
±2595 and ±2662 - a test model that was never run through the exporter's
normalisation, which is why it was such a legible first example.

### 0x0d, material

```
+0x00  u32      type = 0x0d
+0x04  i32      ?
+0x08  char[16] texture name, NUL-padded, e.g. "rustplat.raw"
```

A material applies to every polygon after it until the next one. Across both
archives every polygon is preceded either by another polygon or by a material,
so "the most recent material" is always defined once the first has appeared. A
run is four polygons long at the median and 168 at the longest; 3,013 runs in
all.

### 0x17, flat colour

12 bytes. A zero `i32` at +4 and a palette index at +8.

```
+0x00  u32  type = 0x17
+0x04  i32  0 in all 270
+0x08  i32  palette index
```

267 of the 270 in both archives are between 9 and 239, which is exactly the
[shadeable palette range](colour-tables.md) - not one falls in the reserved 240
to 255. The other three are 270, 360 and 482, whose low bytes are 14, 104 and
226 and whose bit 8 is set, so the field reads as an index with a flag above
it, the same shape as a terrain texture word.

It binds to polygons the same way a material does, and an untextured polygon
uses it instead. Seven models are untextured throughout and between them
account for all 1,030 polygons with no material before them:

```
GLOBE.BIN 576    IRIS1.BIN 168    IRIS4.BIN 168    SHELL.BIN 32
JAW1.BIN   30    FANBODY.BIN 28   JAW2.BIN   28
```

Four of those - `FANBODY`, `JAW1`, `JAW2` and `SHELL`, 118 polygons - have no
colour node before their polygons either, so **nothing in the stream says what
colour they are**. They begin `0x14, 0x02, polygon, polygon, ...` with nothing
between. A renderer has to decide; this port skips them rather than invent a
colour.

### 0x0e and its variants, polygon

Types 0x0e, 0x11, 0x18, 0x1e and 0x22 share this layout. 0x18 is 33,484 of the
33,728 polygons in the two archives and 0x0e is 244; the other three do not
appear at all.

```
+0x00  u32  type
+0x04  i32  corner count            always 3 or 4
+0x08  i32  normal.x                16.16, unit length
+0x0c  i32  normal.y
+0x10  i32  normal.z
+0x14  i32  plane constant          16.16, signed
+0x18  per corner, 12 bytes:
         i32  vertex index
         i32  u                     texel << 16, 0 to 255
         i32  v                     texel << 16, 0 to 255
```

**Normal.** Unit length in 33,648 of 33,728 polygons, measured as
`|n| - 1 < 0.01` with the components read as 16.16. The 80 that are not are
spread across a handful of models.

**Plane constant.** The plane is `dot(normal, p) == plane`. It agrees with
`dot(normal, centroid of the face)` to within 0.2% for 32,373 of the 33,728;
the rest are non-planar quads and faces whose constant is near zero, where a
relative test has nothing to bite on.

**Winding.** `cross(v1 - v0, v2 - v1)` points along the stored normal. 87
polygons have collinear first corners and no winding to test. Of the remaining
33,641, 33,639 agree and 2 disagree - both in `KBOUT.BIN` and `KBOUT2.BIN`, and
both so far from planar that the stored normal is barely meaningful. So the
rule has no real exceptions.

**Texture coordinates.** Not 16.16 in the 0.0-to-1.0 sense. The value is a
whole texel shifted left 16, and every one of the 111,166 corners in both
archives has both coordinates in 0 to 255 with a zero fractional part. So the
texture space is 256 units across whatever the texture's real size is - the
textures themselves are 64 x 64, so a full-face mapping repeats four times, or
the renderer scales. A face mapped corner to corner uses 1 and 255 rather than
0 and 255, insetting by a texel.

### 0x20, group

344 bytes, holding child model names rather than geometry. Every byte is
accounted for.

```
+0x000  u32      type = 0x20
+0x004  i32      ?
+0x008  i32      child count
+0x00c  i32      ?
+0x010  i32      ?
+0x014  ptr      runtime child pointer, zero in the file
+0x018  char[16] * 16   child filenames, NUL-padded
+0x118  ptr * 16        runtime child pointers, zero in the file
                        0x118 + 16 * 4 = 0x158 = 344
```

So a group holds at most sixteen children. The pointer array's offset is
confirmed by the free routine at `0x00473e70`, which recurses into
`[node+0x14]` and into `[node+0x118 + 4*i]` for `i < node[8]`; the name slot
stride of 16 is confirmed by `ALIENSH.BIN`, where `aliensh1.bin` begins at
0x18 and `aliensh2.bin` at 0x28. In that file the eight unused name slots and
all sixteen pointer slots - 0x98 through 0x158 - are zero.

The only group a level places is the SAM site's `SamSite1.Bin`, whose eight
children run `Sam01.Bin` to `Sam05.Bin` and back to `Sam02.Bin` - the order of
a ping-pong animation. 97 of the 7,606 placements use it. Where the engine
needs a group's geometry for the type - its bounding box, at `0x473ee0` - it
recurses into the name at +0x18, the first child. How it advances through the
children when drawing has not been read; the port draws the first.

## Worked example

`MODELS\CUBE.BIN`, 576 bytes:

```
0x000000  type 0x14 meshStart   size 8
0x000008  type 0x02 vertexList  size 108   count = 8
0x000074  type 0x0d material    size 24    name = "rustplat.raw"
0x00008c  type 0x0e polygon     size 72    count = 4
0x0000d4  type 0x0e polygon     size 72    count = 4
0x00011c  type 0x0e polygon     size 72    count = 4
0x000164  type 0x0e polygon     size 72    count = 4
0x0001ac  type 0x0e polygon     size 72    count = 4
0x0001f4  type 0x0e polygon     size 72    count = 4
0x00023c  type 0x00 end         size 4
```

Eight vertices, one texture, six quads - the vertex components are `±2595` in
x and z and `±2662` in y. Its first polygon reads:

```
+0x00  0x0000000e   type
+0x04  4            corners
+0x08  0            normal.x
+0x0c  0            normal.y
+0x10  -65536       normal.z    = (0, 0, -1)
+0x14  170120976    plane       = 2595.84, against z = -2595 on the face
+0x18  2, 65536, 65536          vertex 2 at texel (1, 1)
+0x24  3, 16711680, 65536       vertex 3 at texel (255, 1)
+0x30  7, 16711680, 16711680    vertex 7 at texel (255, 255)
+0x3c  6, 65536, 16711680       vertex 6 at texel (1, 255)
```

The six faces carry exactly the six axis normals, and each maps its texture
corner to corner.

`MODELS\ALIENSH.BIN` is a group: type 0x20, child count 8, then `aliensh1.bin`
through `aliensh8.bin` in the first eight of its sixteen 16-byte name slots
from +0x18. The file is 348 bytes - the 344-byte record plus the 4-byte end
record.

## Census

Across all 342 models:

```
0x18  33,484     0x0d  3,265     0x00    342     0x02    336
0x14     336     0x17    270     0x0e    244     0x19    215
0x0f      52     0x04     45     0x05     41     0x1d     40
0x0a      37     0x06     24     0x20      6     0x0c      4
0x12       4     0x1f      4
```

## The node order a mesh uses

`0x456571` walks a model and expects a specific sequence, skipping one optional
node:

```
0x02  vertex list
0x17  optional, 12 bytes
0x0d  material
0x18  polygon
```

which is the order `CUBE.BIN` and every other leaf model uses. The walker
rounds each size down to a multiple of 4 with `and eax, 0xfffffffc` before
stepping, although every size in the table is already a multiple of 4.

## Unknown

What distinguishes 0x18 from 0x0e. Both have the same size formula and, as far
as every measurement goes, the same payload; the binary has separate flat and
Gouraud shading paths, which is a plausible reason and not evidence for one.

What colour the 118 polygons in `FANBODY`, `JAW1`, `JAW2` and `SHELL` are, since
the stream does not say.

Record types that appear in the data with no semantics yet: 0x0c, 0x12 and
0x1f, and the fill modes of indexed polygons 0x05 and 0x06; and the `i32` at
+4 of the vertex list and material records.

Whether the 256-unit texture space is a repeat or a scale, given the textures
are 64 x 64. This port scales - see
[the renderer](../engine/rendering.md).
