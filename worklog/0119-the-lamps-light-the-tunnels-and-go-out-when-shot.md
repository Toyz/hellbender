---
number: 119
title: The lamps light the tunnels and go out when shot
date: 2026-09-22
area: decomp, render, port
files: crates/hb-sim/src/lights.rs, crates/hb-sim/tests/lights.rs, crates/hb-render/src/level.rs, crates/hb-render/src/scene.rs, crates/hb-render/tests/solids.rs, crates/hb-fly/src/main.rs, docs/engine/lights.md, docs/formats/scenery.md
---

# 119. The lamps light the tunnels and go out when shot

Worklog 91 asked two questions of the `.GLT` and answered neither: what its
eight numbers do, and what puts a light out. It had found the load scan
(`0x48bd60`) and nothing after it. The audit in 112 listed "Too many light sources!!"
at `0x48b6b0` among the unread routines, and `tools/funcs.py` shows the scan's only other
callee is `0x48b6b0`, 1,171 bytes with "Too many light sources!!" in it, and
from there the whole thing unrolls.

## Lamps

`0x48b6b0` makes an 80-byte lamp (`0x5c13a0`) for each light a face gives -
round, cone or flat, by bits of the record's fifth number - with the record's
reach (the first number, times eight), its strength (the second), and its
seventh number as hits. The scan hands it every face of the world: ground,
chamber floor and ceiling, and the six faces of each box set. On `HOTH` that is
207 lamps, 178 of them on chamber ceilings. The tunnels are full of them.

`0x41bda0`, which 91 and 92 read as the scan registering cells, is the chamber
floor's height at the face. The scan is placing the lamp.

## What lights what

Every frame `0x48ae30` puts the lit lamps within ten cells of the eye into a
list of lights (`0x5caff0`); shots and missiles add their own - every fourth
shot a round light a quarter strong. `0x48b3c0` bins the list into a grid
around the eye. `0x48b550` sums, for a point, each light that reaches it on
every axis, weighed by `0x48b1e0`: a round light falls off linearly to its
reach, a cone is full inside 45 degrees, a flat light is full.

Four places ask. The object draw asks for any object **below the ground**,
adds the answer to the ambient and turns the sun off for it. The ground draw
asks at a box's corners. `0x413d80` asks at every ground vertex around the
eye and writes the answers to an array nothing ever reads.

## Blinking, and going out

A lamp with a blink time blinks when it has the eighth number or fewer hits
left: on for the third number, off for the fourth, repainting its face lit and
unlit. The shipped numbers make it on for two seconds and off for one frame.

`0x48c800` is what 91 could not find. A shot or missile that stops against the
world finds the surface and face it stopped on; if the face wears a light
with a broken texture, each lamp there loses a hit, and at none left the lamp
breaks, the face is painted broken, and the shot bursts. The seventh number
is 0, 4 or 6 in the shipped files: some lamps go at the first shot, most take
six.

## In the port

`hb_sim::lights` scans, steps, blinks and breaks, and answers the light at a
point; `hb-render` loads the lamps with the level and lights objects below the
ground with them, sun off; `hb-fly` paints the faces and feeds shots in. A
render test puts the camera by one of `MORBOS`'s chamber objects: 32,891 pixels
change when the lamps are counted.

**Still unknown:** how the ground draw uses the light at a box's corners, so
the chamber walls themselves are not lit yet; what `0x48a9e0` adds from the
player's step; a missile's light's strength; and where `0x48bb50` puts a box
face's lamp.
