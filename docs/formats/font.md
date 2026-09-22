---
title: The two fonts
status: solid
covers: STARTUP\FONT.BIN, STARTUP\FONT.NDX, HELLBEND.EXE:0x50f530
worklog: 24, 67
---

# The two fonts

The game has two, and they are nothing alike. The front end's is a pair of
files under `STARTUP\`; the HUD's is a table inside the executable, and is
what every line drawn over the cockpit is written in.

## The front end's, from `FONT.BIN` and `FONT.NDX`

A bold italic with an outline, 95 glyphs, one for every printable ASCII
character from space to `~`.

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

## The HUD's, from the executable

A table at virtual address `0x50f530`, 48 bytes a character for characters
0x20 to 0xff. The first byte is the glyph's width; the rest is one byte a
pixel, `width` across and six rows down, 1 where the pixel is set. Five of
the six rows carry the letter and the sixth is the descender's, so a `g` and
a `y` use it and an `A` does not.

```
A, width 4          g, width 4
.##.                ....
#..#                .###
####                #..#
#..#                .###
#..#                ...#
....                ###.
```

Widths run from 1 to 5 over the printable range, and the measure routine
(`0x485a00`) adds a pixel between letters, which is how a string is sized
before it is centred. A line takes seven pixels (`0x4810a2` multiplies the
line count by 7).

A message on the HUD is drawn by `0x481030`: the text is word-wrapped to at
most 240 pixels (`0x484920`), centred in the view, three eighths of the way
down it, on a filled box that runs from three pixels left and two above the
text to one past its right and bottom. `0x480ee0` is what puts a message
there, with a duration - 2.0 seconds for the mine's refusal and for a
powerup's line - and only one message shows at a time.

Reading the table means reading `HELLBEND.EXE`, which is on the disc beside
the archives, so `hb_formats::hud_font` maps the virtual address through the
PE section table and reads it from there. `hb hudfont <out.png>` draws a
specimen.

## Running without the executable

The HUD font is the only thing the port reads out of `HELLBEND.EXE` at all -
everything else it needs is in `GAME.POD` and `STARTUP.POD`. So it can be
lifted out once:

```
hb hudfont hudfont.bin --extract
```

writes the table as it is, 12,288 bytes, 48 a character. `hb` and `hb-fly`
both look for `hudfont.bin` beside the archives before they look for the
executable, so a directory holding `system/GAME.POD`, `system/STARTUP.POD`,
`system/Story/` and `hudfont.bin` is a complete game to this port.

Nothing of the table is in this repository, and nothing needs to be: it comes
off the player's own disc.
