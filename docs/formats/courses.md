---
title: The .CRS courses
status: solid
covers: DATA\*.CRS
worklog: 15
---

# The .CRS courses

The paths enemies follow. Line 15 of a [.DEF](level-text.md) record names one
by index, or `-1` for none.

CRLF text. A count on the first line, then that many courses, each preceded by
a line of 45 asterisks. The line after the separator says which of two variants
the course is - they share the container and nothing else.

## Variant one: a point list

86 of the 90 shipped courses.

```
*********************************************
numPoints,groundCourse,periodic
7,0,0
-13647448,-2070568,-7887759
-11759315,-484070,-6077826
...
```

`numPoints` points follow, each an x, y, z triple in 16.16 fixed point.
`groundCourse` is non-zero when the path is meant to be followed along the
ground and `periodic` when it loops back to its first point. Across the shipped
data both take only 0 and 1, but for `KREASH3`'s first course, whose header
line reads `26,27445719,24423740` - the count, then what look like a point's
x and z. The engine takes it as written: 26 points, and a non-zero `periodic`
that loops.

The loader (`0x49d7d0`, from the level load at `0x44c0e0`) reads each course
into a 1,812-byte record in the array at `0x5b3a20`: `groundCourse` at `+0x00`,
`numPoints` at `+0x04`, `periodic` at `+0x08`, and up to 150 points of twelve
bytes from `+0x0c`. The follower reads `+0x08` to decide between going round
and turning back; nothing the port has read looks at `+0x00`. A `.DEF`'s course
id indexes the array directly (`0x49da10`), so ids are 0-based, and `-1` is the
only one it refuses.

## Variant two: a segment list

Four courses, all in `KREASH` and `KREASH3`.

```
*********************************************
[Course 1] ID,numSegs,direction
0,7,0
*********************************************
Course type
0
Course start (x,y,z)
12309032,2229691,28092632
Course end
9236328,3017317,28092632
*********************************************
... numSegs of those
```

Each segment carries its own separator, a type, a start and an end. Segments
meet end to start, so a seven-segment course has eight distinct points.

The engine cannot read this form. Its loader skips two lines and scans three
numbers, so `0,7,0` becomes a course of **zero** points, and everything after
is read out of step. It never matters: nothing that follows a course names one
of these. `0x49d900`, which writes the point form back, has no segment case
either.

## The world is centred on the origin

This is the format that settles it. Across all 1,648 course points in all 24
levels that have a `.CRS`:

```
x   -511.3 .. +511.0 units
y   -124.5 .. +159.8
z   -511.5 .. +511.8
```

So the world is 1024 units square - 128 cells of 8.0, as
[the terrain](terrain.md) says - and its origin is in the middle, not at a
corner. A cell index is the middle seven bits of a coordinate, `(p >> 19) &
0x7f`, which folds -512..+512 onto 0..127: cells 0 to 63 are the positive half
and 64 to 127 the negative one. That is the same masking `heightAtGrid` and
`groundTriangleMidpoint` do, and it is why the grid wraps instead of having an
edge.

The y range is wider than the terrain's own 127.5 units because courses fly
over the structures on it.

## Dangling references

Six levels name a course their own `.CRS` does not contain:

```
hoth2    1 record     kreash2  3      netlvl1  9
netlvl2  9            netlvl3  9      roid2    1
ship     50
```

`SHIP`'s 50 all name course 0 and its `.CRS` holds none at all. None of this
matters to the engine: every one of those placements is of a class whose
routine never reads a course (see [the simulation](../engine/simulation.md)).
The seven course classes' 302 placements all name courses that exist.

## Unknown

What a segment's `type` and `direction` would have meant, since the engine
never reads either. What `groundCourse` is for: class 47 follows the ground
whatever it says.
