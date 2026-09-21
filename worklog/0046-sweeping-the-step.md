---
number: 46
title: Sweeping the step
date: 2026-09-20
area: port
files: crates/hb-fly/src/main.rs,docs/engine/simulation.md
resolves: 41
---

# 46. Sweeping the step

Worklog 41 and 43 gave the ship walls and tunnels, and left one hole open: the
collision only looked at where the ship had arrived. At 48 units a second on
the afterburner a frame is more than a unit and a half, so a thin wall - or a
steep enough piece of ground - could be on the far side of the step before
anything was asked about it.

The engine does not have this problem because its query box covers where the
ship was as well as where it is going (`0x427280` passes both positions and
grows a unit around the pair), so everything between the two is collected in
one go.

The port now walks the step instead: the movement is cut into pieces no longer
than half the ship's unit, each piece is resolved - out of the boxes, above
the ground, inside the chamber - and the walk stops at the first piece that
holds the ship back. A frame at full speed is about four pieces; the slow
flight most of the game happens at is one. It is not the engine's single box,
but nothing the ship can fly into is thinner than half a unit, so it comes to
the same answer.

Resolving one position is now its own function, which is what made the sweep
two lines rather than a rewrite.

**Still unknown:** whether the engine's own sweep subdivides by the same half-unit step, or tests the whole swept box at once.
