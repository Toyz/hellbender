---
number: 41
title: The ship stops at walls
date: 2026-09-20
area: decomp,engine,port
files: crates/hb-world/src/grid.rs,crates/hb-sim/src/collide.rs,crates/hb-sim/tests/collide.rs,crates/hb-fly/src/main.rs,docs/engine/simulation.md
---

# 41. The ship stops at walls

Flying into a building put the ship on the roof. The port's collision was one
height query - the top of whatever is in this column - so a wall was a cliff
that teleported you up it.

## What the engine does

`0x427280` takes the position the flight model just produced and the one
before it, grows a box a unit each way around both, and asks every cell that
box spans what surfaces are inside it. `0x4279b0` walks the cells; `0x428790`
collects, by a mask, the ground's two triangles, the faces of the cell's boxes
and, below zero, the chamber's floor and ceiling.

The response is four lines of arithmetic. For a surface the position is behind,
`0x4277c0` takes the signed distance from its plane - `dot(normal, position -
a point on the plane)`, with the x and z differences wrapped the way every
distance in this world is - multiplies by `0x103e8`, which is one and a
hundredth, and moves the position back along the normal by that. Nothing
touches the velocity, so the ship keeps its speed and slides along the wall.
That single overshoot constant is what stops it from settling exactly on a
plane and jittering in and out of it.

## What the port does

`hb_world::Grid::boxes_near` hands back every box within reach as a solid,
already in the copy of the wrapping world the query point is in.
`hb_sim::collide::push_out` treats the ship as a sphere of the engine's one
unit and pushes it out of each solid - through the least-deep face when it is
inside one, along the line from the nearest point on the box when it is only
touching - with the engine's fraction over, twice, so a corner between two
boxes settles. It reports whether the push was upward, which is what tells
`hb-fly` the ship has landed rather than hit a wall.

The ground stays a height query, now against the ground surface rather than
the top of the boxes, so the boxes do the work that is theirs.

Six tests: outside is left alone, a wall pushes sideways by the missing
distance and its fraction, the top pushes up and says so, inside leaves by the
nearest face, a corner ends up clear of both boxes, and every box of `FLOAT`
pushes a ship out of its middle.

What is still missing is the engine's sweep: it tests the box that covers
where the ship was and where it is going, and this only tests where it is. A
fast enough ship can pass through a thin wall in one frame.
