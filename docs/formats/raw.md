---
title: The .RAW image
status: solid
covers: ART\*.RAW, DATA\*.RAW
worklog: 6
---

# The .RAW image

8-bit indexed pixels with no header at all: `width * height` bytes, row major,
top to bottom, left to right. Each byte indexes a [.ACT](act.md) palette.

## Dimensions

Not in the file. The engine knows them from the slot the image is loaded into,
and the filename encodes which slot. Sizes observed across both archives:

| bytes | dimensions | count | what |
| ---: | --- | ---: | --- |
| 4,096 | 64 x 64 | 3,762 | textures |
| 16,384 | 128 x 128 | 26 | per-level ground index grid, see [terrain](terrain.md) |
| 64,000 | 320 x 200 | 13 | full screen, mode 200 |
| 128,000 | 320 x 400 | 5 | full screen, mode 400 |
| 307,200 | 640 x 480 | 5 | full screen, mode 480 |
| 6,440 | 140 x 46 | 27 | cockpit hand, mode 200 |
| 12,880 | 140 x 92 | 27 | cockpit hand, mode 400 |
| 30,800 | 280 x 110 | 27 | cockpit hand, mode 480 |
| 8,000 | 320 x 25 | 1 | `INFOBAR.RAW` |
| 42 / 84 / 204 | 6x7 / 6x14 / 12x17 | 3 | `KNOB200/400/480.RAW` |
| 0 | - | 21 | placeholders with no art |

## The three video modes

`HELLBEND.INI` selects a mode with `gamePIXX` and `gamePIXY`, and the art is
authored three times. The `.data` format strings name the convention:
`ckpt%d.raw`, `ckpt%db.raw`, `ckpt%dl.raw`, `ckpt%dr.raw`, `data%d.raw`, where
`%d` is `200`, `400` or `480`. The trailing letter is the view - none forward,
`b` back, `l` left, `r` right.

Mode 400 is mode 200 at double height; mode 480 is double width by 2.4x height
with the artist rounding to whole pixels. That scaling is what pins the
non-obvious dimensions: 46 x 2.4 rounds to 110 and 7 x 2.4 rounds to 17, and
those are the only factorisations that satisfy all three sizes of a sprite at
once.

## Notes

- A zero-length `.RAW` is legal and appears 21 times. Treat it as "no image",
  not as an error.
- The per-level 128 x 128 `.RAW` is not a picture. It is the ground altitude
  grid and shares the extension only by accident of the tooling.
- Index 0 is the transparent colour. Nothing in the format says so, but the
  cockpit overlays prove it: in `ART\CKPT200.RAW` the entire viewport region -
  every pixel from row 60 to row 120 and column 40 to column 280 - is index 0
  and nothing else, and 32,051 of the image's 64,000 pixels are index 0. That
  is the see-through area of the cockpit. `VGA.ACT` entry 0 is `(0, 0, 0)`.

## Unknown

The exact pixel height of `INFOBAR.RAW`. 8,000 bytes is 320 x 25 if the width
matches the screen, which is an inference, not a measurement, so
`Shape::guess` in `hb-formats` deliberately refuses it rather than write an
unverified number into code.
