---
number: 77
title: The mine layer lays its own kind of mine
date: 2026-09-21
area: decomp, port, flight
files: crates/hb-sim/src/flyer.rs, crates/hb-sim/src/mine.rs, crates/hb-sim/src/turret.rs, crates/hb-sim/tests/flyer.rs, crates/hb-sim/tests/mine.rs, crates/hb-fly/src/battle.rs, crates/hb-fly/src/main.rs, docs/engine/simulation.md
---

# 77. The mine layer lays its own kind of mine

[[58]] left classes 55 and 58 as "full AI routines in the `0x49` range" and
moved on. This is 55. The `.DEF` records name themselves once you go looking:
`Mine Layer` in HOTH and HOTH2, `Pincher Flying` and `huge ship` in the two
FLOAT levels. Ten types.

`0x496060` turned out to be the dogfight again. The same two cone tests
(`0x492750`, `0x492940`), the same situation ladder, the same steering
(`0x4944c0`), the same phases 200, 201 and 2000 that [[57]]'s port already
flies. Three differences and one phase of its own:

- Phase 200 has no retreat test. A fighter inside the type's retreat range
  turns for home; a layer goes round again.
- Phase 201 breaks at half the turn rate (`0x49650c`) where a fighter's is an
  eighth.
- Situation 6 - the player ahead of it but outside its 30-degree cone - sends
  phase 200 to **2002** (`0x4964b5`).

Phase 2002 steers to the player's position plus 24 units along the *player's*
nose. Not at him: at where he is about to be. It remembers how close it has
come, and the frame it stops closing it drops a mine there and breaks away.
`0x49661e` will not drop one over a dead player and `0x496666` will not drop
one within eight units of a live one, measured flat - so it cannot simply ram
one into your face.

## Two pools, two mines

The drop writes into a table at `0x5c00e0`: a hundred slots of 48 bytes. The
port already has a hundred-slot mine pool, and it is not that one - the
player's is at `0x61bde0`, and I nearly filed this as the same table before
checking. Only the seven instructions of the layer's AI and the stepper at
`0x495de0` touch `0x5c00e0`. Nothing else in the image mentions it.

And the mine in it is a different animal. The player's ([[64]]) arms when the
ship comes within 16 units, spins on two angles while it waits, and splashes
32 units into everything nearby. This one does none of that:

```
damage = shot_damage * max(1/4, |reach - distance| / reach)
```

It sits still. Each frame `0x479b60` tests the player - and only the player -
against the mine's own reach on each of the three axes, wrapped at the world's
edge. Inside all three, that damage goes through the shield, the slot clears,
and a 1.53-unit explosion goes off, the same size the player's draws.

The reach and the bite come from the laying **type**, not the mine: `+0x18` is
`!NewAtakRet`'s retreat range and `+0x20` is line 1's shot damage. So a mine
layer's mine is as big as the distance that layer likes to keep.

They are opposites, in other words. Yours arms on proximity and kills
everything around it; theirs never arms, never moves, and is only ever for
you.

## In the port

`Flyer::layer` is the same flyer with `lays` set; `Launch::Mine` carries the
position, the reach and the bite out of `step`; `mine::laid` is the second
pool. Five tests: a layer drops one in front of the player and a fighter
never does; the pool fills at a hundred; the bite is full in the middle and
floors at a quarter at the edge; and it keeps its eight units.

**Still unknown:** class 58 - the SPINE 17 hover craft in the three MORBOS
levels, `0x4976d0` - which is the last of the two [[58]] named. The scripted
50 to 52 and 62 to 64 are still untouched. And in this routine: `0x4922a0`,
which is called beside the fire routine at the very end and which the
fighters do not call, and what `0x42f910` does to a laid mine every frame
before it is tested - the port assumes nothing, which is to say it assumes it
sits still.
