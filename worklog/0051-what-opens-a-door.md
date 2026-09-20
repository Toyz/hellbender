---
number: 51
title: What opens a door
date: 2026-09-20
area: decomp,engine
files: crates/hb-formats/src/quake.rs,crates/hb-formats/tests/against_the_game.rs,docs/formats/scenery.md
---

# 51. What opens a door

Worklog 47 read the `.QKE` file out of its reader and writer and left three
things open: what the motion line's five numbers mean, what the flags line
counts, and what the two trailing numbers select. All three are answered by
the routine that runs the records, and the answer to the last one is the
answer to the diagnostic the file is famous for.

`0x4122a0` runs once a frame over both lists, skipping any record whose kind
byte is zero, and hands each live box entry to `processBoxQuake` at
`0x410ec0`. That reads the record's cell out of box set A or B - eighteen
bytes a cell, `(row << 7) + column` - and dispatches on a state byte through
a six-way jump table:

```
0  resting     if it is a switch, look for the box that watches it
5  about to     wait, then play the first sound and start moving
1  moving out   step the cell's two altitudes until the first height is met
2  holding      wait out the motion line's third number
3  moving back  the same the other way, playing the second sound
4  holding      wait out the motion line's fifth number
```

Both altitude words step by the same amount, so a box keeps its thickness and
translates. The step is computed fresh each frame by `0x410e40`: the distance
still to travel, divided by the motion line's second number divided by the
frame time. That is the giveaway. **The two numbers are durations, not
rates** - a door takes 2.0 seconds to open whether it lifts four units or
forty - and the third and fifth numbers are the pauses at each end. The
first says which of the box's two altitudes is compared against which of the
entry's two heights.

## The switch and the door

319 of the 1,193 box quakes carry a `@--Box quake switch info--` number of 1.
Those are switches. The block's texture name is uppercased and resolved to a
texture index at load, and when the switch is thrown the engine writes that
index into the cell's texture word - the switch lights up.

Then it has to find the door. `0x410d00` walks the box quakes for one that is
resting and whose flags say what it watches: with a 4 in the flags line's
fifth place it watches an **id**, and its watch line's first number is that
id, matched against the switch's own `!--Additional quake info--` number.
With a 3 it watches a **cell**, and the watch line's first three numbers are
that cell. The index is cached in the record, and when the walk finds nothing
the engine prints `processBoxQuake: no match for watchBox found`.

So the message means a switch with no door, and the level data says how often
that happens: 263 of the 319 switches name an id that some entry watches, and
the other 56 do not. Only the id form is used - 643 live entries carry a 4,
none carries a 3.

A test holds all of it: the switch-to-door count, that watching is by id and
never by cell, and that every duration in every entry is a whole or half
second and no longer than twelve. `hb_formats::quake` grew `is_switch`,
`watches`, `delay` and `timing` so the port can drive the records without
re-deriving any of this.

What is still unread is what throws a switch in the first place - the
transition out of the resting state is there, but not the thing that calls
it - and what the ground quake's two kinds do differently.
