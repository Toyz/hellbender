---
number: 116
title: Every gun starts its clock somewhere in the first half second
date: 2026-09-22
area: decomp, port
files: crates/hb-sim/src/turret.rs, crates/hb-fly/src/battle.rs
---

# 116. Every gun starts its clock somewhere in the first half second

113 left this as unknown: the actor setup at `0x404e90`, which the loader runs
over every actor with its index, writes `rand() & 0xffff` into `+0x68`,
`+0x6c` and `+0x70`. `+0x68` is the fire timer - the frame time `0x407770`
accumulates and compares with the type's fire interval - so a gun's first shot
comes somewhere in its first half second rather than all together. `rand()` is
at most 32,767, so the `& 0xffff` never trims it and the start is under 0.5
seconds.

The port started every `Turret` at zero, and a row of SAM sites fired as one.
`hb_sim::turret::first_wait` is the engine's start, and `hb-fly`'s battle gives
one to every turret, every course follower that shoots, every flyer and every
hover craft as the level begins. The change went in with commit `eda6404`; this
is its record, written after 113 had already been made.

**Still unknown:** what `+0x6c` and `+0x70`, set the same way, are used for.
