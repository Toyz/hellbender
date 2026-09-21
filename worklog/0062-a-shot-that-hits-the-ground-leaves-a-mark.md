---
number: 62
title: A shot that hits the ground leaves a mark
date: 2026-09-20
area: engine, port
files: crates/hb-fly/src/battle.rs, crates/hb-fly/src/main.rs
---

# 62. A shot that hits the ground leaves a mark

Reported from playing: missiles go through the ground. They do not - checking
that first was worth the five minutes. A missile fired straight down over
`FLOAT` from sixty units stops dead at the surface on frame 105 of 120, and
both the shot and the missile update have tested the world every substep
since they were written. At 64 units a second and four substeps a frame, a
missile moves 0.27 units between tests; there is nothing for it to tunnel
through.

What was missing was the evidence. The port killed the shot silently, so from
the cockpit a missile flies into a hillside and simply ceases - which looks
exactly like passing through it.

The engine explodes. The projectile impact at `0x476e22` calls the explosion
with `0xea60` - 60,000 in 16.16, so 0.9155 units - and *then* hands the point
to the quake code, which is the call worklog 51 followed from the other
direction. The missile's own impact does the same with 4.0 (`0x478b07`).
Those two constants are now `SHOT_BURST` and `MISSILE_BURST`, and every shot
or missile that meets the world leaves one, whoever fired it. The quake
trigger stays where it was, on the player's own shots only, because that
guard is on the trigger and not on the explosion.

## Shots could not be fired in a tunnel

Looking at that closure turned up a second thing. `solid` was:

```rust
(p[1] * 65536.0) as i32 <= grid.ceiling_of_solid(x, z)
```

`ceiling_of_solid` is the top of the ground and its boxes. Inside a chamber -
where the world is below zero and the ground is overhead rather than
underfoot - every point in the tunnel is under that surface, so every shot
fired in one died on its first substep. The ship's own collision had this
right since worklog 43 and the weapons never got the same treatment.

It now tests what the flight tests: below zero in a chamber cell, a point is
solid when it is under the chamber's floor or over its ceiling, and the
ground overhead does not come into it.

**Still unknown:** whether the engine's explosion sizes are the whole story -
`0x476f90` takes two more arguments after the size, 1 and 2 for a shot and 2
and 0 for a missile, and what they select is not read. There are 30 call
sites and they do not agree on them.
