---
title: The .GLT lights, .QKE moving geometry and .TTY ground types
status: partial
covers: DATA\*.GLT, DATA\*.QKE, DATA\*.TTY
worklog: 15, 47, 48, 91
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
-1,-1,0,0                             what it watches: an id, or a cell
1,65536,0,65536,0                     motion: which altitude, the seconds
                                      out, the pause, the seconds back, the
                                      pause
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
  heightfield. `0x4121d0` walks the rectangle with `& 0x7f` at each step, so
  a patch may wrap around the world's edge. See below.
- **Box quakes**, 1,193 of them, name one cell and which of the two box sets
  it moves - 747 move set B and 446 set A. 813 of them name at least two
  sounds, and those sounds are `1-0UDOOR.WAV` and `1-0DDOOR.WAV` and their
  like: a door going up and a door going down. The rest move silently.

### The box quake state machine

`0x4122a0` runs once a frame over both lists, skipping any record whose kind
byte is 0, and hands each live box entry to `processBoxQuake` (`0x410ec0`).
That routine finds the record's cell - `(row << 7) + column` into box set A at
`0x675fa0` or set B at `0x6edfc0`, eighteen bytes a cell - and dispatches on a
state byte through a six-way jump table at `0x411474`:

```
0  resting     if it is a switch, look for the box that watches it
5  about to     wait out the watch line's fourth number, then play the first
               sound and start moving
1  moving out   step the cell's two altitudes, until the first height is
               reached
2  holding      wait out the motion line's third number
3  moving back  the same the other way, playing the second sound
4  holding      wait out the motion line's fifth number
```

Both of the cell's altitude words move by the same step, so the box keeps its
thickness and translates; the record remembers where it is at `+0x44` and
`+0x48`. The step comes from `0x410e40`: the travel, divided by the number of
frames the move is allowed, which is the motion line's second number (going
out) or fourth (coming back) divided by the frame time. **So those two
numbers are durations in seconds, not rates** - a door takes 2.0 or 3.0
seconds whatever its height - and the third and fifth are the pauses at each
end. The travel itself is the first height less the second less the box's
thickness, so it is the same both ways.

The two heights are where the box's two edges end up: moving out raises the
**top** until it reaches the first height (`0x411026`), and moving back
lowers the **bottom** until it reaches the second (`0x4111c2`). The shipped
levels park a box at one end or the other - `FLOAT`'s doors sit with their
bottom at the second height and rise, while some of `HOTH`'s sit with their
top already at the first, so their first move arrives at once and what they
really do is close. The motion line's first number is not part of this: it
only picks which edge the switch code compares when it asks whether a box
has moved (`0x410fb9`).

### The ground list

Same machine, a rectangle of cells rather than one box. `0x4121d0` walks the
rectangle and hands each cell to `0x411b80`, which runs the entry through the
same six states off its own jump table at `0x4121b0`. Every cell steps by the
same amount, so the patch keeps its shape, and the travel is simply the first
height less the second - a cell has one height, so there is no thickness to
take off.

The where line's **fifth number is the layer**: 1 the ground, 2 a chamber's
floor, 3 its ceiling (`0x411ba3`, which indexes the cell into `0x73bcc0`,
`0x6bdfb0` or `0x6bdfb2`). Across the 26 levels 290 entries move the ground,
126 a chamber floor and 323 a chamber ceiling.

A ground entry's flags line has four numbers rather than five, so everything
shifts down one: the first is the mode byte, the second and third are bits,
and the **fourth** is the four-bit field that says what the entry watches.
That is why no ground entry is shot open - the field is 4 or 0 in every one
of them, never 1 - and it is where the 320 entries that never stop come from:
with the mode byte at 1 and the first bit set, the resting state puts the
entry straight back into its cycle (`0x411c45`), so it runs up and down for
as long as the level lasts. The rest wait for a box to move.

The kind byte picks the mover. **1** is the one above, 737 of the 739 live
entries. **3**, one entry, reads the ship's own cell and whether it is above
or below zero first (`0x411548`) and moves relative to that. **2**, one
entry, matches neither and is skipped.

`hb_sim::quake` runs both lists, and `hb-fly` writes the moved cells back
into the terrain.

### Switches and the boxes that watch them

A box quake with a `@--Box quake switch info--` number of 1 - 319 of the
1,193 - is a **switch**. Its switch block names two textures, lit and unlit.
The engine uppercases both, resolves them to texture indices at load, and
writes one into all four of the cell's side faces as the switch goes on and
off (`0x412a50`, `0x412ab6`). What decides which is the position of the box
the switch points at: while that box is away from where it rests, the switch
is lit.

The switch then has to find the door it opens. `0x410d00` walks the box
quakes for one that is resting and whose flag field says what it watches:

- flags line's fifth number 4: it watches an **id**, and the watch line's
  first number is that id, matched against the number after
  `!--Additional quake info--` on the switch. The ids in the shipped files
  run to about 30, with `-1` for none.
- flags line's fifth number 3: it watches a **cell**, and the watch line's
  first three numbers are that cell's row, column and box set. No shipped
  entry uses this - the 1,192 live box entries hold 4 in 643 of them and 0,
  1 or 2 in the rest, and those three match nothing here.

The index it finds is cached in the record, and when it finds nothing the
engine prints `processBoxQuake: no match for watchBox found`. That is what
the message means: a switch with no door - and 56 of the 319 switches are
in exactly that state, since only 263 of them name an id that some entry
watches.

The flags line's other numbers become bits: the second, third and fourth
become one bit each where the value is 1, and the fifth is the four-bit
field above, shifted up three. The first number becomes a mode byte, which
the resting state tests against 1.

### What starts one

Shooting it. A projectile's impact calls `0x410b80` with the point it hit,
and that asks the box list (`0x4107a0`) and the ground list (`0x4108a0`) for
anything there. A box quake answers if it is resting, if the flags line's
fifth number is 1 - 422 of the live entries - and if the point is between the
altitudes it is currently at; the entry then goes to the about-to state with
its timer cleared, and the cycle above runs. Both call sites guard on the
shot belonging to the player whose id is in `0x503c68`, which is 0 in a
single-player game.

A moving box quake pulls others along with it. While it steps, it looks for
the ground quakes that watch it - by cell or by id - and puts each of them
into the about-to state as well (`0x410da0`), so one shot can move a door and
the ground under it together.

Two more ways in exist and are dead: `0x410bc0` and `0x410c10` start every
entry whose flags line's fifth number is 2 and whose watch id matches a
number passed in - 60 live entries are waiting for that - but nothing in the
shipped executable calls either. So the fifth number is a taxonomy: 1 is
shot, 2 is called by id and never is, 3 watches a cell, 4 watches an id.

`hb_formats::quake` parses all 26 files to their last line.

## .TTY - the ground type list

Every shipped `.TTY` is the three bytes `0\r\n` - a count of zero and nothing
else. Its name comes from the save side: `0x41e0c0` replaces the level's
extension with `.tty`, opens it in `data` with mode `wt`, and fails with
`"Unable to save ground type list"`.

So the format exists, the editor writes it, and no shipped level uses it. Its
record shape cannot be read from the data.

## Unknown

What the eight numbers of a `.GLT` record set, and what puts a light out.

What is known of the second: the records live at `0x5d0628`, 92 bytes each,
counted by `0x5d05e0`, and the test "is this texture a light" is written out
by hand in eight places rather than called - each one masks a texture word to
twelve bits, walks the array comparing `+0x00` and `+0x04`, and answers 1 for
lit, 2 for unlit, 0 for neither. `+0x08`, the broken texture, is not compared
in any of them.

The load-time scan is `0x48bd60`, called once, from the level load at
`0x44c729`, and it hands each face it finds to `0x41bda0`. So the lights are
found when the level loads and something is placed at each one. What swaps
another has not been read, nor has what the list at `0x5cafe0` is for. In
`.QKE`: what the flags line's first number - the mode byte the resting state
tests - selects beyond 1, and what a kind 3 ground entry does with the ship's
cell. The record shape of `.TTY`, which no shipped level uses.

A ground quake and a box quake turn out to share their whole record shape;
only the third line differs, a rectangle of cells against one cell and a box
set, and only box entries carry the switch block.
