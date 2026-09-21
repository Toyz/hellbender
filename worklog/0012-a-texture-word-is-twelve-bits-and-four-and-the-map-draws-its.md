---
number: 12
title: A texture word is twelve bits and four, and the map draws itself
date: 2026-09-17
area: format, world, render, port
files: crates/hb-formats/src/terrain.rs, crates/hb/src/view.rs, docs/formats/terrain.md
slug: a-texture-word-is-twelve-bits-and-four-and-the-map-draws-its
---

# 12. A texture word is twelve bits and four, and the map draws itself

The `.CLR` high byte has been on the Unknown list since
[4](0004-the-terrain-is-seven-128x128-grids-and-the-loader-names-them.md). It is
a UV orientation code, and the low twelve bits are a texture index.

`0x413c20` is the whole of the first half:

```asm
and  eax, 0xfff
lea  ecx, [eax+eax*2]
lea  edx, [ecx*8 + 0x753cd0]     ; 24-byte entries
```

The table at `0x753cd0` is the same one the `.ANI` animated textures register
into, which is why an animated texture is addressed exactly like a static one -
`"Too many flippin textures 1"` at `0x412874` is the overflow check on it, at
1,024 entries. "Flipping" there means flip-book, not mirrored; I spent a
detour assuming otherwise.

The data is the stronger evidence, because it is a prediction that could have
failed. Across all 26 levels and 425,984 ground cells, `word & 0xfff` is always
a valid index into that level's `.TEX` list. The raw `u16` is out of range
3,658 times - and those 3,658 are exactly the cells whose top nibble is
non-zero. The test asserts both counts.

## The orientation nibble

Consumed in exactly one place. Scanning `.text` for the immediate `0xf000`
gives 20 hits and for `shr r32, 12` gives one, and they are adjacent:

```asm
0x413947  and ecx, 0xf000
0x41394d  shr ecx, 12
0x413954  sar edx, 2           ; the top two bits, tested first
0x413a4b  test cl, 2           ; swaps corner 0 with 1, and 3 with 2
0x413ae0  test cl, 1           ; a further swap of the same shape
```

The function takes the cell's four corners and permutes their entries in a
table at `0x59d36c`. So the nibble is a mirror-and-rotate on the cell's UVs.
Which bit is which depends on the corner argument order at that call site,
which I have not pinned, so the port stores the code and does not claim to
interpret it.

Two facts from the data support the reading: 231 texture indices appear with
more than one code, which is what a tile placed at several orientations looks
like, and the code does not correlate with cell parity, so it is authored
rather than derived.

## The shading word is an intensity, not two half-shades

[11](0011-the-other-lte-is-not-a-ramp-and-a-lenient-parser-was-hiding-i.md)
recorded the ground's two `.LTE` bytes as "one per triangle half" because that
is what two bytes and two halves suggests. Wrong. Read as a little-endian
`u16` they span 0 to 511 in every level - nine bits, an intensity in the low
byte and one flag in bit 8. The ceiling is 255 everywhere and the floor is per
level: 64 in `HOTH`, 96 in `ROID`, 160 in `FLOAT`, which reads as that level's
ambient minimum.

The render is what caught it. Indexing the colour ramp separately per half put
a black checkerboard over the entire map, because the second byte is only ever
0 or 1 and a ramp level taken from it is always row 0, while one taken from the
first byte is usually row 15 - and row 15 of a `.LTE` is black. Treating the
pair as one intensity and indexing at `(255 - intensity) >> 4` gives hillsides
lit from one direction.

That is a case where the picture was the only thing that could have told me.
Both readings parse, both produce numbers in range, and only one of them looks
like terrain.

## hb ground

```
hoth:    205/205 textures resolved, 0 cells without one, palette hoth.act
float:   274/274
jurasic: 224/224
morbos:  194/194
```

Every texture named by every cell of every level resolves to an entry that is
actually in the archives. `HOTH` draws as snowfields with ice canyons, a metal
compound of individual buildings, a second installation and a ring road, and
the shading follows the hills. `JURASIC` draws as black volcanic rock with lava
running between it, which is what a level whose briefing calls the planet
Chimera should look like.

This is the first drawing that uses the texture index, the texture list, the
palette and the shading database at once, so it is a check on all four
together. Four pieces that each parse can still be wired up wrong; a map that
looks like a map is evidence that they are not.

**Still unknown:** bit 8 of the ground shading word. What the chamber's 24-bit
shading value decomposes into - it runs from about 24,000 to 240,000 and is not
the same shape as the ground's. Which of the four orientation bits is which
mirror and which rotation.
