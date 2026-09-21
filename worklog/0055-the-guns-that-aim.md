---
number: 55
title: The guns that aim
date: 2026-09-20
area: decomp,engine,port
files: crates/hb-sim/src/turret.rs,crates/hb-sim/tests/turret.rs,crates/hb-fly/src/battle.rs,docs/engine/simulation.md
resolves: 54
---

# 55. The guns that aim

Class 1 drives 109 placements - the bottom gun turrets, the floating guns -
and it was on the list of routines not read. It turns out to be the class 10
turret with one paragraph changed.

The two routines open with the same fifteen instructions, down to reading the
actor's own turn and move rate bonuses at `+0x74` and `+0x78` and asking to
stay exactly where they are, and they close with the same three calls: ease
the angles toward the wanted ones, accumulate the frame time, and fire through
`0x406dc0` or, for weapon 19, `0x4074e0`. What differs is the aim.

A class 10 turret leads the player in x and z, takes `atan2` of that for its
heading, and asks for pitch 0. A class 1 gun leads in y as well and takes a
second `atan2` for the pitch - `lead_y` against the flat distance, scaled by
minus 65536 over two pi, because positive pitch is nose down. So it tracks the
player up and down, which is what a gun slung under a gantry has to do.

The x87 tracer earned its keep here: the aim is 60 instructions of stack
juggling with three `fpatan`s and two `fsqrt`s between them, and
`tools/x87.py` prints it as the two expressions above rather than leaving it
to be read by hand.

Past vertical the engine wraps rather than clamps. Over a quarter turn the
wanted pitch becomes half a turn less itself and the heading turns about - a
reflection, which is right. Under minus a quarter turn it adds half a turn to
both, which is not the mirror of that. The port does the same thing, oddity
included.

`Turret::aiming` is the class 1 constructor, `battle` collects classes 1 and
10 together, and a class 1 gun's pitch reaches the renderer so the model
points where it is shooting.

**Still unknown:** class 14, which aims exactly as class 1 does but from a point it asks the model for - porting it waits on the animated models being posed.
