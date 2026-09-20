---
number: 47
title: The doors are in the .QKE
date: 2026-09-20
area: decomp,format
files: crates/hb-formats/src/quake.rs,crates/hb-formats/tests/against_the_game.rs,docs/formats/scenery.md
---

# 47. The doors are in the .QKE

Twelve of the twenty documentation pages still say `partial`, and `.QKE` was
the one with the most left in it: worklog 15 read the banners and a couple of
numbers out of a sample and stopped there.

The way in was the save routine. `0x40f260` writes the file field by field
from the struct, so its `fprintf` calls are the record in order, and the
reader at `0x40f970` confirms each one with the matching `fscanf`. A ground
entry is 144 bytes, a box entry 184, and both are the same shape: a kind, two
heights, a where, an actor to watch, a motion line, a flags line, five sound
slots, and a trailing number. Only the third line differs - a ground quake
names a rectangle of cells by two corners, a box quake one cell and which of
the two box sets it moves - and only box entries carry the switch block.

The data then says what the game does with them. Across the 26 levels there
are 767 ground quakes and 1,193 box quakes; 747 of the box ones move set B and
446 set A; and 813 of them name at least two sounds, which are files like
`1-0UDOOR.WAV` and `1-0DDOOR.WAV`. A box quake with a door going up and a door
going down, moving one cell's box between two heights, is a door. The heights
are in the terrain's own scale and run from -98 to 82.5 units, the world's own
range - so the underground ones are the tunnel doors.

`hb_formats::quake` parses all 26 files to their last line, and a test holds
what was learned: the cell references are inside the grid, a box quake's set
is A or B, ground entries have no switch block and box entries all do, and a
door's sounds are `.wav` names.

What is still unread is what the motion line's five numbers mean - the middle
three are 16.16 and take a handful of values, 2.0, 0, 2.0 in 479 entries and
3.0, 0, 3.0 in 324, which looks like a distance and a rate with a pause
between - and what the flags and the two trailing numbers select. Moving the
boxes at runtime is `processBoxQuake`, and that is the next piece.
