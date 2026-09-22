---
number: 87
title: What the port was not missing
date: 2026-09-21
area: decomp, port, render
files: crates/hb-render/src/hud.rs, crates/hb-fly/src/main.rs, crates/hb/src/main.rs, docs/engine/hud.md
---

# 87. What the port was not missing

Three of the things on Toyz's list. Two of them turned out not to be missing,
which is worth as much as fixing them and takes as long to establish.

## Object collision: the engine has none either

The ship's collision is `0x427280`. It calls one thing, `0x4279b0`, which
calls one thing four times, `0x428790`, which calls `0x4286e0`, `0x428ac0`,
`0x4294c0` and `0x4290f0`. Not one of them touches the actor list at
`0x781070` or the type table at `0x500748`. The whole chain is cells:
the ground's triangles, the faces of a cell's boxes, a chamber's floor and
ceiling.

So the engine does not push the ship off a placed actor, and neither does
this port, and flying through a bunker is what the game does. What you cannot
fly through is a **terrain box**, and the port does stop at those - five tests
say so, including one that flies a level's real boxes.

I have not ruled out an actor doing the test from its own side - some class
running the player out of its way - which would not be in the collision code
at all. Nothing in the classes read so far does it.

## The black on the textures is the texture

`MORBOS05.RAW` is red-brown rock with black pitting and green lichen in it,
64 by 64, and it looks exactly like that on screen. The speckling is the art.
Terrain draws in the level's own palette and always has.

## The panel is real, and now it is there

`0x420080` had been sitting in [[68]]'s notes as "a multi-line message area,
56 pixels tall, seven pixels a line" and never ported. It is the top-left
panel: 16, 3, 236 by 56 in 640x480, eight lines, and while anything is in it
`0x674d68` is set - which is the flag the weapon and ammunition lines already
read, because the panel covers them and the weapon's picture too.

`hud::panel` writes into it, `hb-fly` sends every voice line there instead of
flashing it across the middle of the view, and the weapon readout steps aside
while it is up, as the engine's does. The middle-of-the-view message
(`0x481030`) stays for what actually uses it - the mine's refusal.

**Still unknown:** how long a line stays. The phrase table ([[86]]) carries
two, four or five seconds an entry, so the answer is per line and the port
uses four for all of them until the triggers are found. And whether the panel
scrolls or clears between messages - the port keeps the last eight lines and
clears when the clock runs out.
