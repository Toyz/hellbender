---
number: 45
title: The super weapon
date: 2026-09-20
area: decomp,engine,port
files: crates/hb-sim/src/turret.rs,crates/hb-sim/src/weapons.rs,crates/hb-sim/tests/weapons.rs,crates/hb-fly/src/battle.rs,crates/hb-fly/src/main.rs,docs/engine/simulation.md
---

# 45. The super weapon

Worklog 35 found the eight Bion pieces and what collecting all of them does:
weapon 30 becomes unlimited and selects itself. The weapon it selects did
nothing, because the port could not fire it.

It is a missile, launched at the lock like the others, and what makes it
worth eight pieces is in the missile update rather than the launch. Every
sub-step - four a frame - `0x477a19` takes a quarter of its damage and puts it
into everything within 16 units of where the missile is, through the same
cube blast the MIRVs use. It does not have to hit anything: it burns a
sixteen-unit tunnel through whatever it passes, and a full hit is a full hit.

The port fires it, scorches once a frame with the whole damage rather than
four times with a quarter, and selects it when the eighth piece goes in. A
test walks the eight pieces through `powerup::collect`, checks the weapon
becomes unlimited and selectable, fires it at a lock, and holds the reach at
sixteen units against the MIRV's thirty-two.

Two of the player's weapons are still not ported: the cluster missile, whose
launch is a straight-line run of randomised angles I have not pinned to a
count, and the floating mine, which needs the hundred-slot pool at
`0x61bde0` and its own update.
