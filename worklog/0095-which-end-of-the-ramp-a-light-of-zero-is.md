---
number: 95
title: Which end of the ramp a light of zero is
date: 2026-09-21
area: decomp, render
files: docs/engine/rasteriser.md, docs/formats/colour-tables.md, docs/formats/mrgl.md, docs/port/plan.md, crates/hb-formats/src/mrgl.rs
---

# 95. Which end of the ramp a light of zero is

Two of the oldest open questions turned out to be one routine apart, and
[[94]]'s pipeline is what made them reachable: both live in the handlers for
the textured polygon nodes, `0x457990` for 0x0e and `0x458500` for 0x18.

## The dark end

The colour tables page has said since [[38]] that the port inverts the light -
a bright vertex takes a low row of the `.LTE` - because that is what makes a
level look like a level, and that a straight reading of the span said the
opposite, so something carried a sign nobody had found.

Here it is, written out identically in both handlers, right after the light at
`0x48a6a0` returns 0 to `0xffff`:

```
xor  eax, 0xffff        ; 0x457a3d and 0x4585be
cdq
and  edx, 0xf
add  eax, edx
sar  eax, 4             ; divide by 16, rounded toward zero
add  eax, 0x100
mov  ds:0x5b3a18, eax
```

Full light gives `0x100` and no light gives `0x10ff`. The span then puts the
texel in the low byte of that number and uses the whole thing as an offset -
`0x4a658d` is `mov al, [texture]` and then `mov al, [eax + 0x606a20]` - so the
row is the top byte, 1 at full light and 16 at none, and the low bits are
thrown away. One addressing mode for `table[row][texel]`.

With the row-0 duplicate that the ramp loader puts in front of the file's rows,
row 1 is `.LTE` row 0, the identity, and row 16 is row 15, black. So a light of
zero is the dark end, the `0xffff` that `0x44fb7d` writes for a draw that wants
no shading comes out as no shading, and `(255 - light) >> 4`, which is what the
port has been doing, is the same row the engine picks.

The fogged spans build a row from depth with the same shape and the same bias
(`0x4a18bc`: shift up two, clamp to `0xffff`, `xor 0xffff`, `sar 3`, clamp to
`0xfff`, add `0x100`), and when both ends of a span come out `0x100` there is a
fast path that skips the table. Direct3D undoes the whole thing at `0x4041a8` -
subtract `0x100`, `xor` with `0xff0` - to get a brightness back, keeping the
low bits the software path drops.

## 0x18 against 0x0e

The other question, open since [[9]] and on the port's missing list: 33,484
polygons are node 0x18 and a few hundred are 0x0e, with the same size formula
and the same payload, and nothing said why.

It is not the record. Both handlers light the polygon identically - the block
above is in both - and then ask the rasteriser for a different setup through
`0x6670fc`:

- 0x18 asks for 1, which is `0x43805c`: per corner, `0x7fffff / (z >> 8)` plus
  `[0x504280]`, and nothing touches u or v. Affine.
- 0x0e asks for 2, which is `0x43809b` into `0x437e10`: find the nearest z
  across the corners, then scale every corner's u, v and z by `zmin / z`. Its
  span (`0x4a044c`) divides again per span.

So 0x0e is the perspective-corrected polygon and 0x18 the affine one, which is
why 0x18 is nearly all of them - and it explains the `perspectiveFlag` the
config writes. The two also go to Direct3D with different codes, 1 and 0x51.

Both handlers check two globals before either: `0x5126e4`, which the config
writes from `flatShadeFlag` (`0x42cff3`) and which swaps in a span that ignores
the texture, and `0x512664`, which redirects the span to `0x4a7892` - not a
screen fill at all, but ones written into a 64-by-64 buffer at `0x668a40` that
`0x42ff37` clears first. Something rasterises models into a coverage mask.

`docs/engine/rasteriser.md` has the shade row beside the rest of the pipeline,
the colour tables page has the answer where the question was, and the port's
plan is one line shorter.

**Still unknown:** which file's rows are at `0x606a20`. The ramp loader at
`0x4861d3` reads its file to `0x605370` and builds an 18-row table around it at
`0x605270`, which is the base the span family at `0x4a1a01` uses; `0x606a20` is
another 18-row table of the same shape, built rather than read - `0x484555`
maps one table through another into it - and it is the one the lit textured
spans index. And what the 64-by-64 coverage buffer is for.
