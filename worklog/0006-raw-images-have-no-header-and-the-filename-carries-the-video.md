---
number: 6
title: Raw images have no header, and the filename carries the video mode
date: 2026-09-17
area: format, render
files: docs/formats/raw.md, docs/formats/act.md
---

# 6. Raw images have no header, and the filename carries the video mode

`.RAW` is 8-bit indexed pixels, top to bottom, left to right, and that is the
entire format. There is no width, no height and no magic. The sizes across both
archives:

```
  3,762 x 4,096        64 x 64     textures
     26 x 16,384      128 x 128    the per-level ground index grid
     13 x 64,000      320 x 200    full screen, mode 200
      5 x 128,000     320 x 400    full screen, mode 400
      5 x 307,200     640 x 480    full screen, mode 480
     27 x 6,440       140 x 46     cockpit hand, mode 200
     27 x 12,880      140 x 92     cockpit hand, mode 400
     27 x 30,800      280 x 110    cockpit hand, mode 480
      1 x 8,000        infobar
      3 x 42/84/204    6x7, 6x14, 12x17   the throttle knob
     21 x 0            placeholders that never got art
```

Dimensions come from the engine, and the filename says which set to use. The
format strings in `.data` spell it out - `ckpt%d.raw`, `ckpt%db.raw`,
`ckpt%dl.raw`, `ckpt%dr.raw`, `data%d.raw` - where `%d` is 200, 400 or 480, the
three modes `HELLBEND.INI` selects between with `gamePIXX` and `gamePIXY`.
`CKPT200.RAW` is 64,000 bytes and 320x200; `CKPT400.RAW` is 128,000 and
320x400; `CKPT480.RAW` is 307,200 and 640x480. The suffix letter is the view:
none forward, `b` back, `l` left, `r` right.

The odd sizes fall out of the same scaling and are a useful check on it. The
400 mode is the 200 mode at double height and the 480 mode is double width by
2.4x height:

```
KNOB200  42 = 6 x 7        KNOB400  84 = 6 x 14      KNOB480  204 = 12 x 17
HN*200 6,440 = 140 x 46    HN*400 12,880 = 140 x 92  HN*480 30,800 = 280 x 110
```

7 x 2.4 is 16.8 and the artist rounded to 17; 46 x 2.4 is 110.4 and rounded to
110. Both files are exactly those products, so the factorisation is forced -
no other pair of dimensions gives all three sizes at once.

Twenty-one `.RAW` entries are zero bytes: `CREDITS.RAW`, `HELP1.RAW` through
`HELP7.RAW` and others. They are in the directory and have no content, so the
loader must tolerate a zero-length entry. A port has to as well.

## .ACT is a bare palette

`.ACT` is 768 bytes, 256 entries of three bytes, red green blue. It is not the
Adobe colour table it is named after - there is no 4-byte trailer and no
768-byte-plus-4 form anywhere in either archive. 286 of the 288 `.ACT` entries
are exactly 768 bytes; the other two are zero-length placeholders.

The components are 0-255, not the 0-63 that VGA hardware wanted, so nothing has
to be shifted on the way to a modern framebuffer.

Which palette goes with which texture is recorded in the POD directory rather
than in either file - see [2](0002-the-pod-archive-and-a-name-field-with-two-strings-in-it.md).
The level's own ground palette is named on line 5 of its `.LVL`.

**Still unknown:** whether index 0 is transparent. The cockpit overlays must
have a transparent index for the view to show through, and index 0 is the
obvious candidate, but it has not been checked against the blitter yet.
