---
number: 42
title: Being shot down
date: 2026-09-20
area: decomp,engine,port
files: crates/hb-sim/src/death.rs,crates/hb-sim/tests/death.rs,crates/hb-fly/src/battle.rs,crates/hb-fly/src/main.rs,docs/engine/simulation.md
---

# 42. Being shot down

Dying in the port was a teleport: health reached zero and the ship was back at
the start of the level, whole. The engine takes seven seconds over it.

`0x464a32` is the branch. With the hull at zero the player's update stops
flying the ship and calls `0x464870` instead, which does four things a frame:
drops the velocity, adds a quarter of a turn a second to the pitch and the
roll - the pitch stopping at `0x1fff`, forty-five degrees, the roll going
round and round - rebuilds the ship's axes from those angles, and drifts it
forward at `0x7a120`, seven and a half units a second. So the wreck noses over
and flies on, turning.

When it comes within two units of the ground it explodes, with the same eleven
puffs at two units that worklog 40 found, and the loadout is reset. After that
the update takes the other branch and gathers the frame time into `0x512618`,
and the game loop ends the level when that passes five seconds.

`hb_sim::death::Wreck` is the sequence, with the constants named; `hb-fly`
flies the camera through it, so the view tumbles with the ship, flashes "SHOT
DOWN", and starts the level again when it is over. Starting again is the
port's choice: what the engine does with a failed mission - the debriefing,
the death movie the `.LVL` names - is not read.

Two tests: a wreck dropped from sixty units noses over to its limit, drifts,
and explodes two units over the ground about where the arithmetic says; and
one already on the ground explodes at once and asks for the level to end five
seconds later, staying where it fell.
