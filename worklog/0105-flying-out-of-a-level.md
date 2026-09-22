---
number: 105
title: Flying out of a level
date: 2026-09-21
area: decomp, port, flight
resolves: 104
files: crates/hb-sim/src/flight.rs, crates/hb-fly/src/main.rs, worklog/0104-how-a-level-starts.md
---

# 105. Flying out of a level

[[104]] found a scripted flight sequence at `0x45a2a0` and could not say
whether it was the way into a level or the way out. It is the way out.

## What settled it

`0x5125c8` is the flag the branch that calls it tests (`0x482268`), and
`0x4725cb` is where the flag is set:

```
if point.kind == 3                       ; a jump zone
   and distance < 0x49700000
   and ship.y > point.y
   and ship.y < point.y + 0x140000       ; within twenty units above it
then [0x5125c8] = 1
```

Kind 3 is the jump zone, which is what `hb-sim`'s mission already calls
`Outcome::Jumped`. So the sequence runs when the player flies into the zone
that ends the level, and `blast4.wav` seven eighths of the way through is the
jump itself rather than a crash.

That also answers what the second angle at `0x51255c` is. `0x40eb59` drives the
same two words from two keys out of the keyboard array, masking the first to
`0xffff` - a full circle. They are the camera's own yaw and pitch about the
ship, which is the look-around. The jump-out drives them instead of the player,
and its clock running to `0x20000` without a mask is the camera going round the
ship exactly twice.

## What it does

- roll to zero, and pitch to zero if it was nose-down
- a quarter of a turn a second, every frame: the ship's nose up, to
  `0xffffc100` - 88.6 degrees, reached in about a second - and the camera
  round the ship
- the throttle held at `0xffff`, full, every frame
- `blast4.wav` at the ship when the clock passes `0x1c000`
- three keyboard bytes checked each frame, any of them ending it early
- over when the clock passes `0x20000`, which is eight seconds

The engine runs its own little game loop for it - drawing the world, timing its
own frames - rather than threading it through the main one.

## In the port

`hb-fly` had three seconds of the result on the HUD and then the next level.
Now a jump flies out: `Flight::jump_out` takes the controls away, turns off the
auto-level, holds the throttle at full and pitches the nose up a quarter of a
turn a second to the same 88.6 degrees, while the camera swing is added to the
view the same way the four cockpit views are. `blast4.wav` at seven seconds,
any key to cut it short, and then the departure movie and the next level.

`hb-sim`'s `Ship` gains `pitch_by`, since the sequence turns the ship itself
rather than asking the flight model to.

**Still unknown:** whether anything animates the way *in*. Nothing around the
spawn at `0x471333` moves a camera, and the only thing in front of a level is
the arrival movie four of them name ([[101]]).
