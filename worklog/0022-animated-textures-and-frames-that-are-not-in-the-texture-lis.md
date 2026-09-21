---
number: 22
title: Animated textures, and frames that are not in the texture list
date: 2026-09-17
area: format, render, port
files: crates/hb-formats/src/text.rs, crates/hb-render/src/level.rs, docs/formats/level-text.md
slug: animated-textures-and-frames-that-are-not-in-the-texture-lis
---

# 22. Animated textures, and frames that are not in the texture list

`.ANI` is a count, then per entry a base texture, a `frames,delay` pair and
that many frame names. Eleven levels ship one and all eleven parse exactly: 146
cycles of two to eight frames, with delays of a tenth, a quarter or a half
second in 16.16.

The obvious implementation resolves every name against the level's `.TEX` list,
because that is where a terrain texture index points. I wrote it that way, and
`FLOAT` reported 22 of its 24 cycles working, which looked close enough to be
right.

It was not. **140 of the 146 cycles name at least one texture the level's
`.TEX` does not contain**, and every one of those textures is in `ART\` all the
same. The two that `FLOAT` dropped were the two whose *base* is missing; the
other 22 were quietly animating between whichever of their frames happened to
be in the terrain list, which for most of them is only the first.

So the `.TEX` list is the terrain's set and nothing more. An animation frame is
just another texture, loaded by name. The engine keeps a single 1,024-entry
texture table that both register into, which
[12](0012-a-texture-word-is-twelve-bits-and-four-and-the-map-draws-itse.md)
found when it identified `"Too many flippin textures 1"` at `0x412874` as that
table's overflow check - and noted at the time that "flippin" means flip-book.
This is what that table is for.

The port mirrors it by appending the frames to the texture list, so a slot past
the `.TEX` names is an animation frame. `FLOAT` loads 274 terrain textures plus
45 frames; `HOTH` 205 plus 31.

## Keeping it out of the rasteriser

`Level::texture_frames(seconds)` returns which slot each slot resolves to at
that instant - the identity, except where a cycle is running - and the scene
walk looks every texture index up through it. The rasteriser never learns that
a texture moves, and a still frame is the same code with the table left out.

That also means the substitution is testable without drawing anything, which is
the reason for doing it that way rather than reaching for the current time
inside the texture lookup.

**Still unknown:** whether the engine's cycles run off the wall clock or the
frame counter, and whether they share a phase. This port runs them off the wall
clock from level load, so two cycles with the same delay stay in step, which
may or may not be what the original does.
