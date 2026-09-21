---
number: 71
title: The radar I ported was the debug overlay
date: 2026-09-20
area: decomp, ui, port
resolves: 69
files: crates/hb-render/src/hud.rs, crates/hb-formats/src/nav.rs, crates/hb-sim/src/mission.rs, crates/hb-fly/src/main.rs, crates/hb/src/main.rs, docs/engine/hud.md
---

# 71. The radar I ported was the debug overlay

Toyz flew it and said the HUD was still broken. Three things were, and one of
them is mine from [[69]].

## The objective box is three letters

`hb-fly` was putting the mission point's long line in it - `Destroy Target`,
`Pick up Message Pod` - into a box that is eighty pixels wide in 640, forty
at 320x200. It spilled across the cockpit. That is the whole "lol".

What belongs there is the three-letter code the jump table at `0x44ead0`
picks by kind, and the mapping falls out neatly once written down: destroy
is `TGT`, start is `STR`, drop beacon is `RES`, escort is `EST`. Three of the
sixteen entries fall through to the rest of the routine, so sync, end and
warp leave whatever the box already said.

The long line at `0x625110` is written twice in the image and read nowhere,
so where it is drawn - if it is - is still open. It is not the HUD.

## The radar is a different routine

[[69]] found `0x436fe0`, which walks the objects, wraps and turns each
offset, scales it into a rect and draws each one's name. I called it the
radar. It is not: `0x474c3b` runs it only when `0x5b3340` is set, and the
rect it runs with is the whole view, not the dish. It is a debug overlay.

The radar is `0x475100`, four instructions earlier, with the rect set to the
dish. It walks the placements, drops the dead, the dying and classes 18 and
33, and hands each to `0x474f00`. That one has the range I got wrong: the
offset is 16.16, shifted down 11 - units times 32 - then multiplied by the
box's width and shifted down 11 again, which is `width / 64` pixels a unit.
Sixty four units across the box, not the 256 I had. And the edge is round:
the squared offset is tested against 0xef420 first, about 31 units.

A blip is not a mark either. `0x4746a0` draws five pixels - a three-pixel bar
with one above and one below, outlined in the paper colour - and drops the
top and bottom for something below the player, leaving the bar. Its colour is
0x3f when `0x40dc00` says the class is one the guns count as a target, which
is the same question the missile lock asks at `0x47bce6`, and 0x94 when not.

## The arrow lives on the radar

The port drew a nav arrow of my own invention under the distance box. The
engine has no such thing. `0x475290` sets the viewport to the dish, points a
camera down and draws `navtarg.bin` turned by `0x8000` less the bearing -
a four-cornered dart, tip, two shoulders, base. Its colour is the shade
global: -101 ahead, -152 for an objective roughly behind, and a negative
shade is a palette index outright.

So the blue mark on the dish in a screenshot of the game is the objective
arrow, and now it is one here too.

**Still unknown:** how that viewport is set up. The camera goes to the origin
and the model lies in a plane through it, so a projection would divide by
zero - something between `0x42c980` and `0x459a10` parts them, and I have not
read it. The port turns the model's four corners in two dimensions instead,
which puts the same dart in the same box at the same angle. Also still open:
what the debug overlay's per-object string says, and the table at 0x1393 it
comes from.
