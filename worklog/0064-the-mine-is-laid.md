---
number: 64
title: The mine is laid
date: 2026-09-20
area: port
files: crates/hb-sim/src/mine.rs, crates/hb-sim/src/weapons.rs, crates/hb-fly/src/battle.rs, crates/hb-fly/src/main.rs
resolves: 63
---

# 64. The mine is laid

The port of what worklog 63 read. `hb_sim::mine` is a hundred slots, a `lay`
that refuses below 6.1 units a second and puts the mine a unit behind the
ship, and a `step` that arms on the ship coming within sixteen units, turns an
armed mine, and hands back a blast when the ship is inside again.

`hb-fly` lays one on the `0` key - `keyMine`, which the engine binds there -
draws every live mine with `mine.bin` on its own two angles, and on a blast
splashes 32 units into whatever is inside it and takes the damage on the ship
as well, because the trigger radius is half the blast radius and the ship that
set it off is always in it.

The refusal is a line on the HUD rather than a voice: the engine prints it
with `0x480ee0`, so the port sends a `Voice` with the text and no sound, which
the flash message already handles. `Go Faster to Deploy Mine`, exactly as it
is in the image.

Six tests: the speed refusal and the drop a unit behind, the hundred slots
filling and the hundred-and-first doing nothing, a mine sitting inert until
the ship comes near and then arming and going off in the same frame, an armed
one turning a quarter and an eighth of a circle a second, and the blast being
twice the trigger - which is the sentence that explains why the mine hurts
the player who laid it.

That leaves one weapon unported: the cluster missile's spread (`0x47cec0`),
whose fan count could not be pinned in worklog 44.

**Still unknown:** nothing this entry opened. The mine's `+0x1c` and `+0x28`
are still unread, as entry 63 recorded, and the port writes neither.
