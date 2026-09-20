---
number: 54
title: The things that hover
date: 2026-09-20
area: decomp,engine,port
files: crates/hb-sim/src/hover.rs,crates/hb-sim/tests/hover.rs,crates/hb-fly/src/main.rs,docs/engine/simulation.md,docs/port/plan.md
---

# 54. The things that hover

The port's plan page had gone stale - it still listed the HUD, the sprites,
the player's death and the collision as things to do, all of which happened
between worklogs 34 and 53. Rewriting it needed an honest answer to "what is
actually left", and the answer for the simulation is: behaviour classes.

So I counted them, over placements rather than over types, which is what
matters for whether a level looks alive:

```
class 0   1874   scenery                     done (nothing to do)
class 9   1436   bunkers and domes           done (nothing to do)
class 10  1378   turrets                     ported
class 7    597   flying ships                ported as flyers
class 26   523   Death Ankhs, asteroids      not ported
class 53   511   fighters                    ported
class 47   285   course followers            ported
class 60   276   fighters                    ported as flyers
class 56   158   fighters                    ported as flyers
class 59   131   fighters                    ported as flyers
class 1    109   bottom gun turrets          not ported
class 14    96   spider and watch towers     not ported
```

Class 26 was the biggest gap and turned out to be the smallest routine.
`0x40ab80` is four lines: the first frame it remembers where the level put the
actor, and every frame after it adds the frame time over eight to a phase and
to the heading, then sets the altitude to the remembered one plus the sine of
that phase times a quarter of the type's radius.

The frame time over eight is 8,192 of the 16-bit circle a second, so the bob
and the turn both come round in eight seconds, and the bob is a quarter of the
thing's own size. Its jump table entry skips the visibility computation the
neighbouring classes run, but the actor loop's 80-unit box still applies, so
one far away holds still.

`hb_sim::hover` is those four lines, and `hb-fly` drives every class-26
placement with it. None of the 523 names a course, so nothing was being
double-driven; they simply stood there before.

Two more shooters are left, and the bigger of the two aims: class 1 computes
the vector from the actor to the player as floats, the way the turret does.
That is 109 more guns that should be firing back.
