---
title: The .GLT lights, .QKE moving geometry and .TTY ground types
status: partial
covers: DATA\*.GLT, DATA\*.QKE, DATA\*.TTY
worklog: 15, 47, 48
---

# .GLT, .QKE and .TTY

Three small per-level text files, all count-prefixed in the house style.

## .GLT - destructible lights

The extended ground light table, in the loader's own words: its error is
`Bad ext ground light`. It says which textures mean a light, and which to put
in their place when the light is off or shot out.

A count, then per entry three texture names and two parameter lines.

```
11
ZLTE1ON.RAW
ZLTE1OFF.RAW
ZLTE1BRK.RAW
262144,90000,131072,6,1
32768,4,1
```

The three states are readable from the names: lit, unlit, broken. The reader
is `0x48c4c0` and the writer `0x48c6c0`, and between them they give the record
in full: three sixteen-byte names, then the first two numbers, then three
texture indices the loader fills by matching each name against the level's
[texture list](lvl.md) (`0x48c2a6`), then the six numbers left. In memory that
is 92 bytes in an array at `0x5d05f0` counted by `0x5d05e0`.

The fourth line may carry two numbers rather than five, and then there is no
fifth line and the record takes the loader's defaults (`0x48c630`). Every one
of the 90 shipped records uses the long form, so the short one is only the
older files' shape.

[Line 32](lvl.md) of the `.LVL` names the file, and where that line is empty -
as it is in most levels - the loader takes line 9's name instead
(`0x44c60a`); either way it cuts the name at the dot and appends `.glt`. Ten
files ship and all 26 levels resolve to one of them.

At level load, straight after the `.QKE`, `0x48bd60` runs over the level's
faces - its locals point at the chambers' texture words at `0x6bdfb4` - and
compares each face's texel, the low twelve bits, against every record's lit
and unlit index (`0x48bdb7`). A face wearing either is a light: the routine
projects it (`0x41adf0`, `0x41b0b0`) and adds it to a list counted at
`0x5cafe0`. Nothing in the level files marks a light; they are marked by what
they are painted with.

The eight numbers are not read out of the engine. The first is a whole number
of units in 16.16 - 2, 4, 6 or 10 - and the second is 90000 (1.373) or 65535
(1.0). The third and sixth are smaller and less regular, 2.0 down to 0.03.
The fourth is 6 in all 90 records and the fifth 1 in all but one. The seventh
is 0, 4 or 6, the eighth 1 to 8.

```
21 records  262144,90000,131072,6,1   32768,6,2
13 records  262144,90000,131072,6,1   32768,4,1
10 records  131072,90000,32768,6,1    32768,0,2
 5 records  655360,90000,131072,6,1   32768,4,2
 ...
```

`hb_formats::glt` parses all ten files.

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

What the eight numbers of a `.GLT` record set, and what puts a light out:
the unlit and broken textures are loaded and indexed and the load-time scan
finds the faces wearing them, but the code that swaps one texture for
another has not been read, nor has what the list at `0x5cafe0` is for. In
`.QKE`: what the motion line's five numbers mean exactly - a distance and a
rate fits the values but has not been read out of the engine - what the
flags line counts, and what the number after `!--Additional quake info--`
and the switch block's number select. The record shape of `.TTY`, which no
shipped level uses.

A ground quake and a box quake turn out to share their whole record shape;
only the third line differs, a rectangle of cells against one cell and a box
set, and only box entries carry the switch block.
