---
number: 57
title: The towers shoot
date: 2026-09-20
area: decomp,engine,port
files: crates/hb-sim/src/turret.rs,crates/hb-sim/tests/turret.rs,crates/hb-formats/src/anim.rs,crates/hb-render/src/level.rs,crates/hb-fly/src/battle.rs,docs/engine/simulation.md
resolves: 55
---

# 57. The towers shoot

Class 14 was the last of the crowded behaviour classes left unread: 96
placements, the spider towers and the watch towers. Worklog 55 got as far as
"it aims exactly as class 1 does, but from a point it asks the model for",
and stopped there because the animated models were not posed. Worklog 56
posed them, so here it is.

The point it asks for is `.DEF` line 3's **first muzzle entry, read as a part
index rather than a vertex index**. `0x406ac0` looks at the model: for a
binary one the entry is a vertex, for an animated one it walks to the `0x26`
node and takes that part's interpolated centre through the model's matrix.
One field, two meanings, chosen by what the model is. So a spider tower aims
from the gun on its back rather than from the ground between its legs, and
against a target at its feet that is a completely different angle.

The second difference is that it turns its gun instead of itself. `0x408ee0`
is the same exponential ease as the turret's `0x4068f0`, but over a second
set of fields on the actor - a position at `+0x4c`, angles at `+0x58` - which
the body's own angles never see. A watch tower stands still; only its gun
tracks.

The port does both halves of the behaviour: `Turret::aim_from` takes the
origin, which `battle` fills from `Level::part_origin` each frame, and a
class 14 tower's heading is not written back to its placement, so the model
stays put. What it does not do is turn the gun on the model, which needs the
second pose the engine keeps for it - written down rather than guessed.

That leaves the unread classes at 17 with 54 placements, 55 with 49, 58 with
34, 18 with 29, 3 with 15 and 35 with 12, plus the scripted handful for the
shuttle and for Nyx.

**Still unknown:** turning a class 14 tower's gun on the model, which needs the second pose the engine keeps for it at `+0x4c`.
