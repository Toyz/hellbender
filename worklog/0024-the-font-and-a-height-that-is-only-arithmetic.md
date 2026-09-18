---
number: 24
title: The font, and a height that is only arithmetic
date: 2026-09-17
area: format, ui, port
files: crates/hb-formats/src/font.rs, crates/hb-fly/src/main.rs, docs/formats/font.md
---

# 24. The font, and a height that is only arithmetic

`STARTUP\FONT.NDX` is 95 decimal numbers as text and `STARTUP\FONT.BIN` is
26,151 bytes. Ninety-five is printable ASCII from space to `~`, so the numbers
are glyph widths - they run 3 to 22, which is the right spread for a
proportional face.

Nothing says how tall a glyph is. The widths sum to 1,137, the bitmap is
26,151, and `26151 / 1137 = 23` exactly. No other whole number divides it. So
the height is 23, and rendering a glyph row major at that height produces a
legible letter while the column-major reading of the same bytes produces
noise - which is the check that settles the storage order at the same time.

The result is a bold italic with an outline, and "HELLBENDER" set in it reads
as the game's own logotype.

## A fourth witness for the reserved range

Of the 26,151 pixels, 14,395 are index 0 and so transparent. Of the 11,756 that
are ink, 5,381 are index **255** and the rest are the outline's shades. Nearly
half the ink sits in the reserved 240 to 255 range.

That is the fourth part of the format to land on that boundary independently:

```
7   the .LTE passes 240-255 through unshaded, the .MIX refuses to blend them
17  every flat-colour node in every model falls between 9 and 239
21  the highest index in HOTH's 839,680 ground-texture pixels is exactly 239
24  the font's ink is mostly 255
```

Four mechanisms with nothing to do with each other, agreeing. The reason is
now obvious in hindsight: a typeface has to come out the colour it was drawn
in whatever the lighting is doing, so the engine keeps sixteen entries the
renderer may not touch, and everything that must stay exact lives there.

That is the sort of thing that is only visible after the fact. Entry 7 recorded
the boundary as a measurement without an explanation, and three entries later
the explanation arrived on its own.

## A readout

`hb-fly` draws the level, the altitude, the speed and the cell in the game's
own typeface, toggled with `h`. `hb font` renders a specimen sheet, which is
how the glyphs were checked.

23 pixels is more than a tenth of a 200-line screen, so this is a big face for
a HUD - it was drawn for the front end, where it is a heading rather than a
readout. Whatever the game puts in the cockpit is a different asset and has not
been found yet.

**Still unknown:** the line height and letter spacing the engine uses. This
port advances the pen by the glyph's width with no gap, which reads correctly
and may not be what the front end does.
