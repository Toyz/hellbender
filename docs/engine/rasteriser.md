---
title: The original's polygon pipeline
status: partial
covers: HELLBEND.EXE:0x40f7b0, 0x437f90, 0x437b20, 0x4a723f, 0x4a76ac
worklog: 94
---

# The original's polygon pipeline

Everything the engine draws with corners - a model's polygon, a terrain cell, a
box face, the sky quad - goes through the same three stages: gather, project,
edge-walk. This page is what reading those stages says. The port does not
transcribe them; see [the port's renderer](rendering.md) for what it does
instead.

## The corner record

36 bytes, one per transformed vertex, in the array at `0x59d360`. A polygon
names its corners by index into it, and the indices are `i * 36` because
`0x40f7b0` computes `[eax + eax*8 + 0x59d360]` with `eax = i * 4`.

| Offset | What |
| --- | --- |
| +0x00 | view-space x, 16.16 |
| +0x04 | view-space y, 16.16 |
| +0x08 | view-space z, 16.16; the textured setup replaces it with 1/z |
| +0x0c | u for a textured polygon - and the slot a shaded one interpolates |
| +0x10 | v |
| +0x14 | the vertex light |
| +0x18 | screen x, 16.16, written by the projection; `0x80000000` until then |
| +0x1c | screen y, 16.16 |
| +0x20 | one more interpolated value, carried to the edge record |

The sign bit of +0x18 is how the pipeline knows a corner is not projected yet.
`0x40f7b0` ANDs it across the polygon's corners first: every corner unprojected
*and* something else true means the polygon is skipped outright, and the count
of set ones decides whether the polygon can go straight to the fast path or has
to be clipped. The gather itself is a `rep movs` of nine dwords per corner into
`0x770360` (unclipped) or `0x76f150` (clipped), so the record the rest of the
pipeline sees is the same 36 bytes, copied.

The clip interpolates +0x00 through +0x14 between two corners at a ratio and
carries +0x20 across (`0x414020`), which is the other reason to believe that
+0x18 and +0x1c hold nothing yet at that point.

## The projection

`0x437f90` walks the gathered corners, and for each whose +0x18 still has its
sign bit:

```
+0x18 = x * [0x5b3650] / z + [0x5b3658]
+0x1c = y * [0x5b3654] / z + [0x5b365c]
```

Two divides per corner, in integers. `0x5b3650`/`0x5b3654` are the half-width
and half-height scales and `0x5b3658`/`0x5b365c` the screen centre.

Then it branches on how the polygon is to be filled. `0x6670fc` - which each
draw handler sets before calling - is switched on through the table at
`0x438398`, five cases, on `value - 1`; the flat and shaded polygon handlers
set 0, which is none of them, so they skip this step. Case 3 (`0x4380ae`) is
the perspective-correct texture setup: per corner it computes
`0x7fffffff / (z >> 8)`, multiplies +0x0c and +0x10 by it, and leaves the
reciprocal in +0x08.

## Direct3D

Before any of that, `0x437f90` checks `0x502268` - the `useDirect3D` setting -
and if it is on, hands the polygon to `0x4261f0` with `0x667100` and returns.
`0x4261f0` is a thunk to `0x403ef0`, which appends opcodes to a Direct3D
execute buffer. So `0x667100`, which is the other word a draw handler sets, is
what the hardware path is told about the primitive: 16 for a flat polygon and 4
for a shaded one. The software path never reads it.

## The edge records

`0x4381c4` walks the corners in pairs, wrapping, and hands each pair to
`0x437b20`, which builds an edge record: 56 bytes, in the list at `0x53b5c8`,
counted by `0x53b950`. Pairs on the same scanline are dropped; the rest are put
in top-to-bottom order, and the polygon's top and bottom scanlines accumulate
in `0x53b94c` and `0x53b948`.

An edge carries a value and its per-scanline step side by side:

| Offset | What | Step at | From corner |
| --- | --- | --- | --- |
| +0x00 | first scanline | | +0x1c >> 16 |
| +0x04 | last scanline | | |
| +0x08 | x | +0x0c | +0x18 |
| +0x10 | light | +0x14 | +0x14 |
| +0x18 | u | +0x1c | +0x0c |
| +0x20 | v | +0x24 | +0x10 |
| +0x28 | z or 1/z | +0x2c | +0x08 |
| +0x30 | the spare | +0x34 | +0x20 |

Each step is `(b - a) * (1 / height) >> 16`, the reciprocal coming from the
same 1024-entry table the spans use.

## The spans

A scanline calls whatever routine is in `0x59d10c`, which the draw handler set,
with `esi` and `edi` the two edges. Every one of them starts the same way:
`[esi+8]` and `[edi+8]` are the x ends, swapped if out of order, shifted down
16 to pixels, and `[ebx*4+0x5f1d70]` is the row's address in the framebuffer.

- `0x4a76ac`, the flat fill, takes the byte in `0x59d100` through the 256-entry
  remap at `0x606a20`, replicates it into all four bytes of a dword and
  `rep stos` it.
- `0x4a723f`, the shaded fill, subtracts the two edges' +0x18, multiplies by
  `[0x603070 + length*4]`, and walks the span a pixel at a time with
  `shld`/`shl 8`, masking the running value to a byte and taking *that* through
  `0x606a20`. Unrolled by four.

`0x603070` is 1024 entries of `0xffffffff / n`, filled at `0x484811`; entry 0 is
`0xffffffff`. So the multiply is the divide by the span's length.

`0x606a20` is not the palette: it is a 256-byte remap applied to whole frames
too - `0x482b5f` runs all 64000 pixels of a 320x200 frame through it, and
`0x482b39` runs them through `0x606870` and then it.

## Unknown

What fills a corner's +0x0c for a shaded polygon. It is the slot a textured
polygon's u lives in, the model stream carries nothing per vertex, and the
shaded polygon's handler (`0x456930`) sets only the two rasteriser words - so
something earlier in the model transform must put a shade there, and that write
has not been found.

What +0x20 of a corner is. It is interpolated by the clip, carried into the
edge record and stepped like the rest, and no span read so far uses it.

Which of the five `0x6670fc` cases is which, beyond case 3.
