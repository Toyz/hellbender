---
number: 120
title: The corners of the world take the lamps' light
date: 2026-09-22
area: decomp, render, port
files: crates/hb-formats/src/terrain.rs, crates/hb-render/src/scene.rs, crates/hb-render/tests/solids.rs, docs/engine/lights.md, docs/engine/rendering.md, docs/formats/terrain.md, docs/port/plan.md
supersedes: 29, 119
resolves: 12, 29, 119
---

# 120. The corners of the world take the lamps' light

Worklog 119 got two things wrong. It said `tools/funcs.py` showed the lamp
scan's only other callee was `0x48b6b0`; the scan has several callees, and
`0x48b6b0` was the one that mattered. And it said `0x413d80` writes the light
at every ground vertex "to an array nothing ever reads". The ground's cell
draw reads it. It reads from `0x52eff8`, 230 entries before `0x52f390`, and
the search looked for the address the writer used. `0x52f390` is the eye's
own vertex in rows of 22 running ten cells before the eye to eleven after;
`0x52eff8` is where the array starts.

## Ground and chambers

`0x414bf0` sets each corner's light (`+0x14`) from the shade byte shifted up
eight, or the ambient, reads the four corners' lamp light from that array, and
hands both to `0x4144b0`, which adds them before drawing. `0x418a60` builds the
same arrays for the chamber floor and ceiling at their heights; the floor's
draw (`0x418d70`) and the ceiling's (`0x419230`) read them and go through
`0x4144b0` too. A whole light adds 256 to a shade of 0 to 255, and nothing
clamps it there.

Reading the chamber draws answers the question 12 and 29 left open: what the
chamber's "24-bit" shade is. It is not one number. Its three bytes land at
`+8`, `+9` and `+10` of the 12-byte chamber record (`0x6bdfb0`), and the floor
draw lights each corner from its own cell's `+8`, or the ambient if bit 0 of
`+10` is set; the ceiling from `+9`, or the ambient on bit 1. Read as one
number, a floor byte, a ceiling byte and flags of 0 to 3 are the 24,000 to
240,000 that 12 measured. The port had lit a chamber flat with its first byte.

## Boxes

The box draw (`0x415420`, 13,749 bytes) asks for the lamps' light at a box's
eight corners, and the order it asks in settles which shadow bit is which
corner, which 29 had taken on trust: bit n is corner n, bottom then top, each
in the ground's order (`0x41572a` to `0x4157ed`).

It also corrects 29. A shadowed corner gets the ambient, but a lit one does
not get full light. It is marked `-1`, and each of the six faces gives its
marked corners `0x48a6a0` of the face's normal - the sun's light by how
squarely the face looks into it, the same routine that lights a model's
polygons - then adds the lamps' light at all four (`0x4165ab` and five like
it). The six calls pass -z, +z, +x, -x, +y and -y. So there is a bottom face,
drawn only when it looks toward the eye (`0x456650`) like the others; the port
had drawn five faces whichever way they looked. Now it drops the faces that
look away, which halves the box triangles in a view of `FLOAT` without
changing a pixel.

## In the port

Every ground, chamber and box corner adds `light_at` times 256 at its own
position, as it is drawn. A render test stands in a `HOTH` tunnel under a lit
ceiling lamp with the objects taken out: 46,810 pixels change when the eight
lights near it are counted.

**Still unknown:** what the span routine does with a light past full; the
points `0x415120` adds to a box face, which the draw casts shadows for again
(`0x415e35`); what `0x48a9e0` adds; a missile's light's strength.
