---
number: 16
title: The instance list was in the same file all along
date: 2026-09-17
area: format, content, render, port
files: crates/hb-formats/src/text.rs, crates/hb-render/src/scene.rs, docs/formats/level-text.md
---

# 16. The instance list was in the same file all along

[15](0015-the-world-is-centred-and-def-is-a-table-of-types-not-a-list-.md)
established that `.DEF`'s records are types rather than placements, and left
"where are instances placed" open with a guess at the spare byte in a ground box
cell. The guess was wrong and the answer was two hundred bytes further down the
same file.

## Ruling out the guess first

The spare bytes are padding. Scanning `.text` for every four-byte value that
falls inside one cell of each array says which offsets code actually touches:

```
ground   0x73bcc0 /6   +0 +2 +4 +5
boxA     0x675fa0 /18  +0 +2 +4 +12 +16
chamber  0x6bdfb0 /12  +0 +2 +4 +6 +8 +9 +10
boxB     0x6edfc0 /18  +0 +2 +4 +12 +16
```

Nothing references +17 of a box cell or +11 of a chamber cell. A box is 2 + 2 +
12 + 1 = 17 bytes of data padded to 18 for alignment, and a chamber is 11
padded to 12. Two entries' worth of Unknown closed by a scan rather than a
disassembly.

The scan paid for itself twice: `+12` of a box cell appears exactly once in each
set, and `+12` is texture slot 4, which
[10](0010-the-ground-query-closes-and-a-level-that-looks-like-its-own-n.md)
identified as the top face by measuring texture variety. Two independent routes
to the same slot.

## The second section

The DEF loader does not stop when the records run out. At `0x40592e`, having
read 100-or-fewer type records, it reads another count from the same file,
refuses more than 500, and loops:

```asm
fscanf(file, "%d\n", &count)
cmp  eax, 0x1f4                 ; 500
fscanf(file, "%d,%d,%d,%d,%d,%d,%d,%d", ...)
```

And `FLOAT.DEF` has 2,371 lines where 83 records need 2,076. The remaining 295
are a count and 293 instances:

```
293
0,81920,-21493030,-3768320,19365510,0,0,0
0,81920,-22517262,-5439488,20000101,0,0,32703
1,65536,-21759556,-3768320,18127669,0,0,32632
```

```
kind, scale, x, y, z, 0, 0, heading
```

I had been slicing the file to `1 + count * 25` and never looked past it. The
line count was there the whole time.

## What the data says

Across all 26 levels: **7,606 instances**, not one with a kind index outside its
level's type table. x and z between -512.0 and +511.9 units; y between -125.5
and +127.5. That is exactly the world's horizontal bounds from
[15](0015-the-world-is-centred-and-def-is-a-table-of-types-not-a-list-.md) and
exactly the terrain's vertical span of 127.5 units, which is altitude byte 255
scaled by 2^15. Fields 5 and 6 are zero in all 7,606.

`HOTH` places 476, `SHIP` 290, `FLOAT` 293.

## Models are normalised

The scale field made no sense until I measured the models. 235 of the 238
`.BIN` models have a maximum absolute vertex component of exactly 16,383 or
16,384, and **none exceeds 16,384**. Model space is 2.14 fixed point spanning
-1.0 to +1.0, and the placement's `scale` is the object's half-extent in world
units. A vertex reaches the world as `(vertex * scale) >> 14`.

The three exceptions are two empty models and `CUBE.BIN`, whose components are
±2595 and ±2662 - a test model that never went through the exporter's
normalisation, which is exactly why it was such a legible first example back in
[5](0005-mrgl-the-node-stream-inside-a-model-and-its-size-table-as-a.md).

My first attempt used `vertex / 256`, which makes a full-size object 128 units
across - sixteen cells. It looked wrong and it was.

## Drawing them

`hb-render` now draws the placed objects. A position is in the world's signed
coordinates while the terrain walk uses unwrapped indices around the eye, so
each placement is rebased onto the nearest copy of the wrapping world first;
without that an object at -300 units renders 1024 units from the ground it
stands on.

The visual confirmation is weaker than the terrain's, and the reason is in
`docs/engine/rendering.md`: the renderer pairs a mesh's materials with its
polygons by running index, because it walks the parsed lists rather than the
node stream. That is right for a single-material mesh and a guess for the rest.
Objects appear where the map says they should and at plausible sizes; whether
each face has its own texture is not yet shown.

**Still unknown:** what instance fields 5 and 6 are, given they are zero
everywhere. How a mesh's materials bind to its polygons. The `.TXT` animated
model format, which 11 of `HOTH`'s 476 placements and 24 of `FLOAT`'s 293
need.
