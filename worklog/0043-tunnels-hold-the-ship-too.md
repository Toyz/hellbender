---
number: 43
title: Tunnels hold the ship too
date: 2026-09-20
area: decomp,engine,port
files: crates/hb-fly/src/main.rs,crates/hb-world/tests/chambers.rs,docs/engine/simulation.md
---

# 43. Tunnels hold the ship too

Worklog 41 stopped the ship at walls above ground. Underground it still flew
through everything, because the port had nothing for chambers at all, and the
ground clamp sampled one point - the ship's middle - so a slope rising under
one wing came through the view.

## What a chamber is

`0x4290f0`, the chamber half of the surface collection, reads two signed
values per grid point out of the array at `0x6bdfb0`: a floor at +0 and a
ceiling at +2, each a sixteen-bit height shifted up eight. So a chamber is a
second pair of heightfields laid under the ground, and the routine reads both
at the cell's four corners. It counts the corners where the two are equal: a
grid point where the floor meets the ceiling is rock, and two of them together
are a wall.

The data agrees. `HOTH` flags 5,414 cells as chamber; the open ones are a
hundred units down and always under their own ground, and the rest - the ones
where floor meets ceiling - are the rock between the tunnels. Two tests in
`hb-world` hold those facts: the shape of a chamber cell, and that every level
with chambers keeps its ceilings under its ground.

## What the port does now

Underground - below zero and in a cell flagged as chamber - the ship is held
between the floor and the ceiling, each sampled at the middle and the four
corners of the ship's own unit, and when the two meet within the ship's height
it is put back where it was horizontally: a wall, crudely. Above ground the
same corner sampling now goes into the ground clamp, which is what stops a
rising slope from cutting through the view.

It is not the engine's collision: that pushes out of the chamber's triangles
the way it pushes out of everything else, with the overshoot, and sweeps
between frames. This keeps the ship inside the tunnel, which is what it was
for.
