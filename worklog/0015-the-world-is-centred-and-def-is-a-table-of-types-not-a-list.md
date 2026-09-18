---
number: 15
title: The world is centred, and .DEF is a table of types not a list of things
date: 2026-09-17
area: format, world, content, port
files: crates/hb-formats/src/course.rs, docs/formats/courses.md, docs/formats/scenery.md
---

# 15. The world is centred, and .DEF is a table of types not a list of things

Setting out to place models in the world, I went looking for where an object's
position is written down. It is not where I assumed, and finding that out
settled the world's coordinate system instead.

## .DEF is a type table

[3](0003-the-lvl-manifest-and-five-sentinels-marking-five-revisions.md) called
`DATA\*.DEF` "enemy and object definitions" and left it at that. The obvious
reading of 83 records in `FLOAT` is 83 things standing in the level, and field 2
of a record's first line - a large, varied number like 782409 - is the obvious
place for a position.

It is not one. Field 2 is a function of the model: of the 250 models named
across all 1,848 records, 242 take exactly one value, and `wbunker.bin` takes
two across 439 records. A position is unique per instance; this is a property of
a kind. The display names agree - "Extra" 395 times, "none" 156 - which is a
fixed-size table with unused slots. The loader at `0x404fc5` caps it at 100 per
level, which is a table's shape, not a scene's.

So **where instances are placed is still unknown**. It is not in `.DEF`, not in
`.CRS`, and not in any per-level file I have read. The candidate I would attack
next is the spare byte at +0x11 of each [ground box cell](../docs/formats/terrain.md),
which nothing accounts for and which is exactly the right size for an index into
a 100-entry table. That would also fit what the levels look like: the buildings
in a Hellbender level *are* ground boxes, and a box that is a bunker needs to
know it is a bunker in order to be destroyed into `wbnkruin.bin`.

Recording the dead end is the point of writing it down. The next session should
not spend an hour on field 2 again.

## The world is centred on the origin

`.CRS` settled it. Across all 1,648 course points in the 24 levels that have
one:

```
x   -511.3 .. +511.0 units
y   -124.5 .. +159.8
z   -511.5 .. +511.8
```

1024 units square, which is 128 cells of 8.0 exactly as
[8](0008-half-the-world-was-in-the-wrong-hemisphere-and-the-diagonal-a.md)
worked out - and the origin is in the middle of it.

That retroactively explains the wrap. A cell index is the middle seven bits of
a coordinate, `(p >> 19) & 0x7f`, and a two's-complement coordinate masked to
seven bits folds -512..+512 onto 0..127 on its own: cells 0 to 63 are the
positive half, 64 to 127 the negative. The engine is not choosing to wrap. It
is not choosing anything; the arithmetic wraps because that is what masking
does, and the level design simply never approaches the seam.

`Cell::origin` in the port takes the index at face value, which is fine for a
renderer that places its camera the same way, and wrong for anything that has
to agree with a file's coordinates. `Cell::signed_origin` is the one that does.

## Three more formats named

**`.CRS`** has two variants sharing one container - 86 courses are a point list
with `groundCourse` and `periodic` flags, and four, all in `KREASH` and
`KREASH3`, are a list of segments that meet end to start. `crates/hb-formats`
parses both, and a segment course's shared endpoints are counted once: 16
segments across the four give 20 points, not 32.

**`.GLT`** is destructible lights: a count, then per entry three texture names -
`ZLTE1ON.RAW`, `ZLTE1OFF.RAW`, `ZLTE1BRK.RAW` - and two parameter lines.

**`.QKE`** is moving geometry, and the engine's own word for it is "quake". A
box quake entry names two sounds, and in `FLOAT` they are `1-0UDOOR.WAV` and
`1-0DDOOR.WAV`. So a box quake is a ground box that moves: a door or a lift.
`processBoxQuake: no match for watchBox found` says the system tracks which
cells are in motion, which it would have to for collision to stay right.

**`.TTY`** is the ground type list. Its name comes from the save side at
`0x41e0c0`, which opens `data\<stem>.tty` with mode `wt` and fails with
`"Unable to save ground type list"`. Every shipped one is `0\r\n`, so its record
shape cannot be read at all.

## The data has dangling references

Six levels name a course their own `.CRS` does not contain, and `SHIP` does it
50 times against a file with no courses in it. The engine has a diagnostic for
exactly this - `"Bad course ID for enemy"` - so it checks and carries on.

The test that found it started life as `a_def_course_reference_resolves`, which
is the kind of assertion that feels right and encodes an assumption about the
data rather than a fact about it. It now asserts the exact list of levels and
counts instead, so it is a measurement that will notice if the parser changes
under it.

**Still unknown:** where object instances are placed. What a `.CRS` segment's
`type` and `direction` are. Every numeric field of `.GLT` and `.QKE`.
