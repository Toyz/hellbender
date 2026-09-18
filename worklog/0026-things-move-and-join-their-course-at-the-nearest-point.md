---
number: 26
title: Things move, and join their course at the nearest point
date: 2026-09-17
area: engine, decomp, port
files: crates/hb-sim, crates/hb-fly/src/main.rs, docs/engine/simulation.md
---

# 26. Things move, and join their course at the nearest point

The levels have been still until now. 1,301 placements across the 26 levels
belong to a type whose `.DEF` line 15 names a course that exists, and those now
move. In `JURASIC` that is 214 of the 406 objects.

## A measurement that made no sense

Before writing any movement I checked the obvious thing: are these objects
placed on their course? Mostly not. Only 202 of the 1,301 sit within one unit
of it. The median distance is **460 units** - nearly half the world.

That looks like a misreading - of the course id, of the coordinates, of which
course belongs to which type. It is not. It is the design.

## Phase 0 picks the nearest point

The follow logic at `0x00421240` is a phase machine over the actor's `+0x64`.
It validates the course the way every logic routine does - `+0x244` is the
course id, and the three "Bad course ID" and "No course points" diagnostics
come from here - and then phase 0 does this:

```asm
mov  [esi+0x64], 2              ; next phase
mov  [esp+0x10], 0x40000000     ; best distance so far: huge
lea  ebp, [edi+0x14]            ; first course point
  ... walk every point, keep the nearest ...
```

An actor joins its course at **whichever point is closest to where it
stands**. It does not have to be placed on the course and is not expected to
be. The transport logic's own names - `TakeoffLand`, `TakeoffLandLeave`,
`Disappear` - say the same: a transport starts on a pad and flies to its route.

So the 460-unit median is a measurement of how far the level designers put
things from the routes they were given, and it is exactly what the engine's
first phase exists to handle.

## hb-sim

A new crate, no dependencies, and small on purpose. `Follower` reproduces
phase 0 - the nearest point, chosen exactly as the engine chooses it - and then
walks the course point to point. Everything past that is the crate's own
choice and `docs/engine/simulation.md` lists each one: a speed of 20 units a
second, straight lines where the engine probably fits curves (it has a
`"Curve parameter calculation failed"` diagnostic), and looping only when the
course says it is periodic.

The heading an actor faces uses the convention the demo flight measured in
[25](0025-the-original-game-s-own-flight-and-the-four-bugs-it-found.md). There
is a test that a course running along +x produces a heading of `0x4000`, which
would have failed before that entry.

The shipped-data test steps every one of the 1,301 followers through a minute
of simulated time and checks nothing goes non-finite or leaves the world.

## In the window

`hb-fly` keeps a live copy of the placements, rewrites the moving ones from
their followers each frame, and hands that copy to the renderer - so the
renderer still knows nothing about simulation, the same way it knows nothing
about animation. 60 frames a second in a release build with 214 things
moving.

**Still unknown:** everything past phase 0 - how fast each type goes, how it
turns, whether it fits curves, what it does at the end of a course, and how the
seven logic routines differ. What starts an actor moving: whether all of them
go at once or some wait for the player.
