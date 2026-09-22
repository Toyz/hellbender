---
number: 113
title: The cars drive their courses, and nothing else does
date: 2026-09-22
area: decomp, port
files: crates/hb-sim/src/course.rs, crates/hb-sim/tests/course.rs, crates/hb-fly/src/main.rs, crates/hb-fly/src/battle.rs, docs/engine/simulation.md, docs/formats/courses.md
---

# 113. The cars drive their courses, and nothing else does

Worklog 26 read phase 0 of `0x421240` and the port made up the rest: fly to
the nearest point at 20 units a second, slide from point to point, stop at the
end. And it gave a course to every placement whose type named one. With
`tools/funcs.py` the whole routine is 338 bytes and one callee, so it was
time to read the rest.

## Who reads a course

The jump table at `0x40c6cc` sends each class to its routine, and seven of
those routines fetch a course through `0x49da10`: classes 46, 47, 48, 50, 51,
52 and 62. No other class ever looks. A census of the 26 levels:

```
placements whose type names a course, by class
  0: 576   3: 9    7: 239   10: 125   14: 6   17: 4   35: 9
 47: 285  50: 3   51: 3     52: 10    53: 205 55: 8   59: 25  60: 52  62: 1
```

The port was moving the 576 scenery objects and 125 turrets along courses the
engine never reads for them. Only 302 placements follow a course, and every
course those 302 name exists - the dangling ids courses.md worried about all
belong to classes that do not look.

## Class 47, all of it

285 of the 302. Phase 2, which 26 had not read, puts the actor **on** the
nearest point - the position becomes the point's. Nothing flies to its
course; everything is on it by its third frame. Phase 3 is `0x4231b0`:

- aim at the target point (`0x423b60`, `atan2` of the world-wrapped
  difference, times 65536/2pi - the constant at `0x4eb198` is 10430.378)
- within eight units in x and in z, take the next point
- ease the heading toward the aim at the type's turn rate
- move along the heading it *had*, at the type's move rate - so it drives
  curves between points
- y is the floor under it (`0x41c300`, sampled after x moves and before z
  does) plus the type's `+0x14`, zero for every class 47 type

At an end, a periodic course (record `+0x08`, which the loader at `0x49d7d0`
fills from the header's third number) goes round; a plain one folds the index
back and flips `+0x144`, so it turns round and comes back. The placement
loader sets `+0x144` to 1 (`0x4059da`), forward.

`0x404e90`, run by the loader over every actor with its index, adds a small
per-actor bias to speed and turn - `(index << 30) >> 16`, which is 0, +1/4,
-1/2 or -1/4 of a unit a second. That is why a column of identical cars on one
course does not move as one.

The "Curve parameter calculation failed" diagnostic that made 26 think the
engine fits curves is in `0x423e60`, and nothing calls it. The curves the cars
drive come from steering.

## It shoots, too

`0x421240` ends by calling `0x407770` with 32. That is not a shot speed: the
routine accumulates frame time into `+0x68`, and past the type's fire interval
fires if its third argument is under 40 units. 32 always is. So a class 47
fires every interval, along its own heading, when the player is ahead - the
same `0x406dc0` a turret uses. The T-rexes, the morbots, the Kraaken and the
`ROID` loaders all have weapons; the cars have a zero shot speed, which the
shot code treats as no shot. Classes 46, 48 and 62 end the same way; the
transports do not.

## The course file

The loader reads `groundCourse` into `+0x00`, `numPoints` into `+0x04` and
`periodic` into `+0x08` of a 1,812-byte record, 150 points at most. It only
knows the point form: a segment course from `KREASH` would load with zero
points. Nothing that follows a course names one, so it never shows.
`KREASH3`'s first header is `26,27445719,24423740` - mangled, but the count is
right and the rest reads as looping.

`hb_sim::course` is the routine in 16.16, and `hb-fly` gives followers only to
the course classes and triggers for the four that shoot. The test runs all 302
for a simulated minute and checks each stays within 50 units of the leg it is
driving.

The same setup routine starts every actor's fire timer at `rand() & 0xffff`
(`+0x68`), somewhere in the first half second. The port had every gun start
at zero, so a row of SAM sites fired as one; now turrets, followers, flyers
and hover craft each start somewhere in that half second.

**Still unknown:** the other six course routines. The port runs the
transports and `FX4` (17 placements) on class 47's.
