---
title: The .GLT lights, .QKE moving geometry and .TTY ground types
status: partial
covers: DATA\*.GLT, DATA\*.QKE, DATA\*.TTY
worklog: 15, 47
---

# .GLT, .QKE and .TTY

Three small per-level text files, all count-prefixed in the house style.

## .GLT - destructible lights

A count, then per entry three texture names and two parameter lines.

```
11
ZLTE1ON.RAW
ZLTE1OFF.RAW
ZLTE1BRK.RAW
262144,90000,131072,6,1
32768,4,1
```

The three states are readable from the names: lit, unlit, broken. `262144` is
4.0 in 16.16 and `131072` is 2.0. Ten levels ship one, between 231 and 983
bytes.

## .QKE - moving geometry

The biggest of the small files, up to 72 KB. "Quake" is the engine's own word:
the diagnostics are `processBoxQuake: no match for watchBox found` and the
counters `-------- Ground Quake Count -------` and
`-------- Box Quake Count --------`, and the file writes those same banners.

Two lists, each a banner, a count, and that many entries. The reader is
`0x40f970` and the writer `0x40f260`, and between them every line is
accounted for: a ground entry is 144 bytes at `0x75b0e0`, a box entry 184 at
`0x763d90`, and each field is written in the order the struct holds it.

An entry is the same shape in both lists:

```
------- Ground Quake Entry 1-------   banner, skipped
1                                     kind: 0 does nothing, 1 is live
-16256,-18816                         two heights, terrain scale
56,98,56,98,2                         ground: two cell corners and a mode
                                      box:   one cell and which box set
-1,-1,0,0                             the actor it watches, -1 for none
1,65536,0,65536,0                     motion: the middle three are 16.16
0,0,0,4                               flags: four here, five in a box entry
NULL                                  five sound slots, 12 characters each
NULL
NULL
NULL
NULL
!--Additional quake info--            banner
0                                     one number
@--Box quake switch info--            box entries only
0                                     a number and two more sounds
NULL
NULL
```

The two heights are in the terrain's own scale - a stored height shifted up
eight, the same as the [chambers](terrain.md) - and they are where the piece
moves between. Across the 26 levels they run from -98 to 82.5 units, which is
the world's own range.

What the game does with them:

- **Ground quakes**, 767 of them, name a rectangle of cells by two corners,
  both inside the 128 x 128 grid, and move that patch of the ground's
  heightfield.
- **Box quakes**, 1,193 of them, name one cell and which of the two box sets
  it moves - 747 move set B and 446 set A. 813 of them name at least two
  sounds, and those sounds are `1-0UDOOR.WAV` and `1-0DDOOR.WAV` and their
  like: a door going up and a door going down. The rest move silently.
- The **motion** line's middle three are 16.16 and the same few values over
  and over: 2.0, 0, 2.0 in 479 entries, 3.0, 0, 3.0 in 324, 1.0, 0, 1.0 in
  165. A distance and a rate, on the evidence, with the middle a pause.
- The **watch** line's first value is an actor to watch, `-1` where there is
  none. That is what "no match for watchBox found" is about.

`hb_formats::quake` parses all 26 files to their last line.

## .TTY - the ground type list

Every shipped `.TTY` is the three bytes `0\r\n` - a count of zero and nothing
else. Its name comes from the save side: `0x41e0c0` replaces the level's
extension with `.tty`, opens it in `data` with mode `wt`, and fails with
`"Unable to save ground type list"`.

So the format exists, the editor writes it, and no shipped level uses it. Its
record shape cannot be read from the data.

## Unknown

Every numeric field of `.GLT`. In `.QKE`: what the motion line's five numbers
mean exactly - a distance and a rate fits the values but has not been read out
of the engine - what the flags line counts, and what the number after
`!--Additional quake info--` and the switch block's number select. The record
shape of `.TTY`, which no shipped level uses.

A ground quake and a box quake turn out to share their whole record shape;
only the third line differs, a rectangle of cells against one cell and a box
set, and only box entries carry the switch block.
