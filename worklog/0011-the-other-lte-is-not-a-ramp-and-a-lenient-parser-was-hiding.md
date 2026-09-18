---
number: 11
title: The other .LTE is not a ramp, and a lenient parser was hiding it
date: 2026-09-17
area: format, world, port, test
files: crates/hb-formats/src/terrain.rs, crates/hb-formats/src/colour.rs, docs/formats/terrain.md
---

# 11. The other .LTE is not a ramp, and a lenient parser was hiding it

[6](0006-raw-images-have-no-header-and-the-filename-carries-the-video.md)
recorded a puzzle: every level has two files called `<stem>.LTE`, one under
`FOG\` at 4,096 bytes and one under `DATA\` at 114,688, and the `.LVL` names
one `.lte` without saying which directory. The answer is that they are not the
same format and the engine never has to choose.

Line 17 of the [.LVL](../docs/formats/lvl.md) names the `FOG\` one, the 16-row
colour ramp. The terrain loader opens the `DATA\` one itself, at `0x413460`,
with its own `sprintf("%s.lte")` against the `data` directory - it does not ask
the manifest at all.

## Seven bytes a cell

`114688 / 16384 = 7`, and the loop at `0x4134bc` spends them one at a time
across four different arrays:

```asm
mov esi, 0x73bcc4          ; ground + 4,   stride 6
mov edi, 0x6bdfb8          ; chamber + 8,  stride 12
...
fgetc -> [esi]             ; 1  ground[+4]
fgetc -> [esi+1]           ; 2  ground[+5]
fgetc -> [ebp+0x675f9e]    ; 3  box A[+0x10]
fgetc -> [edi-0xc]         ; 4  chamber[+8]
fgetc -> [edi-0xb]         ; 5  chamber[+9]
fgetc -> [edi-0xa]         ; 6  chamber[+10]
fgetc -> [ebp+0x6edfbe]    ; 7  box B[+0x10]
```

which is exactly the fields entries 4 and 8 left as spare bytes. The ground
cell's "unknown `u16`" at +4 is two bytes, one per triangle half. The chamber's
four spare bytes are three shade bytes and one still-unknown. And the box cell
turns out to have two spare bytes that I had miscounted away: 18 is 2 + 2 + 12
for the altitudes and textures, which is 16, not 18, and the shading pass fills
the seventeenth.

It is a cache, and the engine says so. When the file is missing:

```
No .LTE file.  Shading database for the last
time during loading.  Phew!
```

and it computes the same values at `0x41c5d0`, walking the grid against a light
direction in four globals at `0x666f34`. All 26 levels ship one, so the slow
path never runs in practice.

## What the bytes are not

They are not a 0-15 index into the colour ramp, which is what I expected. In
`FLOAT` the first ground byte runs 160 to 255 across 96 distinct values and the
second is only ever 0 or 1. The chamber's three behave the same way - one wide,
one narrow, one tiny. So either the pair is a `u16` that a ramp index is
extracted from, or they are two separate quantities. Reading `0x41c5d0`
properly would settle it; it has not been read past its first screen.

## The lenient parser

`Ramp::parse` accepted any multiple of 256, because the first ramps I looked at
were 4,096 and 114,688 and I assumed the second was a 448-row variant of the
first. 114,688 is a multiple of 256, so the parser read the shading database as
a 448-row ramp and said nothing. The docs then recorded "448 rows" as a fact
about a file that has no rows at all.

The test that caught it is the one asserting the two formats refuse each other:

```rust
assert_eq!(data.len() % 256, 0, "the trap is real");
assert!(colour::Ramp::parse(data).is_err());
assert!(terrain::Shading::parse(fog).is_err());
```

The first line is there deliberately. It asserts that the hazard exists, so
that if some future change makes 114,688 stop being a multiple of 256 the test
says the trap is gone rather than quietly passing for the wrong reason.

`Ramp::parse` now demands exactly 4,096 bytes. Every ramp in both archives is
that size, so nothing was lost by tightening it, and the looseness bought
nothing but a wrong page in the documentation.

**Still unknown:** what the shade bytes mean numerically. The one remaining
spare byte in each box cell and chamber cell. What sets the light direction at
`0x666f34`, which is in uninitialised data and so must come from the level or
the front end.
