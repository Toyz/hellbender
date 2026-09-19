---
number: 38
title: Missiles, and the lock
date: 2026-09-18
area: decomp,engine,port
files: crates/hb-sim/src/turret.rs,crates/hb-sim/src/weapons.rs,crates/hb-sim/src/combat.rs,crates/hb-sim/tests/weapons.rs,crates/hb-sim/tests/turret.rs,crates/hb-fly/src/battle.rs,crates/hb-fly/src/main.rs,docs/engine/simulation.md
---

# 38. Missiles, and the lock

The player starts with twenty Dead-On missiles, five Vipers and two cruise
missiles, and the port could not fire one.

## One pool for everyone

The player's missiles go into the same 16-slot pool as the SAM sites'
(`0x477890`), and one routine flies them all. The record's `+0xc` says whose
it is and `+0x10` what it wants: `0x477f20` returns the player's position for
an enemy's missile, a placement's for the player's, and the missile's own
heading when the target is -1 - which is how the Dead-On flies straight. So
the port's SAM missile needed only an owner, a target and a damage to become
the player's too. Every player missile does a full hit, and a type's
missile multiplier - `.DEF` line 19 - decides what that is worth.

## The lock

`0x50e6fc` holds it. Each frame `0x47b700` checks it with `0x47bbd0`; with
nothing locked it takes the first placement that passes; the lock key steps to
the next. The test is strict: on screen - the frustum test `0x42f710` with no
radius, so within 45 degrees of the nose each way - alive, in the 80-unit
box, not friendly, and of the right kind. The kind is where two small
predicates turned up: `0x40dca0` and `0x40dc00` split the 65 behaviour
classes into things that fly and things that do not. The Viper locks the
first set and the cruise missile the second. That flying set agrees with
what the port had decided from the AI routines - 7, 53, 56, 59 and 60 are
all in it - and adds 29 more classes not read yet.

## In the port

`Missile` has an owner, target, damage, kind and life; the enemy's path is
unchanged and the turret tests pass as before. `hb_sim::weapons` launches the
three missiles and runs the lock; `hb-fly` binds 4, 5 and 6 to Dead-On,
cruise and Viper, V to the lock, marks the HUD's weapon with a star while
locked and puts brackets round the locked placement. Three new tests cover
the lock's kinds and frustum, its stepping and dropping, and the launch
position, homing and straight flight.

Not done: the splitting missiles, the mine and the super weapon; the cruise
missile's own steering; and whatever the engine draws for its lock.
