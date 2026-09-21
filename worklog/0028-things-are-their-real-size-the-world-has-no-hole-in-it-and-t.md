---
number: 28
title: Things are their real size, the world has no hole in it, and turrets shoot back
date: 2026-09-17
area: decomp,engine,render,port,format
files: crates/hb-formats/src/text.rs,crates/hb-sim/src/combat.rs,crates/hb-sim/src/turret.rs,crates/hb-render/src/scene.rs,crates/hb-render/src/level.rs,crates/hb-fly/src/battle.rs,crates/hb-fly/src/main.rs,crates/hb/src/main.rs
slug: things-are-their-real-size-the-world-has-no-hole-in-it-and-t
---

# 28. Things are their real size, the world has no hole in it, and turrets shoot back

Setting out to make things shoot back meant reading the actor code properly,
and the first thing it turned up was that two of the port's readings of the
`.DEF` file were each other's.

## A placement's second field is hit points, and a type's radius is its size

The placement loader's `fscanf` at `0x4059ab` writes straight into the actor
struct, so the eight fields have offsets: kind `0x18`, **`0x1c`**, x `0x00`,
y `0x04`, z `0x08`, then `0x0c`, `0x10`, heading `0x14`. Actor `+0x1c` is what
the damage routine subtracts from (`0x40d3fc`: `mov eax,[edi+0x1c]; sub eax,ebx`)
and what a shot checks is above zero before testing an actor (`0x40d772`). It
is hit points. `0x0c` and `0x10` are pitch and roll - the same order as the
demo's angles.

The draw never reads it. The actor draw at `0x40da00` fills the draw record's
size from **type** offset `0x08` - line 0 field 2 of the `.DEF` record, the
field [16](0016-the-instance-list-was-in-the-same-file-all-along.md) found was
"a function of the model" - and so does `0x42f910` for everything else it
draws, where the third argument is the bounding radius it culls with. The
visibility test at `0x40c21f` weighs `radius << 8 / distance` against 16.
Models are normalised to +/-1.0, so the radius is the draw scale.

So the port had it the wrong way round: it drew objects at their hit points and
gave them hit points from their radius. The data agrees with the new reading
without being asked. Hit points are a count of player laser hits, whose damage
is 4,096 - 7,453 of 7,606 placements are a whole number of them, and none is
zero or below (`a_placements_second_field_counts_laser_hits`). Radii run from
3.8 units at the tenth percentile to 16.3 at the ninetieth, median 5.8, against
an 8-unit cell. Drawn at hit points, objects were a median **twenty times too
small** (tenth percentile 7x, ninetieth 87x).

That is very probably the "everything feels too big" from the playtest. The
terrain was right; the things standing on it were toys.

## Half the world was not being drawn

Checking the new scale needed a frame aimed at a known object, so `hb look
<level> <n>` frames placement n from the south. The first one was black below
the horizon. So was the previous commit's.

The terrain walk started from `Cell::containing(camera)`, the **wrapped** cell,
and drew each cell at `index << 19`. With the camera at x = -176 units that is
cell 106, drawn at +848 - 1,024 units from the eye. Every negative coordinate,
which is half the world, rendered as sky. The walk now starts from
`camera.x >> 19` unmasked and looks data up through the wrapped index; objects
rebase onto the same frame. `a_view_is_the_same_from_either_side_of_the_wrap`
renders one place in HOTH from both sides of the wrap and requires identical
frames plus ground under the horizon; on the previous commit it fails.
`hb-fly` also keeps its camera in the signed world the way the engine does,
sign-extending x and z from 26 bits after every move (`shl 6; sar 6`, e.g.
`0x4069e7`).

[25](0025-the-original-game-s-own-flight-and-the-four-bugs-it-found.md)
confirmed the world against the recorded demo through height and heading
queries, which never went near the renderer's cell walk. Its frames at 20 and
30 seconds, at negative x, render black on the previous commit.

## The behaviour switch

The `missile.wav` string led to `0x40aa90` - a class-25 routine that raises
something out of the ground; no shipped type uses it - and from its one caller
to the switch at `0x40bb93`: `jmp [eax*4 + 0x40c6cc]` on type `+0x0c`, 65
cases. Type records are 664 bytes (`ecx*83*8`) at `[0x500748]`. Case 47 calls
`0x421240`, the course follower from [26](0026-things-move-and-join-their-course-at-the-nearest-point.md),
which confirms the table. Case 10, 118 types - tanks, boats, guns, SAM sites -
calls `0x408c30`.

## The turret

`0x408c30` aims with lead: `t = |d| / shot_speed`, target = player position
plus player velocity (`0x5b3a00`, `0x5b3a08`) times `t`, horizontal only,
`fpatan` into a heading. Pitch is asked to be 0. `0x4068f0` eases toward it -
position by `error * [type+0x64] * dt`, angles by `error * [type+0x68] * dt`;
getting those two the right way round took a second count of the stack
offsets. Frame time accumulates at actor `+0x68` and past `[type+0x6c]` the
turret fires: `0x4074e0` if `[type+0xdc]` is 19, `0x406dc0` otherwise.

`0x406dc0` steps a barrel counter (modes 1 and 2 cycle three and five barrels,
offset 2,048 in pitch or heading from `0x500760` and `0x500778`), builds the
direction, returns if the player is behind it, picks a random muzzle vertex
from line 3, rotates it with the actor, and spawns `0x476780(frame,
[type+0x70], [type+0x218], side, [type+0xdc])`.

So line 1 is move rate, turn rate, fire interval, damage, weapon kind, and
line 10's last field is the shot speed. `flgun` swings at 7.6 a second and
fires every half second for an eighth of the player's health; `krtank` turns at
0.5.

A shot flies the barrel's direction from wherever its muzzle is, so it does not
converge on the player. The HOTH spike gun's five muzzles are spike tips 4.8 to
8.8 units up; 17 units off, a player at their height took 13 of 59 shots and
one at the gun's own height took 3. That is the engine's behaviour as read, not
a port bug, and the test pins those numbers.

## Shots

Pool of 256 at `0x614570`, 104-byte slots, player in 0-127. Two seconds of life
(`0x20000`), eight sub-steps a frame with a point test after each, stopped by
ground or ceiling. Player shots test actors: a line-5 hit sphere (tested axis by
axis) or, without one, the model's box turned to the actor (`0x40ce70`; the box
is filled from the model at `0x404d40`). Enemy shots test the player's box, 2
units each way (`0x465650`). Damage times the type multiplier chosen by weapon
kind from the jump table at `0x40d558`. Near-miss sound at 16 units while
closing (`0x4768a0`).

The player's weapons are a table of 68-byte rows at `0x50e7a0`: the laser is
32 units a second and 1/16 damage, and `0x47d57a` adds the ship's velocity
magnitude to the speed. The port's shots had been 180 units a second for 1.4
seconds doing 1.0; they are now 32 plus the ship's speed, for 2 seconds, doing
1/16.

Frame time is confirmed as 16.16 seconds: `timeGetTime()` times 1179.648 over
18 is exactly 65.536 a millisecond, capped at 0.25 s (`0x481cd7`).

## The guided missile and the player

The SAM sites fire weapon 19 into a second pool (16 x 88 bytes at `0x613700`).
It starts at rest, gains 16 u/s a second to 64, lives 6 s, does 0.25, and
steers in four sub-steps with a gain that ramps 0 to 4.0 over a second, holds
to 1.5 s, and then snaps straight onto the player every sub-step. The SAM
site's model is a group (`SamSite1.Bin`, children `Sam01`-`Sam05` and back),
which the port had never drawn and could not take a muzzle from; it now stands
for its first child, as `0x473ee0` does for the bounds.

The player's health is `0x5b39ec`, full at `0xffff`; the shield `0x62d6e0`
starts at 0.5, takes `shield/2` off every hit, and wears by 1/32 a hit
(`0x4653a0`). A hit plays one of `exp1`-`exp5.wav`.

## In the port

`hb-sim::turret` and a rewritten `hb-sim::combat` hold all of it; `hb-fly`'s
new `battle.rs` runs it per level. The HUD shows health. The port's own
choices - weapon row 1 for the player, ten shots a second, enemy shots not
hitting other actors, respawning at the start on death, points for shots, the
group's first child - are listed in [the simulation](../docs/engine/simulation.md).

Tests: `a_placements_second_field_counts_laser_hits`,
`every_type_record_carries_its_weapon` (1,848 records; 118 turret types, 14 of
them SAMs, 2 with no shot speed), 13 in `hb-sim/tests/combat.rs`, 8 in
`hb-sim/tests/turret.rs` including a real HOTH spike gun and a real FLOAT SAM
site, and `a_view_is_the_same_from_either_side_of_the_wrap`. 93 tests in all.

**Still unknown:** the other 63 behaviour classes, including the flyers (53,
56, 60) that make most of what moves; how a group model animates; the player's
rate of fire and which weapon row it starts on; what the engine does at death;
type field 4, line 2, line 7's first two values and line 10's middle two. The
field of view is still the port's own, and the port's 90 u/s speed limit is
well above the demo's median 16.5 - both worth a look now that objects are the
right size.
