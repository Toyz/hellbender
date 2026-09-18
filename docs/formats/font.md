---
title: The .BIN and .NDX font
status: solid
covers: STARTUP\FONT.BIN, STARTUP\FONT.NDX
worklog: 24
---

# The font

Two files under `STARTUP\`, and between them the front end's typeface: a bold
italic with an outline, 95 glyphs, one for every printable ASCII character from
space to `~`.

## Layout

`FONT.NDX` is text - one decimal width per line, 95 of them, CRLF:

```
8
6
6
16
11
...
```

`FONT.BIN` is the pixels: glyph after glyph in the same order, each
`width * 23` bytes, row major, 8-bit indexed.

```
glyph i covers ASCII 32 + i
its pixels start at sum(widths[0..i]) * 23
```

## The height is arithmetic

Nothing in either file states it. The widths sum to **1,137** and the bitmap is
**26,151** bytes, and 26,151 / 1,137 is exactly **23**. There is no other whole
number that divides it, so the height is 23 and the storage is row major - the
column-major reading of the same bytes produces noise.

Widths run from 3 (for `'`) to 22. The glyphs use about the top 18 of the 23
rows; the rest is descender space that only some characters reach into.

## The ink is a reserved colour

Of the 26,151 pixels, 14,395 are index 0 - transparent - and of the 11,756 that
are ink, 5,381 are index 255 and the rest are shades of the outline. That puts
nearly half the ink in the **reserved 240 to 255 range** that the `.LTE` never
shades and the `.MIX` never blends.

Which is the point of that range. A typeface has to appear in the colour it was
drawn in whatever the lighting is doing, and the engine reserves sixteen
entries so it can. This is the fourth independent confirmation of that boundary
- after the shade and blend tables, the models' flat colours and the ground
textures.

## Notes

- Index 0 is transparent here as it is in every other 8-bit image the game
  ships - see [.RAW](raw.md).
- There is one font. `TVREG.BIN` and `TVSW.BIN` sit beside it under `STARTUP\`
  and are both zero bytes.

## Unknown

The line height and letter spacing the engine uses - this port advances the pen
by the glyph's width with no gap, which reads correctly but may not be what the
front end does.
