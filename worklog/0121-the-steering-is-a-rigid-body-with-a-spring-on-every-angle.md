---
number: 121
title: The steering is a rigid body with a spring on every angle
date: 2026-09-22
area: decomp, port
files: crates/hb-sim/src/steer.rs, crates/hb-sim/tests/steer.rs, crates/hb-sim/src/flyer.rs, crates/hb-sim/tests/flyer.rs, crates/hb-world/src/grid.rs, crates/hb-world/tests/chambers.rs, crates/hb-formats/src/fixed.rs, crates/hb-fly/src/battle.rs, crates/hb-fly/src/main.rs, docs/engine/simulation.md, docs/formats/terrain.md, crates/hb-sim/src/flight.rs, crates/hb-sim/src/mission.rs
supersedes: 78
resolves: 33, 77, 78
---

# 121. The steering is a rigid body with a spring on every angle

Three entries have ended on `0x4944c0`: 33 found it and put it aside as 1,250
instructions of x87, 78 needed its modes 2 and 3 for the hover craft, and the
transports from 113 call it too. Everything that flies goes through it, so it
was read before the transports.

## Reading it

`tools/x87.py` traces straight lines, and this routine is all branches, so it
was read by hand from the listing with the tracer checking one block at a
time. The stack frame settles once all four registers are pushed, and every
local is named against that frame: the arguments are the actor, the type, the
speed, the turn rate, a thrust factor, the target and a mode.

It is a rigid body. The actor carries a velocity in its own frame at `+0x15c`
to `+0x164` and three angular rates at `+0x168` to `+0x170`. Each frame it
works out a heading and a pitch toward the target - away in mode 1, at the
player's lead point in modes 3 and 4 - and, when the target is well off its
nose and it is moving, a bank. Each angle gets a spring and a damper: 4 times
the error for pitch and heading, 5 for roll, less 2.5 times the rate. The
tracer settled the one question objdump could not: `de ea` is
`fsubp st(2), st`, and whichever way objdump names it, the bytes subtract the
damping from the spring. Thrust is 8 units a second squared times the factor,
backward in modes 2 and 4; at the asked-for speed it turns into minus the
forward speed, so the speed settles and a speed of zero coasts down. Sideways
and vertical speed decay by a tenth a second. The velocity is turned into the
world by the actor's matrix, which `0x40b1c0` rebuilds from the angles after
every behaviour routine - so by last frame's angles. The matrix is the port's
own `axes()` term for term.

Two things in it are not what they look like. The ground look-ahead predicts
where the actor will be from a stack slot that is only written at the end of
the routine, so it predicts with whatever velocity the previous call left
there. And the floor clamp at the end is by class: the flyers never go below
their clearance, the tunnel classes follow the floor underground, and
everything else, the transports among them, is held only under the ceiling.

The clearance is type `+0x98`. The `.DEF` parser never writes it; `0x404d40`,
after the parse, copies a 52-byte model record over `+0x74` to `+0xa7` and then
copies `+0x98` over the radius at `+0x08`. So at run time the two are the same
value, and the port's radius is it.

## The floor and the ceiling

The clamp asks `0x41c300` and `0x41c4d0`, which the port did not have: it
had the top of the solid under a point, not the floor a point would come down
on. Above the ground the floor is box A's top if the point is above the box's
bottom, else the ground, else - where the ground is at zero - the chamber
floor; below it, box B's top, else the chamber floor. The ceiling is the bottom
of the box over the point, the chamber's ceiling underground, or 256 units.
`hb_world::Grid::floor_under` and `ceiling_over` are those two. The test
expected a tunnel's ceiling to be the ground plane, on the strength of notes
in `terrain.md` and `Grid::has_box` that an empty box B reads as altitude
zero. Every empty box B cell in six levels is at -32,640 - byte 0, -128
units - so under the ground the ceiling is the chamber's, as the code says,
and both notes are corrected.

## What changed in the flying

`hb_sim::steer` is the routine, and the fighters, mine layers and hover craft
fly on it instead of the port's turn-at-the-turn-rate. Reading the callers'
arguments turned up three more things:

- The fighters ask for twice the thrust when situation 4 has them chasing at
  twice the player's speed (`0x496c30`), and for mode 3 in phase 200 when the
  player is ahead but off their nose (`0x496ce4`), and in phase 900.
- The hover craft in 2002 backs off at **no less** than the player's speed.
  78 read the comparison at `0x4979fa` backwards: `cmp eax, ebp; jle` keeps
  the craft's speed when the player's is smaller, so it takes the larger.
- Mode 3's lead point is `0x4922a0`: the player's position plus his velocity
  times the time a shot at the type's shot speed takes to close, allowing for
  how fast he is drawing away.

A real fighter from each of `HOTH` and `IOWAH`, set against a player circling
25 units off the ground for a simulated minute, makes passes between 26 and
120 units and fires 7 and 18 times, never lower than 5.8 units above the
floor.

**Still unknown:** what `0x4279b0` answers for the look-ahead's segment,
which would move the target sideways; what writes the pushes and kicks at
`+0x178` to `+0x18c`; and what the 52-byte model record `0x473ee0` fills
holds, beyond its `+0x24` being the radius.
