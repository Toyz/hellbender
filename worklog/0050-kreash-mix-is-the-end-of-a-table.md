---
number: 50
title: KREASH.MIX is the end of a table
date: 2026-09-20
area: decomp,format
files: docs/formats/colour-tables.md
---

# 50. KREASH.MIX is the end of a table

The colour tables page had three unknowns. One of them was a file: every
per-level `.MIX` in `GAME.POD` is zero bytes except `KREASH.MIX`, which is
7,936 - and worklog 6 wrote that off as "not a multiple of 256 and so not the
same shape". It is 31 times 256 exactly. That was worth another look.

The loader first. `0x485140` swaps the level's extension for `.mix`, opens it
in `FOG\`, and reads it with a single `fread(0x5f2e70, 256, 256)` whose return
it never checks. So a short file is not rejected, or padded, or noticed: the
other 57,600 bytes of the table are whatever the buffer held before.

What is in the file is the end of a table rather than the start of one. Two
identities pin a mix table's rows: blending a colour with index 0 leaves it
alone and blending it with itself leaves it alone, so `mix[a][0] == a` and
`mix[a][a] == a`. Reading the 31 rows as rows 225 to 255 of a full table, the
last sixteen satisfy both exactly - 240, 241, ... 255 straight down the first
column and down the diagonal. The fifteen rows before them fail both, so they
are not rows 225 to 239. Reading the file as rows 0 to 30, which is what a
truncated write leaves, fails everywhere.

So `KREASH.MIX` is 4,096 bytes of a real mix table's reserved rows with 3,840
bytes of something else in front - a fragment of a build that was interrupted,
not a table the game can use. Its reserved rows also disagree with `VGA.MIX`,
where 240 to 255 blend to 0 rather than to themselves.

The second unknown moved without closing. The terrain shading database's
per-cell bytes are not row numbers: the ground draw reads the byte for each
vertex and shifts it up eight (`0x414e30`), so the rasteriser interpolates an
intensity from 0 to 0.996, and a vertex whose following flag byte has bit 0
set takes the level's ambient instead of its own (`0x414e0b`). Sixteen rows
over that range is the top four bits, inverted, which is what `hb-render`
already does - but the span loop that would prove it has not been found yet.
