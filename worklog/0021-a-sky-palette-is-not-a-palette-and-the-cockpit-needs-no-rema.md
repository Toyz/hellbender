---
number: 21
title: A sky palette is not a palette, and the cockpit needs no remap
date: 2026-09-17
area: format, render, port
files: crates/hb-render/src/level.rs, crates/hb-render/src/scene.rs, docs/formats/sky.md
---

# 21. A sky palette is not a palette, and the cockpit needs no remap

Two things stood between the port and something that looks like the game: the
sky, which was black, and the cockpit, which was not there. Both turned on one
question - how do you get art drawn in one palette into a frame drawn in
another - and the two answers are opposite.

## The cockpit needs no answer at all

A level's palette **is** `VGA.ACT` with sixteen entries changed. All 26 levels
match it on at least 240 of 256, and on all 16 of the reserved 240 to 255
exactly. So the cockpit art, which is drawn in `VGA.ACT`, can be blitted
straight into a frame that is in the level's palette. No remap, no second
palette, nothing.

That is presumably why the levels' palettes were built that way, and it makes
the cockpit a twelve-line function: copy the pixels, skip index 0. That index 0
is transparent was measured back in
[6](0006-raw-images-have-no-header-and-the-filename-carries-the-video.md) by
noticing that the whole viewport region of `CKPT200.RAW` is index 0 and nothing
else.

## The sky needs every answer

A sky `.ACT` is not a palette. It is a gradient band of 15 to 32 entries
starting at index 192, with every other entry black:

```
HOTHSK.ACT     15 entries, 192..206
FLOATSK.ACT    32 entries, 192..223
JURASKY.ACT    32 entries, 192..223, (163,3,7) up to (255,117,5)
```

And a sky texture does not index it directly. `SKY.RAW` uses indices 240 to
254, `NEWSKY.RAW` 244 to 251, `JURASKYY.RAW` 246 to 253 - all above the band.
Subtracting **48** lands every index of every texture inside its own palette's
band, in all twenty levels that have one. Why 48 is unknown; that it holds
twenty times out of twenty is not.

Then the band has to reach a frame that is in the level's palette, and those
two share nothing: `FLOAT.ACT` and `FLOATSK.ACT` agree on 17 of 256 entries, 16
of them the reserved ones. The bridge is the level's `.MAP` - the 15-bit colour
to palette index table from
[6](0006-raw-images-have-no-header-and-the-filename-carries-the-video.md),
whose purpose had never been pinned down. It answers exactly "what is this sky
colour called in this level", which is the question.

```
sky index  ->  minus 48  ->  sky palette RGB  ->  level .MAP  ->  frame index
```

`JURASIC` renders as a red-orange sky over black volcanic rock, which is what
its ramp says. `HOTH` renders as overcast grey. The mechanism was not guessable
from any one file - it needs the texture, the sky `.ACT`, the level `.ACT` and
the `.MAP` on the table at once.

## A third witness for the reserved range

While checking whether the sky's band was carved out of the level palette, I
histogrammed every pixel of `HOTH`'s 205 ground textures. 839,680 pixels, and
the highest index any of them uses is **exactly 239**. Not one reaches 240.

That is the third independent confirmation of the 0-239 renderer range: first
the `.LTE` passing 240-255 through unshaded and the `.MIX` refusing to blend
them, then the flat-colour nodes all falling in 9 to 239, and now the art
itself. Three parts of the format that have nothing to do with each other,
agreeing on the same boundary.

The band at 192 to 223 is *not* reserved, though - ground textures use it for
2.33% of their pixels. The sky and the ground genuinely do collide there, and
the `.MAP` is what keeps them apart.

**Still unknown:** why the sky index bias is 48. How the engine projects the
sky texture - this port wraps it once around the horizon and once from horizon
to zenith, which is a choice, not a reading. What the six space levels draw,
their sky slot being a zero-length `.VOX`.
