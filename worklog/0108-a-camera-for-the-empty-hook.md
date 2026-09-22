---
number: 108
title: A camera for the empty hook
date: 2026-09-21
area: port, ui
files: crates/hb-fly/src/main.rs, docs/port/plan.md
---

# 108. A camera for the empty hook

[[107]] established that a level's entry animation is not in either shipped
executable: `0x45a290` is called as a level starts and its whole body is `ret`,
and the December build's `0x459b90` is the same byte. Asked again whether it
could be restored, so the question got asked twice more and answered twice
more:

- The camera's own pitch, `0x51255c` in the disc build and `0x51257c` in the
  December one, has exactly 22 references in each, at the same offsets from the
  same routines. No sequence in one that is missing from the other.
- The two `.INI` files differ in eleven settings, and the interesting one is
  `cinemaFlag`: **1** on the disc, **0** in the trial. It writes `0x512650`
  (`0x42d108`), which is the word every movie routine in [[106]] is gated on.
  So that open question is closed, and the trial ships with the films off
  rather than with an animation on.

There is nothing to restore. So the port writes one.

## What it does

Four seconds, borrowed whole from the jump-out at `0x45a2a0` and run backwards
over its first half: the eye two units off the ship - `0x512558`, which
`0x481792` sets as a level starts - `0x3f00` above it looking down and half a
turn round, coming level and back into the cockpit. The cockpit picture is not
drawn while the eye is outside it.

**The ship does not move while it runs.** That was the first cut of it and it
was wrong: flying forward at full throttle for four seconds put the player 64
units from the start point and on the ground. Now the ship waits exactly where
`0x471333` put it and only the camera moves, so control begins where the engine
begins it. Any key skips, and `--no-entry` turns it off.

It is the port's second deliberate departure, and `docs/port/plan.md` now has
both.

**Still unknown:** what view modes 1 and 2 are - `0x512568` cycles three and
the jump-out picks 2, so one of them is this outside view and the engine had a
key for it all along.
