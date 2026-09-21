---
number: 67
title: The HUD has its own font, and it is in the executable
date: 2026-09-20
area: decomp, format, ui, port
files: crates/hb-formats/src/hud_font.rs, crates/hb-formats/tests/against_the_game.rs, crates/hb/src/main.rs, crates/hb-fly/src/main.rs, docs/formats/font.md
---

# 67. The HUD has its own font, and it is in the executable

The port's HUD has been written in `FONT.BIN`, the front end's typeface, at
its authored 23 pixels. On a 200-line screen that is a tenth of the height a
line, which is why the readout has always looked like a caption rather than
an instrument. It is the wrong font, and finding the right one started from
the mine.

`0x480ee0` is what the mine's refusal calls to put `Go Faster to Deploy Mine`
on the HUD. Following it: it keeps one message at a time in a buffer at
`0x611cd0` with a countdown at `0x50f594`, and `0x481030` draws it - word
wrapped to 240 pixels, centred, three eighths of the way down the view, on a
filled box, **seven pixels a line**. Seven pixels a line is not a 23-pixel
font.

The measure routine it calls, `0x485a00`, indexes a table at `0x50f530` as
`char * 48` and takes the first byte as a width. That table is the font: 48
bytes a character for 0x20 to 0xff, a width byte and then one byte a pixel,
`width` across by six rows down, 1 where the pixel is set.

```
A, width 4          g, width 4
.##.                ....
#..#                .###
####                #..#
#..#                .###
#..#                ...#
....                ###.
```

Five rows carry the letter and the sixth is the descender's. Widths run 1 to
5 across the printable range, and the measure adds a pixel between letters.

## Reading a font out of a PE image

It is not in an archive, so `hb_formats::hud_font` reads `HELLBEND.EXE`,
which is on the disc beside `GAME.POD` and is input like any other file. That
needed the smallest possible PE reader - the DOS header's `e_lfanew`, the
image base, and a walk of the section table to turn a virtual address into a
file offset - which is forty lines and now sits under the font.

`hb hudfont <out.png>` draws a specimen at four times up, which is how I
checked it: the alphabet, the digits, the punctuation and the mine's refusal
all read. The test in `against_the_game` says it in assertions instead - an
`A` is four wide with a bar across the middle and nothing on the last row, a
`g` has something on the last row, every printable character has a glyph that
fits its record, and `Go Faster to Deploy Mine` measures under the engine's
240-pixel wrap.

`hb-fly` now draws its readout, its objective line and its messages in it,
and a message goes where the engine puts one: centred, three eighths down, on
a box.

**Still unknown:** whether the table runs to 0xff with anything in it above
the printable range - the loader reads all 224 records and 95 have glyphs -
and what the engine draws in the cockpit's instrument areas, which is the
rest of the HUD and is not this.
