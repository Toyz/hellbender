---
title: The simulation
status: partial
covers: HELLBEND.EXE logic phases, crates/hb-sim
worklog: 26, 27
---

# The simulation

What moves in a Hellbender level, and how much of it has been read. Most of it
has not; this page is the map of what is known so far.

## Actors follow courses

A type in a level's [`.DEF`](../formats/level-text.md) names a course on its
line 15, and every placement of that type follows it. Across the 26 levels,
1,560 placements belong to a type that names a course and 1,301 of those
courses exist - the other 259 are the dangling references described under
[courses](../formats/courses.md).

The engine's logic routines are named by their own diagnostics:

```
logicFollowPath                 logicFollowPathAttitude
logicFollowGroundPath           logicPathDogfight
logicTransportDisappear         logicTransportTakeoffLand
logicTransportTakeoffLandLeave
```

Each is a phase machine over the actor's `+0x64` field, and each checks its
course the same way before doing anything:

```
course = courses[actor+0x244]      "Bad course ID passed to ..." if negative
                                   "Bad course ID for enemy"   if missing
                                   "No course points for course" if empty
```

## Joining a course

Phase 0 of the follow logic at `0x00421240` sets a best distance of
`0x40000000`, walks every point of the course, and keeps the nearest. So an
actor **joins its course at whichever point is closest to where it stands**,
and does not have to be placed on it.

That explains a measurement that otherwise makes no sense: only 202 of the
1,301 course-following placements sit within one unit of their course, and the
median is 460 units away. They are meant to fly to it. The transport logic's
names - take off, land, leave - say the same thing: a transport starts on a pad
and goes to its route.

## What this port does

`hb-sim` reproduces phase 0 exactly and then follows the course point to point.
Everything past phase 0 is its own choice:

| choice | why |
| --- | --- |
| 20 units a second | The engine's speed source has not been read. A type's `.DEF` line 1 has a field that reads as 15 to 30 units a second in half the records and is zero in the other half. |
| Straight lines between points | The engine has a `"Curve parameter calculation failed"` diagnostic, so it probably fits curves. |
| A periodic course loops; a plain one stops at its last point | What the engine does at the end of a non-periodic course is not known. |
| All seven logic routines treated as "follow" | Only the follow logic's first phase has been read. Transports, dogfighting and attitude-following all behave alike here. |

The heading an actor faces is `atan2(dx, dz)` of its direction of travel, the
same convention the recorded demo flight measured.

## Shooting

What comes from the data, per type in the level's `.DEF`:

- **What it becomes when destroyed** - the second model on line 0. Seven
  distinct values across the 1,848 types: `wbnkruin.bin` for the 439 bunkers,
  five dome ruins `krdom0r.bin` to `krdom4r.bin`, and `cube.bin` for the other
  1,394. `cube.bin` is the un-normalised test model, and this port reads it as
  the exporter's placeholder for "nothing left" - an inference, but one that
  makes a building vanish rather than turn into a cube.
- **The sound it makes** - line 24, the second name under
  `{ Escape and destroy sound files`. 143 of them resolve to a `.WAV` in the
  archives. One type names `CRY-DES.DEF` where it means `CRY-DES.WAV`, a typo
  that shipped; it resolves to nothing.
- **Its damage multipliers** - lines 17 to 19, cannon, laser and missile, in
  16.16. 1.0 in almost every type.
- **Its hit points, probably** - field 2 of line 0. It is a function of the
  model, which is the shape hit points should have, and read as 16.16 it
  orders sensibly: a cube 0.55, a bunker 4.65, a control centre 11.9. That is
  an inference from the data, not a reading of the engine.

What is this port's own choice:

| choice | why |
| --- | --- |
| A shot travels 180 units a second and lives 1.4 seconds | Not read. |
| A laser hit does 1.0 before the type's multiplier | Not read. |
| Ten shots a second while the button is held | Not read. |
| A placed object is hit as a sphere of radius equal to its scale | Models are normalised to +/-1.0, so the scale is the half-extent. The engine's own test has not been read. |
| A shot is drawn as a point | The engine's is a model - `bullet.bin` is in STARTUP.POD. |

The fire button is the space bar, because `HELLBEND.INI` binds `fireKey=57`,
which is the space bar's scan code - and 636 of the 638 key presses in the
recorded demos are it.

## Unknown

Everything past phase 0: speeds, turning, curve fitting, what happens at the end
of a course, and how the seven logic routines differ. Whether field 2 really is
hit points. The engine's weapons - speeds, damage, the fourteen weapon slots in
the powerup table. What makes an actor start moving - whether all of them move
from the start of the level or some wait for the player. Anything shooting
back.
