---
number: 92
title: A file no level uses, read from its writer
date: 2026-09-21
area: decomp, format
files: docs/formats/scenery.md, worklog/0091-the-lights-are-found-at-load-and-never-broken.md
---

# 92. A file no level uses, read from its writer

Two things, and the first is a correction to [[91]], made an hour after it.

## `0x41bda0` is not a light routine

[[91]] said the load scan "hands every light face to `0x41bda0`, so the
lights are found once and something is placed at each one". The second half
was a guess dressed as a finding, and reading the routine says otherwise.

It takes two packed world positions, masks each with `0x3f80000` and shifts
down nineteen - seven bits, 0 to 127, which is the terrain grid - and walks
the cells between them, three values a cell. Four other places call it, none
of them about lights. It is a segment-to-cells walk.

So the scan is registering the **cells** a light face spans, into the list it
clears at `0x5cafe0`. Nothing is placed anywhere. What writes a light's unlit
or broken index back into the terrain is still not found, which is the same
place [[91]] left it, only without the invented object.

## `.TTY` was readable all along

Every shipped `.TTY` is three bytes - `0\r\n`, a count of zero - so [[47]]
wrote "its record shape cannot be read from the data" and stopped. True, and
beside the point: the engine **writes** these files, and a writer states a
format as plainly as a reader does.

`0x41e0c0` opens `data\<level>.tty` with mode `wt`. `0x41e139` prints the
count from `0x6edfb0` with `"%d\n"`. Then it walks an array of twenty-byte
records from `0x7367c2` and prints each with `"%s,%d\n"` - the name at
`+0x02`, a `u16` at `+0x00`.

```
2
SOMETHING.RAW,3
SOMEWHERE.RAW,1
```

A count, then a name and a number a line, in the same house style as every
other one of these files. What the name and the number *mean* is still open -
a texture and a ground type is the obvious reading and the file calls itself
a "ground type list" - but that is a different question from what shape the
records are, and the shape is settled.

**Still unknown:** the two fields' meaning, which nothing shipped exercises;
and everything [[91]] left on the lights.
