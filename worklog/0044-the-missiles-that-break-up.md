---
number: 44
title: The missiles that break up
date: 2026-09-20
area: decomp,engine,port
files: crates/hb-sim/src/turret.rs,crates/hb-sim/src/combat.rs,crates/hb-sim/src/weapons.rs,crates/hb-sim/tests/weapons.rs,crates/hb-sim/tests/combat.rs,crates/hb-fly/src/battle.rs,crates/hb-fly/src/main.rs,docs/engine/simulation.md
---

# 44. The missiles that break up

Two of the player's missiles are not missiles so much as carriers. The MIRV
and the guided MIRV fly for one second and then come apart.

The trigger is the missile's own clock at `+0x38`, which the steering already
uses: the gain on its turn ramps up over the first second of that clock and
the missile snaps straight at its target after one and a half. The split
routines check the same field against 1.0 and, once past it, make ten
missiles from where the carrier is, at half its speed, each pitched anywhere
within ninety degrees and headed anywhere at all.

What the ten are differs. `0x477b90`, the MIRV's, makes Dead-Ons with no
target: a shotgun. `0x477d10`, the guided MIRV's, makes Vipers and walks the
actor list to give each one a target of its own. Both then call `0x47d3b0`
with 32 units and half a hit, and kill the carrier.

`0x47d3b0` is worth its own note: it is the game's splash damage, and it
tests a **cube**. An actor is caught if its distance on x, on y and on z is
each within the reach - the wrapped distances, like everything else in this
world - and it is only tested if the actor loop ran it this frame. The damage
goes through the same entry a shot's does, so a type's missile multiplier
applies.

In the port `Missile::splitting` and `Missile::split` are the carrier's
second, `combat::splash` is the cube, and `hb-fly` turns a carrier into its
ten, damages what the blast catches, and puts an explosion where it was. The
MIRV and guided MIRV join the weapons the port can fire, on 8 and 9.

Three tests: a MIRV launched unguided breaks into ten Dead-Ons at half speed
going different ways, a guided MIRV hands its ten the targets it is given and
lets them fly on when there are none, and a blast catches the corner of its
cube at 31 units but not 33 on one axis, across the world's wrap.
