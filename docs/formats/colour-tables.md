---
title: The colour tables - .MAP, .LTE, .FOG, .MIX
status: partial
covers: FOG\*.MAP, FOG\*.LTE, FOG\*.FOG, FOG\*.MIX, DATA\*.LTE
worklog: 6, 11, 50, 82
---

# The colour tables

Hellbender renders in 8-bit indexed colour, so every operation that would be
arithmetic on a true-colour framebuffer - shading, fogging, blending, picking a
colour - is a table lookup instead. Four table formats do all of it. None has a
header; each is a bare array whose shape is fixed by its size.

## .MAP - 15-bit colour to palette index

32,768 bytes: one `u8` per RGB555 value, indexed as `(r5 << 10) | (g5 << 5) |
b5` with each component 0-31.

```
index = map[(r >> 3) << 10 | (g >> 3) << 5 | (b >> 3)]
```

`FOG\VGA.MAP` resolved against `ART\VGA.ACT` returns a plausible colour for
every sample tested, so the addressing is right. It is not an exact nearest
match: over 300 random RGB555 values the mean squared distance from the
requested colour to the returned palette entry is 3,469, where an exhaustive
nearest-colour search over the same palette averages 1,815. The table was built
by something cheaper than a full search, which is normal for 1996 and which a
port must reproduce rather than recompute if it wants the original's colours.

One `.MAP` per level plus `VGA.MAP` for the front end, 12 in all.

## .LTE - the light ramp

4,096 bytes: 16 rows of 256, `lte[level][index] -> index`.

```
row  0  0 1 2 3 4 5 6 7 ...                identity, full brightness
row  1  0 1 66 68 69 71 73 75 ...
row  8  0 0 1 65 66 67 68 69 ...
row 15  0 ... 0 then 240 241 ... 255       black, except the reserved indices
```

Row 0 is the identity and row 15 sends indices 0 to 239 to index 0, so the rows
are a brightness ramp from full to black and the level index is the shade. It
is a ramp in the true sense: across `VGA.LTE`, 3,815 of the 3,840 steps from
one level to the next are equal or darker in luminance against `VGA.ACT`.

Indices 240 to 255 are **reserved** and every level passes them through
unchanged. See below.

There is a second, much larger `.LTE` per level under `DATA\` at 114,688
bytes. It is **not a ramp**: it is the
[terrain shading database](terrain.md), seven bytes per cell. 114,688 happens
to be a multiple of 256, so a parser that checks only for that reads it as a
448-row ramp without complaining - `Ramp::parse` in the port checks for exactly
4,096 bytes for that reason.

Line 17 of the [.LVL](lvl.md) names the `FOG\` one. The terrain loader opens
the `DATA\` one itself, without asking the manifest.

## .FOG - the fog ramp

Same shape as `.LTE`: 4,096 bytes, 16 rows of 256. The difference is the far
end.

```
row  0  0 1 2 3 4 5 6 7 ...           identity, no fog
row  8  33 209 209 4 5 255 255 ...
row 15  255 255 255 255 ...           everything to index 255
```

Row 15 collapses to index 255 rather than 0, so `.FOG` fades toward the
palette's last entry - the fog colour - while `.LTE` fades toward black. Same
mechanism, different destination, which is why they are separate files and the
same size.

Fog makes no exception for the reserved indices: `FLOAT.FOG` row 15 holds a
single distinct value, 255, across all 256 entries. Light does; fog does not.

One `.FOG` per level, named on line 16 of the `.LVL`.

## .MIX - the blend table

65,536 bytes: `mix[a][b] -> index`, the palette index that best approximates
mixing colour `a` with colour `b`. `FOG\VGA.MIX` is symmetric - `mix[a][b] ==
mix[b][a]` for every one of the 65,536 pairs - and row 0 is the identity over
indices 0 to 239, so blending with index 0 is a no-op and the operation is an
average or an add rather than a replace.

Indices 240 to 255 blend to 0 rather than to themselves, which is the table
saying they may not be blended at all.

Only `VGA.MIX` is populated at 65,536 bytes. Every per-level `.MIX` in GAME.POD
is zero-length except `KREASH.MIX` at 7,936 bytes, which is 31 rows of 256 -
part of a table, not a table.

The engine does not notice. The loader at `0x485140` swaps the level's
extension for `.mix`, opens it in `FOG\`, and reads it with one
`fread(0x5f2e70, 256, 256)` (`0x485197`) whose return it never looks at: a
short file leaves the remaining 57,600 bytes of the buffer holding whatever
was there before.

What is in the 7,936 bytes is the **end** of a table, with something else in
front of it. Reading the file as rows 225 to 255 of a 65,536-byte table, the
last sixteen rows come out exactly right: `mix[a][0] == a` and
`mix[a][a] == a` hold for all of 240 to 255, both reading 240, 241, ... 255
straight down. The fifteen rows before them fail the same two tests, so they
are not rows 225 to 239 of that table. Reading the file as the first 31 rows
instead - which is what a truncated write would leave - fails everywhere.

So it is a fragment of an interrupted build: the last 4,096 bytes of a mix
table for `KREASH`, with 3,840 bytes of something else ahead of them. (Its
reserved rows also disagree with `VGA.MIX`, where 240 to 255 blend to 0
rather than to themselves - one more sign that the two files were not made
the same way.)

Translucency in a level either falls back to `VGA.MIX` or is off.

## The palette is 240 colours plus 16 reserved

Three independent tables agree on the same boundary at index 240:

- `.LTE` passes indices 240-255 through unchanged at every one of its 16
  levels, so they are never shaded.
- `.MIX` maps them to 0 rather than to themselves, so they are never blended.
- `.RAW` index 0 is transparent, at the other end of the same range.

So the renderer owns 0-239 and something else owns 240-255. The obvious
candidates are the HUD, the cockpit instruments and the front end - anything
that must appear in exactly its authored colour whatever the lighting is doing.
`.FOG` is the exception that proves the rule: fog covers everything, reserved
indices included.

## Notes

- All four tables map index to index. A port that renders in true colour still
  needs them if it wants the original's exact output, because the ramps are
  authored, not computed.
- The `.LTE` and `.FOG` shade index is 4 bits. That is the whole dynamic range
  of lighting in the engine.

## How a span reads the ramp

The `.LTE` is read straight into `0x605370` with one
`fread(0x605370, 256, 16)` (`0x4861d3`), and the loader then does two more
things to it (`0x48623c`): it copies row 0 into the 256 bytes *ahead* of the
file, at `0x605270`, and it fills the 256 bytes at `0x606270` - which is
where the file's last row landed - with a single byte from `0x666f54`.

So the table the rasteriser sees starts at `0x605270`, has row 0 duplicated
in front of it and ends in a flat row.

The textured span loop (`0x4a1a01` and the fifteen copies of it that follow -
it is unrolled) reads it like this:

```
mov bl, ch                  ; the texture's x
mov bh, dh                  ; and its y
mov eax, ebp                ; ebp is the interpolated light
mov al, [ebx]               ; al = the texel
mov bl, [eax + 0x605270]    ; and the shade of it
```

`al` is overwritten with the texel and `ah` is left holding the light's own
high byte, so one 32-bit register carries both halves of the lookup and the
whole thing is `table[light >> 8][texel]` in a single addressing mode. The
light is stepped per pixel by `0x513310`, whose gradient was built with
`sar 4` (`0x4a19c0`), so the accumulator is the light shifted down four - and
the byte that picks the row is the light's **top four bits**, which is what
this port uses.

## Unknown

How `.MAP` was generated.

Which end of the ramp a light of zero is. Row 0 of the `.LTE` is the identity
and row 15 sends everything to black, and the port inverts - a bright vertex
takes a low row - because that is what makes a rendered level look like a
rendered level. But `0x44fb7d` puts `0xffff` in the vertex light for a draw
that wants no shading at all, and a straight reading of the span would make
that the *darkest* row, so one of the two is carrying a sign this reading has
not accounted for. The terrain's own bytes run 96 to 255 rather than over the
whole range, which is consistent with either.
