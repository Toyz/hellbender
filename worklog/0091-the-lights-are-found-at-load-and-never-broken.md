---
number: 91
title: The lights are found at load and never broken
date: 2026-09-21
area: decomp, format
files: docs/formats/scenery.md
---

# 91. The lights are found at load and never broken

[[48]] left `.GLT` with two questions: what the eight numbers set, and what
puts a light out. The second is narrower now, and the narrowing is the
finding.

The records are at `0x5d0628`, 92 bytes each, counted by `0x5d05e0`. Thirty
two places touch that count, and reading a handful of them shows why: the
test "is this texture a light" is not a function. It is written out by hand
in eight places, each one the same twelve lines - mask a texture word to
twelve bits, walk the array, compare `+0x00`, compare `+0x04`, answer 1 for
lit and 2 for unlit and 0 for neither.

`+0x08` is the **broken** texture, and not one of those eight compares it.

The load-time scan is `0x48bd60`. It has exactly one caller, `0x44c729`, in
the level load, and it hands every light face it finds to `0x41bda0`. So the
lights are found once, when the level loads, and something is placed at each
one.

What is not there is any path that writes a light's unlit or broken index
back into the terrain. Eight copies of a test that answers "this is a lit
light" or "this is an unlit light", and nothing that turns one into the
other. Either the swap is somewhere the light table is not mentioned - which
would be odd, since it would need the record - or the shipped build finds the
lights, places whatever `0x41bda0` places, and never changes a texture at
all.

**Still unknown:** which of those two it is; what `0x41bda0` puts at a light,
which would say what a light *is* to the renderer; the eight numbers, which
are still eight numbers; and the list at `0x5cafe0` that the scan clears
before it starts.
