---
number: 58
title: Four more classes
date: 2026-09-20
area: decomp,engine,port
files: crates/hb-sim/src/behaviour.rs,crates/hb-sim/src/turret.rs,crates/hb-sim/tests/behaviour.rs,crates/hb-sim/tests/turret.rs,crates/hb-fly/src/battle.rs,crates/hb-fly/src/main.rs,docs/engine/simulation.md
---

# 58. Four more classes

Worklog 54 counted the behaviour classes by placement and found a tail of
small ones. This reads four of them: 17 with 54 placements, 18 with 29, 3
with 15 and 35 with 12. None took long, because three of the four turn out to
be routines already read with one paragraph changed.

**Class 3** is the turret with no lead. It takes `atan2` of the straight line
to the player for its heading and another for its pitch, and there is not a
velocity term in the routine. The classes that shoot are now a table of five
aims over one machine:

```
class 10  lead in x and z, pitch 0
class  1  lead in all three axes and pitch at the player
class  3  pitch at the player, no lead
class 14  as class 1, measured from a part of the model
class 35  as class 14, and the body turns
```

**Class 35** is class 14 with a spin. Its jump table entry adds the frame
time over eight to the actor's heading and then calls class 14's routine
outright - the same eighth-of-a-circle-a-second the hovering class turns at.

**Class 17** leaves. `Shipping out!` is one of the type names and that is
exactly what it does: up at four units a second, turning a sixteenth of a
circle a second, and once it is higher than twice the sky layer it clears the
actor's `+0x1c` and is gone. That ceiling - twice the `.LVL`'s line 30 - is
the same one a dropped powerup is held under, which worklog 48 found from the
other end.

**Class 18** falls. An asteroid starts half the sky layer above the ground,
scattered up to sixteen units either way from where the level put it, and
comes down at 64 units a second tumbling a whole circle a second about one
axis and a quarter about the other two. On the ground it explodes and starts
again from the top, so a level's asteroids rain for as long as you are there
to see them.

Its explosion is offset by three type fields at `+0x8c`, `+0x90` and `+0x94`.
Nothing in the executable writes any of them - a search of the whole text
segment finds only stack slots at those offsets - so they are zero and the
rock explodes where it landed. That is worth recording precisely because it
looks like a gap: the fields exist, the code reads them, and the data never
fills them in.

One care in the port: class 17's departure is not a kill. The engine clears
the actor's `+0x1c`, which drops it from the loop that thinks and the one
that draws, and `hb-fly` does the same with a list of its own rather than
marking it destroyed - otherwise a friendly frigate shipping out would count
against the mission.

`hb_sim::behaviour` now holds the three classes that only move, and
`hb_sim::turret::Aim` the five that shoot. The unread list is down to classes
55 and 58, which live in the `0x49` range with the flyers, and the scripted
handful for the shuttle and for Nyx.

**Still unknown:** classes 55 and 58, which drive 83 placements between them and are full AI routines in the `0x49` range, and the scripted handful: 50 to 52 for the shuttle and its escort, 62 to 64 for Nyx.
