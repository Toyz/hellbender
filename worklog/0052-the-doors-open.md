---
number: 52
title: The doors open
date: 2026-09-20
area: port,engine
files: crates/hb-sim/src/quake.rs,crates/hb-sim/tests/quake.rs,crates/hb-render/src/level.rs,crates/hb-fly/src/battle.rs,crates/hb-fly/src/main.rs,docs/formats/scenery.md
resolves: 51
---

# 52. The doors open

Worklog 51 read the box quake state machine. This ports it, so shooting a door
in `hb-fly` opens it.

`hb_sim::quake` is the state machine and nothing else: a `Door` per live box
entry with where its box is now, `shot` to start the ones a hit lands inside,
`step` to advance them, and a list of the cells that moved for the caller to
write back. The renderer's `Level` now carries the parsed `.QKE`, `hb-fly`
builds its doors from it with the terrain's own altitudes, and the loop writes
each moved box straight into `level.terrain` before anything borrows the grid
to read it. The collision and the renderer both read the terrain, so a door
that moves is a door you can fly through - or be crushed by.

A shot reaching the world already came back as `Stop::Ground`; the battle now
keeps those points, and the loop turns each into a cell and an altitude and
offers it to the doors. Missiles too.

## What the data said that the code did not

The one thing the engine's listing left ambiguous was which height is which.
Moving out compares the box's **top** against the first height and moving back
compares its **bottom** against the second, and I assumed that meant every box
rests with its bottom at the second height. Checking that against the archive
said otherwise: only 124 of 277 do.

The rest are parked at the other end. `FLOAT`'s doors sit with their bottom at
the second height and rise to the first; some of `HOTH`'s sit with their top
already at the first, so their first move arrives immediately and what they
actually do is hold, then close, then hold. Same six states, opposite
resting place. With both allowed, 210 of the 277 park at one end or the other,
and the 67 that do not are the entries whose two heights are both zero - the
ones that go nowhere at all, which the engine handles by stepping them by
exactly zero.

That last case cost a small bug: the port snapped them to altitude zero
instead of leaving them alone. The engine computes its final step as
`(h0 or h1) ? h0 - top : 0`, and now so does this.

## What is not there yet

Nothing throws a switch except a shot, which matches the retail build - the
two id-broadcast entry points have no callers. The switch's own texture swap
is read but not ported, so a switch does not light up. The ground quakes are
parsed and ignored; they move a rectangle of the heightfield rather than one
box, and their two kinds have not been told apart.

**Still unknown:** the switch's own texture swap, which is read but not ported, and the ground quakes, which are parsed and ignored.
