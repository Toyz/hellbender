---
number: 94
title: A polygon from corner list to span
date: 2026-09-21
area: decomp, render
resolves: 93
files: docs/engine/rasteriser.md, docs/engine/rendering.md, docs/formats/mrgl.md, crates/hb-formats/src/mrgl.rs, worklog/0093-node-6-is-node-5-with-a-shaded-span.md
---

# 94. A polygon from corner list to span

[[93]] read node 6 as node 5 with a shaded span and then said one thing too
many: that the value the shaded span interpolates is `+0x18` of the transformed
vertex. It is not. Following the two routines node 6's handler ends with -
`0x40f7b0` to gather the corners and `0x40f8e0` to draw them - turned out to
read the whole pipeline, and `+0x18` belongs to a different record entirely.

## Three stages

**Gather** (`0x40f7b0`). A polygon names its corners by index into the array at
`0x59d360`, 36 bytes each - `[eax + eax*8 + 0x59d360]` with `eax = i * 4`. It
tests the sign bit of `+0x18` across them all, then `rep movs` nine dwords per
corner into `0x770360`, or into `0x76f150` and through the clipper when any
corner needs it. The copy is the whole record, so what the rest of the pipeline
walks is the same 36 bytes.

**Project** (`0x437f90`). For each corner whose `+0x18` still has its sign bit:

```
+0x18 = x * [0x5b3650] / z + [0x5b3658]
+0x1c = y * [0x5b3654] / z + [0x5b365c]
```

So `+0x18` and `+0x1c` are screen x and y, and `0x80000000` is the mark for
"not projected yet" - which is also what `0x40f7b0` was testing. Two integer
divides a corner.

**Edge-walk** (`0x4381c4` into `0x437b20`). Corners in pairs, wrapping; a pair
on one scanline is dropped, the rest become 56-byte edge records in the list at
`0x53b5c8`. Each record is a value and its per-scanline step side by side, and
`0x437b20` says plainly which corner field each comes from: x at `+0x08` from
the corner's `+0x18`, light at `+0x10` from `+0x14`, u at `+0x18` from `+0x0c`,
v at `+0x20` from `+0x10`, z at `+0x28` from `+0x08`, and one more at `+0x30`
from `+0x20`.

That is the correction. The span's `esi` and `edi` are edge records, not
corners. `[esi+8]` is x because the edge's x is at `+0x08`; `[esi+0x18]` is the
edge's *u* slot, which for a shaded polygon carries a shade rather than a
texture coordinate.

## What the fill modes actually are

Both spans start identically - the two edges' `+0x08`, swapped if out of order,
shifted to pixels, and `[ebx*4+0x5f1d70]` for the row. Then:

- `0x4a76ac` masks `0x59d100` to a byte, takes it through the 256-entry remap
  at `0x606a20`, replicates it into a dword and `rep stos`.
- `0x4a723f` subtracts the two edges' `+0x18`, multiplies by
  `[0x603070 + length*4]`, and steps across with `shld`/`shl 8`, masking the
  running value and taking each pixel's own byte through the same `0x606a20`.

`0x603070` had nothing in the image because `0x484811` fills it at startup:
1024 entries of `0xffffffff / n`, entry 0 all ones. So flat and shaded stand,
and the remap they share is not the palette - `0x482b5f` runs a whole 320x200
frame through `0x606a20`, and `0x482b39` runs one through `0x606870` and then
through it.

## Who reads 0x667100

[[93]] left it as a word nobody reads, because all 34 references to it in the
image looked like stores. The 35th is a read at `0x43802a`, inside `0x437f90`
and inside a branch: if `0x502268` - the `useDirect3D` setting, one of the two
the config writes out - is on, the polygon is handed to `0x4261f0` with
`0x667100` and the software path is never entered. `0x4261f0` thunks to
`0x403ef0`, which appends opcodes to a Direct3D execute buffer.

So the number is what the hardware path is told about the primitive, 16 for
flat and 4 for shaded, and the software rasteriser ignores it. The other word,
`0x6670fc`, is the software selector: `0x437f90` switches on it through the
table at `0x438398`, five cases on `value - 1`, and both polygon handlers set
0, which is none of them. Case 3 (`0x4380ae`) is the perspective-correct
texture setup - `0x7fffffff / (z >> 8)` per corner, into `+0x0c` and `+0x10`,
the reciprocal left in `+0x08`.

All of this is now `docs/engine/rasteriser.md`, with the two record layouts as
tables, and `docs/engine/rendering.md` points at it where it used to say the
inner loops had not been read.

**Still unknown:** what fills a corner's `+0x0c` for a shaded polygon. It is
the u slot, the model stream has nothing per vertex, and node 6's handler sets
only the two rasteriser words - so a shade must be written there earlier in the
model transform, and that write has not been found. Also what a corner's
`+0x20` is: the clip interpolates it, the edge record steps it, and no span
read so far touches it. And four of the five `0x6670fc` cases.
