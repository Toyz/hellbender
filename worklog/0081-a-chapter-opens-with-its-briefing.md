---
number: 81
title: A chapter opens with its briefing
date: 2026-09-21
area: port, content
files: crates/hb-fly/src/main.rs
---

# 81. A chapter opens with its briefing

[[80]] read the briefings and drew one with `hb brief`. This puts it in front
of the game, which is where it belongs.

`hb-fly` builds the screen when it loads a level and holds it up until a key
is pressed: the frame time is forced to zero while it is showing, so nothing
moves behind it - the doors do not open, the flyers do not think, the clock
does not run. Any key but the quit key flies on, and the frame clock restarts
from that moment so the first frame of the level is not a hundred-second one.

A level with no briefing gets none, which is fifteen of the twenty three:
only the first level of a chapter has one, so the screen appears eight times
in a playthrough, at the top of each chapter, which is what it is for.

The screen is 320x200 in its own palette and the view may be 320x400 or
640x480, so it is scaled in and converted straight to the window's colours
rather than going through the level's palette - it is not part of the world
and has no business borrowing the world's colours.

**Still unknown:** the globe, which should be turning on it ([[80]]), and the
`.SMK` movies that go around it.
