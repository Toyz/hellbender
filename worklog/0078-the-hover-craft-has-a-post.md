---
number: 78
title: The hover craft has a post
date: 2026-09-21
area: decomp, port, flight
resolves: 58
files: crates/hb-sim/src/flyer.rs, crates/hb-sim/tests/flyer.rs, crates/hb-fly/src/battle.rs, docs/engine/simulation.md
---

# 78. The hover craft has a post

Class 58, `0x4976d0`, the other half of what [[58]] left open. Three types,
all `mtwship.bin`, all in MORBOS: *The SPINE 17 hover craft are one tough
mother. Missiles and madness abound.*

It shares the fighters' machinery - the cone tests, the steering at
`0x4944c0`, the turn-in-time test, the gun - and almost none of their
character, because of one thing phase 0 does:

```
[esi+0xb4] = actor.x
[esi+0xb8] = actor.y
[esi+0xbc] = actor.z
```

It remembers where it was put. Every frame after that, the preamble measures
how far it has strayed from that post, flat, and compares it with the type's
attack range. Outside it, the speed handed to the steering is **zero**
(`0x4977da`). Not a retreat, not a turn - it stops. The only thing it still
asks is `0x492750`: if the player is behind him, it goes to phase 201 and
goes home.

You cannot lead one away. It comes as far as its own attack range from where
it was standing and no further.

## The phases

Five, and one of them is new. 2006 is where it starts and where it returns:
speed zero, steering mode 3, target the player - sitting on its post, facing
him, until he comes inside the attack range. Then 200 chases, 2000 breaks
off, 2002 closes, and 201 is the way home.

201 is the nice one. `0x49794e` hands the steering the **distance from home
as the speed**, so a craft 300 units out goes home at 300 units a second and
eases in as it arrives. My first port zeroed the speed off the tether and
forgot that 201 puts one back, so a test flew one 400 units away and watched
it sit there for a hundred seconds. The engine's own answer was in the two
instructions I had skipped.

Modes 2 and 3 of the steering appear only here. What `0x4944c0` does
differently with them is not read: the port flies 2 as an approach capped at
the player's own speed, which is what the phase's other instructions say
(`0x50cc50` is the player's speed and the cap takes the smaller), and 3 as
standing still, which is what a speed of zero makes it anyway.

## In the port

`flyer::Hover` holds a `Flyer` for the flying and its own phase machine on
top. `Flyer::fly` went public so the two share one set of wings. Two tests: it
holds its post while the player circles outside the attack range, comes for
him when he is inside it, and is back on station within eight units after
being led four hundred units away; and off its tether with the player behind
it, it turns for home on the first frame.

That is both of the classes [[58]] named, two entries apart.

**Still unknown:** steering modes 2 and 3, which is now the third entry to
end on `0x4944c0` being 1,250 unread instructions. The scripted classes 50 to
52 (the shuttle and its escort) and 62 to 64 (Nyx) are still untouched, and
they are the last behaviour classes with placements in the shipped levels.
