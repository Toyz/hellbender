---
number: 70
title: The reticle is a model
date: 2026-09-20
area: decomp, format, ui, port
files: crates/hb-formats/src/mrgl.rs, crates/hb-formats/tests/against_the_game.rs, crates/hb-render/src/hud.rs, crates/hb-fly/src/main.rs, crates/hb/src/main.rs, docs/formats/mrgl.md, docs/engine/hud.md
---

# 70. The reticle is a model

[[68]] and [[69]] both ended on the same unknown: the crosshair in the middle
of the view is in none of the HUD routines. I had chased it through
`keyCrosshair`, which toggles `0x512584`, into a palette index oscillating
between 32 and 63, and stopped there because that index goes into `0x5b3888`,
which half the rasteriser reads.

That was the answer and I walked past it. `0x5b3888` is the shade a model is
drawn with, and four instructions after the oscillation, `0x4652ae` pushes
`0x5b381c` at the model draw. `0x5b381c` is loaded at `0x4634cf` from
`target.bin`.

## Nine vertices, fifty units ahead

`MODELS\TARGET.BIN` is 276 bytes: a vertex list, four polygons, and nothing
else - no material, no texture. The vertices are a centre and an octagon two
units across, all at z = 50, and the four polygons are triangles from the
centre out to the up, left, down and right pairs of the ring. A four-armed
cross. Drawn with the camera at the origin and no rotation, fifty units out
and two across subtends about six pixels at 320x200, which is the size the
crosshair is in a screenshot of the game.

Its colour is that oscillation: 32 to 63 and back, a step a frame, negated -
and a negative shade is a palette index outright rather than a band, which
[[38]] worked out. 32 to 63 is the green everything else on the HUD is in.

## Two things the model reader had wrong

The model would not load. Two reasons, both real:

**The vertex list starts at 100.** `+4` of node 0x02 is where the list begins
in the numbering the polygons index - the same field the texel list at 0x04
has and uses. It is zero in all 337 shipped models except this one, which
numbers its nine vertices 100 to 108, so the reader had never needed it and
dropped the polygons as out of range. Every model has exactly one vertex
list, so nothing else moves.

**Node 5 is a polygon the reader skipped.** The draw dispatch table is at
`0x50c448`, a handler per node type, and node 5's is `0x456810`: the same
back-face test as 0x19, the same light, the same ramp lookup on the current
shade colour, and a fill written a word at a time from a colour replicated
into all four bytes. So it is the flat fill again, faster. 41 polygons across
both archives were being thrown away, among them the reticle's four.

The three census tests that count polygons across the archives all moved, so
the helper that says what an indexed polygon is now includes node 5, which
puts them back where they were and keeps each test about what it was about.

## In the port

`hud::reticle` takes the parsed model and projects it the way the engine's
identity camera does, and `hb-fly` walks the colour a step a frame and binds
G to it. `hb fly <level> <out.png>` draws it too, which is how I looked at
it: a green four-armed cross in the middle of the frame, over the cockpit,
beside the readouts and the radar. Which is the screenshot.

**Still unknown:** what a radar blip says, which is [[69]]'s and still open;
the fill mode of node 6, which is the last indexed polygon type nobody has
read; and whether the reticle moves. The engine draws it at the view's centre
with no rotation, and nothing in that routine offsets it, but a gun that
leads its target would want to - `0x474d30` takes a position and is called
beside the radar for every object, and what it draws has not been read.
