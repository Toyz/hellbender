---
number: 84
title: A destroyed building is one square, not eleven
date: 2026-09-21
area: decomp, render, port
resolves: 40
files: crates/hb-sim/src/explosion.rs, crates/hb-sim/tests/explosion.rs, crates/hb-fly/src/battle.rs, crates/hb-fly/src/main.rs, docs/engine/simulation.md
---

# 84. A destroyed building is one square, not eleven

Toyz: "the explosions on buildings and such seem way too big something feels
wrong". They were, and by about eleven times.

## The window was see-through as well

First, the smaller thing in the same message. The briefing screen showed the
desktop through its dark parts. `minifb` hands the buffer to a 32-bit visual
and a compositing window manager reads the top byte, so a pixel written as
`(r << 16) | (g << 8) | b` has an alpha of zero and is a hole. It never
showed while the view was a lit landscape and showed everywhere on a screen
that is mostly black. Both paths write `0xff000000` now.

## The explosion

`0x47f3f0` is the eleven-puff burst that [[40]] read: ten of twice the size
scattered within it, one of four times at the centre. The port has been
calling it for everything that blows up.

It has six callers in the whole image, and four of them are the player's own
ends - shot down, flown into the ground, flown into a ceiling. Everything
else calls `0x476f90` **straight** and gets one puff: a shot's mark, a
missile's, a mine's. One square, not eleven.

Including a destroyed actor. `0x407c20` switches on the dead thing's class
through a byte table at `0x407d3c` into a jump table at `0x407d00`, and every
arm does the same thing - `0x476f90(&position, type_radius, mode, flag)` -
differing only in the mode and the flag. Class 33 halves the radius. Mode
only matters at `0x476fd3`, where mode 2, which is the burst's, draws a
random rate and the others leave it at 1.

So a building leaves one square of its own size. The port was leaving eleven,
the widest of them four times the building's radius, scattered a radius out
in every direction. A big enough building filled the screen with fire, which
is what Toyz was looking at.

## And a dead end worth writing down

[[40]] also left "what the engine does for a destroyed actor is a different
path, `0x40c7d0`, and is not read". I read it. It searches the type table for
the model `half.bin`, and:

```
0x500c68  "half.bin"
0x500c74  "Unable to find half "
```

No shipped `.DEF` declares it. No archive holds it. The search fails every
time, the message goes to the log, and the actor is spawned with a type index
of -1, which draws nothing. Whatever `half.bin` was - the name suggests a
wreck cut in two - it did not ship, and the code that would have used it runs
into the wall on every kill in the game.

**Still unknown:** what the mode and flag select beyond the rate, which is
two bits that reach `0x476f90` from thirty call sites and are not read; and
what `half.bin` was for, which is a question about a file that does not
exist.
