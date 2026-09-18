---
number: 10
title: The ground query closes, and a level that looks like its own name
date: 2026-09-17
area: world, decomp, port, render
files: crates/hb-world/src/grid.rs, crates/hb/src/view.rs, docs/formats/terrain.md
---

# 10. The ground query closes, and a level that looks like its own name

`groundTriangleInt` at `0x428ac0` turns out to be two things bolted together.
It calls `groundTriangleMidpoint` for a point on the triangle, then dispatches
on the layer to one of three routines for a normal - ground `0x41b0b0`, chamber
floor `0x41b3c0`, chamber ceiling `0x41b6d0`. A point and a normal is a plane,
and a plane is the height anywhere in the triangle. That is the whole ground
query, and it is now readable.

## The half test, transcribed

`0x41b0b0` opens by reducing the world position to a cell and a pair of
fractions:

```asm
eax = (x & 0x7ffff) * 0x10000 / 0x80000     ; fx
esi = (x & 0x3f80000) >> 19                 ; cell x
```

then branches:

```
parity 0    cmp fz, fx        fz < fx        one half, else the other
parity 1    edx = 1.0 - fx
            cmp edx, fz       fx + fz < 1    one half, else the other
```

The first line is `fx == fz`, the main diagonal. The second is `fx + fz == 1`,
the anti-diagonal. Which is exactly the split
[8](0008-half-the-world-was-in-the-wrong-hemisphere-and-the-diagonal-a.md)
derived from `groundTriangleMidpoint`'s sample constants - arrived at by a
different routine reading different data. The triangulation is transcribed
twice over now rather than inferred once, and the two transcriptions agree.

Better: the corner triples fall out directly. Each branch reads three
altitudes, and they are the three corners of the triangle:

```
parity 0, fz < fx      h(x,z), h(x+1,z), h(x+1,z+1)     {origin, +x, +x+z}
parity 1, fx + fz < 1  h(x,z), h(x+1,z), h(x,z+1)       {origin, +x, +z}
```

Both match what `hb-world` already had from the centroids. The corner sets stop
being a derivation.

## The normal is a height gradient

Each case builds a vector from two corner-altitude differences and hands it to
the normaliser at `0x42bb80`:

```
n = normalise( h(a) - h(b), 0x80000, h(c) - h(d) )
```

`(a,b)` is the triangle's x-aligned edge and `(c,d)` its z-aligned one. Every
triangle in this scheme has exactly one of each - the diagonal is the edge left
out - and `every_triangle_has_one_edge_along_each_axis` now holds that.

The y component is the cell size, `0x80000`, so the vector is a height change
per cell, and **+y is up**. That is the first hard statement about the world's
orientation rather than an assumption.

## intersectingBoxSurface, and the layer numbering closes

`0x004294c0` accepts layers 1 and 4 and nothing else, reading the bottom at +0
and the top at +2 of an 18-byte cell:

```
1  box set A   0x00675fa0
4  box set B   0x006edfc0
```

It builds the box's corners from `index << 19` and `(index + 1) << 19`, so a
ground box is an axis-aligned column spanning exactly one cell and its two
altitudes fully determine it. With that, the layer numbering is complete: 0, 2
and 3 go to the height queries and 1 and 4 to the box query, and nothing
accepts all five.

## Which box texture is on which face

Not in the binary anywhere I have found, so it was measured. Take every box
cell across eight levels whose four side slots are three-of-a-kind plus one odd
one out, and which has exactly one boxless neighbour:

```
odd slot 0    z neighbour exposed 160    x 2
odd slot 1    z 129                      x 4
odd slot 2    x 145                      z 0
odd slot 3    x 141                      z 0
```

Decisive: slots 0 and 1 face along z, slots 2 and 3 along x. Slot 4 is the top,
with 80 distinct textures across `FLOAT`'s 1,142 boxes, and slot 5 the bottom
with 18 - which is what you would expect of a face nobody sees.

Which member of each pair faces which way is **not** settled, and the honest
answer is that this evidence cannot settle it: 81 against 77 for the z pair and
82 against 63 for the x pair is noise. A stricter test over isolated two-cell
runs gives 7 against 1, which is suggestive, too small to rely on, and
disagrees with the one case I read by hand. `BoxFace::AXIS_ONLY` is a constant
in the port rather than a comment so that it shows up in a search.

## Looking at it

`hb heightmap` draws a level's ground from above, shaded by the per-triangle
normal, with box cells tinted and roofed cells tinted differently. Sub-cell
resolution, so the checkerboard split is visible in the shading.

`HOTH` comes out as a snowbound valley with a network of canyons cut through
it, a walled compound of buildings in the middle and a ring road around the
outside - and the roofed-cell tint traces the canyons exactly, which is what
chambers are for.

`FLOAT` comes out as a scatter of isolated platforms in an empty void, one of
them a cross. The level is called FLOAT and the briefing calls the planet
Eyrie. A level that looks like its own name is not proof, but a level that
looked like static would have been disproof, and after entry 8's inverted
hemispheres that was the live worry.

```
float: 640x640, 1142 box cells, 2053 roofed cells
hoth:  640x640,  696 box cells, 16359 roofed cells
```

`HOTH` has a chamber in almost every cell and `FLOAT` in one in eight, which
reads correctly as a valley carved out of solid ground against platforms
hanging in nothing.

**Still unknown:** which member of each box side pair faces which way. The
ground cell's third `u16` and the chamber cell's four spare bytes. The `.CLR`
high byte. Why `0xABB9` is not `0xAAAB`.
