---
number: 98
title: Smacker, written out by hand
date: 2026-09-21
area: format, video, port, test
files: crates/hb-formats/src/smk.rs, crates/hb-formats/src/lib.rs, crates/hb-formats/tests/smacker.rs, crates/hb/src/main.rs, docs/formats/audio-video.md
---

# 98. Smacker, written out by hand

The 32 movies under `system/Story/` are 280 MB of the game nobody here had
opened: the intro, the four chapter briefings, the deaths, the ejection, the
ending and the credits. The page on them said "ffmpeg decodes them without
help", which is true and no use to a port that takes no dependencies. So this
is a Smacker decoder, and it was read out of the files rather than out of
anything else.

## The container, proved before anything else

104 bytes of header, then a `u32` a frame, a byte a frame, the packed trees,
and the frames. Every file says `SMK2`, 320 by 240, `rate = -6666` - which the
header's third case turns into 15.0015 frames a second - and `flags = 0`. One
audio track, 11,025 Hz mono 16-bit compressed, except `MSLOGO.SMK` at 22,050.

The check that the reading is right came first: `header + sum of frame sizes ==
file size`, for all 32, exactly. Not one byte over or under. With that holding
there is somewhere firm to stand while the rest is guessed at.

## The trees, and the number that proved them

One bit stream, least significant bit first, four sixteen-bit trees in it. Each
is a presence bit, two byte trees for the halves of a value, three sixteen-bit
escapes, the tree, and a bit.

The first attempt read 350 bytes of a 137,393-byte blob and stopped, which is
the shape of a misalignment rather than a bug. Trying it with and without a
leading presence bit before the whole tree, and letting each variant run all
four trees, settled it in one go: with the bit, `KRESHBRF.SMK` consumed 22,359
of its 22,584 bytes and produced 3393, 1262, 5123 and 356 leaves.

Which is where the real proof turned up. The header's four sizes for that file
are 27152, 10104, 40992 and 2856, and

```
(3393 + 1) * 8 = 27152
(1262 + 1) * 8 = 10104
(5123 + 1) * 8 = 40992
( 356 + 1) * 8 = 2856
```

That relation holds for all four trees of all 32 movies. A leaf count is not
something a wrong reading gets right by accident, four at a time, 128 times
over.

The escapes are the format's cache: a leaf whose value is one of the three
stands for the first, second or third most recently decoded value, and every
decode that is not already at the front moves the list along.

## A frame

Palette first when the frame's byte says so, then a chunk per audio track
behind its own length, then the blocks: 4x4, in rows, a code from the type tree
carrying a kind, a run and sometimes a colour. Two-colour blocks take a pair
from the colour tree and sixteen bits from the map tree, one a pixel. Full
blocks take two codes a row from the full tree, right pair first. Skip leaves
what the last frame left. Fill is one colour for the whole run.

`hb movie <name> <out.png> [frame]` prints a movie's header and writes one
frame. Frame 40 of `MSLOGO.SMK` is the Microsoft logo being drawn by its spark,
frame 300 of `Intro.smk` is the hangar with the ships on their racks, and frame
100 of `MORBBRF.SMK` is the briefing screen with its own text on it:

```
PLANET:  MORBOS
TARGET:  Bion commando transports
SHIP CLASS: II-S
```

Which is the same briefing the port has been drawing by hand out of the `.TXT`
since [[79]]. The game had it as video all along.

Three tests: the frames fill the file, the trees are the size the header says,
and every frame of every movie decodes without the block stream running past
its own bytes. 44,000 frames, eight seconds.

**Still unknown:** the audio. A track's chunk is a Huffman-coded difference
stream with its own trees, and nothing here reads it. Also whether any of the
three flags ever matters - no shipped file sets one - and what the two bits
masked off each frame size mean, since no shipped frame sets either.
