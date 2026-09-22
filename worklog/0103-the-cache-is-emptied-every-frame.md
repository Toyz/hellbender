---
number: 103
title: The cache is emptied every frame
date: 2026-09-21
area: format, video, test
resolves: 98
files: crates/hb-formats/src/smk.rs, crates/hb-formats/tests/smacker.rs, crates/hb/src/main.rs, docs/formats/audio-video.md
---

# 103. The cache is emptied every frame

"I swear the intro video to morbos was bugged af." It was. 102 of `MORBIN.SMK`'s
201 frames came out wrong, and [[98]]'s decoder had looked right because the
movie it was checked against was the one movie that could not catch it.

## Measuring before guessing

`hb movie <name> <out.idx>` writes every frame as bare palette indices, and
`ffmpeg -pix_fmt pal8` writes the same thing, so the two can be compared frame
by frame with no palette in the way. That said:

```
MORBBRF.SMK:   0 of 600 frames differ
MSLOGO.SMK:   11 of  90
SNOWIN.SMK:  117 of 201
MORBIN.SMK:  102 of 201
```

A briefing perfect and an arrival half wrong. Every theory that followed had to
explain that shape.

The wrong ones were not wrong in a small way: from the first bad block onward
the frame diverged, and three of them ran past their own chunk and left blocks
undrawn. That is a decoder out of step with the bit stream, not one wrong about
a pixel.

Things measured and cleared, in order: the chunk split (our video chunk is
`ffprobe`'s video packet minus the 769 bytes ffmpeg prepends its palette in,
on all 90 frames of `MSLOGO.SMK`); the palette (only frame 0 carries one, and
the indices differ, not just the colours); the block kinds (`MORBBRF.SMK` uses
all four tens of thousands of times and is exact); the trees (leaf counts still
match the header's `(leaves + 1) * 8` for all four trees of all 32 movies).

## The experiment that answered it

Frame 13 of `MSLOGO.SMK` is the first bad one. Its bytes, its kind byte and the
movie's trees, cut into a one-frame `.SMK` of their own - and decoded from a
black picture and a fresh decoder, **our output is byte-identical to ffmpeg's**
for all 76,800 pixels.

Same bits, same trees, right answer. So nothing in the frame is misread; what
is wrong is what the decoder carried into it. The picture matched through frame
12, which leaves one thing: the three-deep value cache the escape leaves read.

Emptying all four caches at the start of every frame:

```
MSLOGO.SMK:  0 of  90
MORBIN.SMK:  0 of 201
MORBBRF.SMK: 0 of 600
```

and across all 32 movies, **21,780 frames, every one byte-identical**.

## Why the briefing could not catch it

Carrying the caches is only visible when a frame's first cached code lands on a
slot the previous frame left different. `MORBBRF.SMK` is a static screen whose
frames redraw the same regions in the same order, so its carried cache happened
to hold what a fresh one would. A movie with real motion does not have that
luck.

This is the one thing about Smacker that reading the files cannot give you: a
wrong decoder here is right most of the time.

## The test that would have caught it

Not a comparison - there is no reference on a machine with only the disc. An
invariant: every frame must cover all 4,800 of its blocks and stop inside its
own bytes. The old decoder passes that on all 600 frames of `MORBBRF.SMK` and
fails on the fourth frame of `MORBIN.SMK`.

**Still unknown:** nothing new. The two flag bits masked off each frame size
and the three header flags are still unset in every shipped file.
