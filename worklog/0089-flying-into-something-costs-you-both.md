---
number: 89
title: Flying into something costs you both
date: 2026-09-21
area: decomp, port, flight
resolves: 87
files: crates/hb-sim/src/combat.rs, crates/hb-sim/tests/combat.rs, crates/hb-fly/src/battle.rs, docs/engine/simulation.md
---

# 89. Flying into something costs you both

[[87]] said object collision was not missing because the engine has none
either, and gave the evidence: `0x427280` to `0x4279b0` to `0x428790`, cells
all the way down, never an actor. Toyz flew it again and said it was still
missing.

Both true. I had looked in the collision code, and it is not in the collision
code.

## Who else hurts the player

Rather than chase the collision further, list every caller of `0x4653a0`,
which is the one routine that takes health off the player. Fourteen of them.
Most are shots. `0x429a23` is a door closing on you - the string beside it is
`You have been crushed`. And `0x40d736` is in the **actor** module, which has
no business damaging the player at all.

`0x40d650` is what it is in. The player's own frame calls it every tick
(`0x464f2f`) with three things: the ship's position, `0x2000 * frame_time`,
and `0x1000`. It walks every live actor whose class is neither 0 nor 9, asks
`0x40ce70` whether that point is inside the type's hit spheres or its turned
box - the same test a shot uses, with a radius of zero - and on a hit:

- the actor takes 0.125 a second, through `0x40d2b0` with 1000 as the weapon
  kind so no damage multiplier applies, and
- the player takes 1/16, on the frame, unless `0x5b36a4` says otherwise.

So flying into a bunker does not stop you and does not bounce you off. It
grinds both of you down for as long as you are inside it. Sit there and you
both die. That is the collision, and it is a fight rather than a wall - which
is why looking for a push found nothing.

The port had `combat::object_at` already, written for something else and
doing exactly this test. Four lines in the battle's frame and two constants.

**Still unknown:** what `0x5b36a4` is - it gates the player's half of the
damage and is saved and restored by the viewport push ([[83]]), so it is
something the little scenes turn off. And whether the engine stops at the
first actor it finds inside, which is what `0x40d650`'s loop reads like and
what this port does.
