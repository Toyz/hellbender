---
number: 93
title: Node 6 is node 5 with a shaded span
date: 2026-09-21
area: decomp, format, render
files: crates/hb-formats/src/mrgl.rs, crates/hb-formats/tests/against_the_game.rs, docs/formats/mrgl.md
---

# 93. Node 6 is node 5 with a shaded span

[[70]] ended with node 6 named as the last indexed polygon nobody had read,
and the method that read node 5 there reads this one too: the draw dispatch
table at `0x50c448` gives node 6 the handler `0x456930`.

It is node 5's routine again. The same prologue, the same corner list at
`+0x18` of the record, the same back-face test - the plane's three numbers
against `0x5b37d8`, and a zero normal taken as facing - and the same two calls
at the end, `0x40f7b0` to gather the corners and `0x40f8e0` to draw. Only two
instructions differ, and both are stores the rasteriser reads:

```
node 5 (0x456892):  0x59d10c = 0x4a76ac    0x667100 = 0x10
node 6 (0x4569aa):  0x59d10c = 0x4a723f    0x667100 = 0x04
```

And node 6 is missing node 5's middle: no call to the light at `0x48a6a0`, no
read of `0x5b3888`, no ramp lookup. It never touches the current shade colour
at all.

## The two spans

`0x59d10c` is the routine the span loop calls per scanline.

`0x4a76ac`, node 5's, is what [[70]] described: one colour out of `0x59d100`,
through the remap at `0x606a20`, replicated into all four bytes of a dword and
`rep stos`'d across the span.

`0x4a723f`, node 6's, takes the two ends of the span and reads `+0x18` of
each. It subtracts them, multiplies by `[ecx*4+0x603070]` where `ecx` is the
span's length in pixels, and walks the result across the span with
`shld`/`shl 8`, masking the running value to a byte and indexing the *same*
remap table `0x606a20` for each pixel. It writes a byte at a time, unrolled by
four.

`0x603070` had no contents in the image, which is because `0x484811` fills it
at startup: entry 0 is `0xffffffff` and entries 1 to 0x3ff are
`0xffffffff / n`. A reciprocal table, 1024 wide. So the multiply is a divide
by the span length, and what walks across the span is a linear interpolation
between the two corners' `+0x18`.

Which makes node 5 flat and node 6 gouraud, and they are otherwise the same
polygon: the same vertex indices, the same winding, the same clip.

## Where the shade comes from

`+0x18` of what, exactly. `0x40f7b0` answers it: it walks the corner index
list and for each index computes `eax*4` then `[eax+eax*8+0x59d378]`, which is
a stride of 36 bytes into the transformed vertex array at `0x59d378`. So the
interpolated value is a field of the transformed vertex, one per vertex rather
than one per polygon - which is what a shaded polygon needs and what node 6's
missing light explains. The polygon carries no extra data for it; its record
in the stream is node 5's record exactly, a count and a list of vertex indices.

24 polygons in the two archives are node 6. The parser now accepts them with
the other indexed polygons, and the census helper in the tests counts them, so
the three counts that walk the archives keep meaning what they meant. The port
draws them flat, with the polygon's own light, which is what it did for node 5
before it and is wrong by a gradient.

**Still unknown:** what writes `+0x18` of a transformed vertex. All eight
references to `0x59d378` in the image index it rather than name the field, so
the write is somewhere in the transform with a base register and no constant
to grep for; a per-vertex normal in the model would be the obvious source and
the MRGL stream has no node that looks like one. Also `0x667100`, 16 for
node 5 and 4 for node 6: all 34 references to it in the image are stores, one
per draw handler, so whatever reads it reads it through a base pointer.
