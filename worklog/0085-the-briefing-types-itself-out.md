---
number: 85
title: The briefing types itself out
date: 2026-09-21
area: decomp, content, port
files: crates/hb-formats/src/brief.rs, crates/hb-fly/src/main.rs, crates/hb/src/main.rs, docs/formats/level-text.md
---

# 85. The briefing types itself out

[[81]] put the briefing on the screen and Toyz said the text was still
broken. It was: seventeen lines centred on a 200-line screen ran straight
through the two ornaments in the middle of the frame and out the other side.

I had been laying it out by eye. `0x459d40` is the screen itself, and it
says three things.

**The font is the small one.** `0x485da0` and `0x485a00`, which are the HUD's
five-pixel font and its measure. That was a guess in [[80]] made because
`FONT.BIN` at 23 pixels a line cannot fit a briefing on a 200-line screen; it
is measured now.

**It types.** `0x45a034` does not draw a line. It takes **one character**
(`movsx eax, [esi+ebp+0xf0]`), sprintf's it, draws it, measures it, and moves
the pen along by that width - with the loop bounded by a count that grows.
The briefing appears a letter at a time.

**And the world is running behind it.** The same routine turns a camera
(`0x42c9a0`, from an angle out of `fpatan`) and calls the actor loop
(`0x406650`) on every pass of the type-out. The globe turns, and the level's
actors are already thinking, while you read.

## The panel

The art has two dark panels - one above the pair of ornaments at y 115 and a
shorter one below - and the briefing uses the upper. Measured off `BRIEF.RAW`
the rectangle with nothing drawn on it is x 40, y 26, 238 by 86, which is
twelve lines of the small font.

Twelve lines is not seventeen, which is what made the typing make sense: the
port wraps to the panel, types at forty characters a second, and scrolls once
there is more than twelve lines' worth, so the letter being typed is always
the one you are looking at. `hb brief` is a still, so it shows the top.

**Still unknown:** the engine's own typing rate, which comes from a clock
this reading did not follow - forty a second is the port's. What the lower
panel is for. And where the globe goes, which is [[83]]'s viewport question
still: the routine sets a camera and calls the actor loop, so the globe is
drawn as part of a scene rather than blitted, and the port draws neither.
