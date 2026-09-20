---
number: 53
title: The ground moves too
date: 2026-09-20
area: decomp,port
files: crates/hb-sim/src/quake.rs,crates/hb-sim/tests/quake.rs,crates/hb-formats/src/quake.rs,crates/hb-fly/src/main.rs,docs/formats/scenery.md
---

# 53. The ground moves too

The other list in a `.QKE`. 739 live entries across the levels, against 1,192
box ones, and until now the port parsed them and threw them away.

It is the same six-state machine. `0x4121d0` walks the entry's rectangle of
cells and hands each one to `0x411b80`, which dispatches on the state through
its own jump table. Every cell steps by the same amount, so the patch keeps
its shape, and the travel is just the first height less the second - a ground
cell has one height, so there is no thickness to subtract the way a box has.

Two things the box list did not have:

**The where line's fifth number is a layer.** 1 is the ground, 2 a chamber's
floor, 3 its ceiling - the three arrays the mover indexes into at `0x411ba3`.
Across the levels that is 290 entries moving the ground, 126 a chamber floor
and 323 a chamber ceiling. So most of the moving scenery in Hellbender is
underground.

**320 of them never stop.** A ground entry's flags line is four numbers, not
five, so the field that says what an entry watches is its fourth rather than
its fifth, and the first two are the mode byte and a bit. With the mode byte
at 1 and that bit set, the resting state puts the entry straight back into
its cycle: it runs up and down for as long as the level lasts. That is what
the flags `1,1,1,0` mean, and 320 entries carry them.

The same shift explains something the box list left hanging. No ground entry
is shot open - the watch field is 4 or 0 in every one of them and never 1 -
so the only way a ground patch starts is by itself or because a box it
watches moved.

The kind byte picks the mover: 1 for all but two of the live entries, 3 for
one, which reads the ship's own cell first and is not ported, and 2 for one,
which the dispatch does not match at all and which therefore does nothing.

`hb_sim::quake` is now `Quakes` - doors and patches together, with a `Layer`
saying which grid a moved cell belongs to - and `hb-fly` writes each moved
cell back into the terrain, ground and chambers included. `HOTH`'s chamber
ceilings move on their own from the moment the level loads.
