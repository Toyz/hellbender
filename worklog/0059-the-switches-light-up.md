---
number: 59
title: The switches light up
date: 2026-09-20
area: port,engine
files: crates/hb-sim/src/quake.rs,crates/hb-sim/tests/quake.rs,crates/hb-fly/src/main.rs,docs/formats/scenery.md
resolves: 52
---

# 59. The switches light up

The last piece of the `.QKE` that was read but not ported. A box quake with a
switch block names two textures, and the engine writes one of them into all
four of the cell's side faces as the switch goes on and off. What decides
which is not the switch's own state but the position of the box it points at:
while that box is away from where it rests, the switch is lit.

So a switch in this port is a door that carries two texture names and a
`lit` flag, and `Quakes::step` sets the flag from whether anything watching
its id is still moving. Each change comes back as a `Swap`, which `hb-fly`
resolves against the level's texture list and writes into the terrain - the
same place the moved boxes go, so the renderer picks it up with everything
else.

That leaves the `.QKE` with nothing unported except the kind 3 ground entry,
which keys off where the ship is and has one instance in the whole game.

**Still unknown:** the kind 3 ground quake, which keys off the ship's own cell and has one instance in the whole game.
