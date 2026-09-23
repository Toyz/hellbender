---
number: 122
title: The transports, and the one that loses you FLOAT
date: 2026-09-22
area: decomp, port
files: crates/hb-sim/src/transport.rs, crates/hb-sim/tests/transport.rs, crates/hb-sim/src/course.rs, crates/hb-sim/tests/course.rs, crates/hb-sim/src/steer.rs, crates/hb-sim/src/mission.rs, crates/hb-formats/src/text.rs, crates/hb-render/src/level.rs, crates/hb-fly/src/battle.rs, crates/hb-fly/src/main.rs, docs/engine/simulation.md, docs/formats/level-text.md
resolves: 113
---

# 122. The transports, and the one that loses you FLOAT

113 left six course routines unread and ran the 17 placements that use them
on class 47's ground driving. With the steering read (121), the three
transport routines are short: `0x421d90` (class 50), `0x4220f0` (51) and
`0x422790` (52). Sixteen of the 17 are transports; the seventeenth is `FX4`,
class 62, which stays on class 47's routine.

## What they do

All three fly with the steering at a thrust factor of a half and move on
from a point once they have passed it. `0x423ca0` is that test: the
position's projection on the leg from the previous point, against the leg's
length.

Class 50 - the Rishi in `IOWAH2`, the Coalition shuttle and transport in
`IOWAH3` - flies to its nearest point, follows the course, and at the last
point is gone: `+0x1c` cleared, a burst (`0x4017d0`), and its escape sound
through `0x454880`. The sound is type `+0x258`, the first of the two names on
the `.DEF` record's `{ Escape and destroy sound files` lines, which the port
checked the header of and never read. The Rishi's is `rshi-saf.wav`.

Class 51 takes off - four units a second up, a sixteenth of a turn a second
round, four seconds - flies the course, brakes near its end with the steering
at speed zero while turning toward a point beside the player, comes down,
turns round, waits four seconds and takes off again. And then it goes
nowhere. Its target is still the last point; the leg `0x423ca0` measures runs
from the target plus one, folded back by the same arithmetic as `0x423af0`,
which on a plain course is the same point; `0x487770` normalises the empty
leg with no guard, the NaNs make the comparison false, and the leg is never
passed. All three shipped class 51 courses are plain, so each lands once and
then circles its last point for good. The test runs all three for twenty
simulated minutes and they do exactly that.

Class 52 is 51 with an ending. It reaches its first point with `0x407960`,
the simpler flying several routines use (heading and pitch closing on the
wanted ones by the turn rate as a fraction a second, the roll minus half the
heading's step), lands only at its last point, and after four seconds on the
ground climbs to the sky layer. Within eight units of it, it is gone, it plays
`trp-esc.wav` - "Transport has escaped." - and `0x422ec7` writes 1 to
`0x512720`, the flag the mission reads as lost. `FLOAT`'s and `FLOAT2`'s
"Shipping out!" transports are the level: let one reach the sky and it is
over.

## In the port

`hb_sim::transport` is the three routines; `hb_sim::course` now holds what
every course routine shares - the walk from point to point, the nearest
point, the aim (`0x423b60`, which is the shared `angles_of` and
`over_the_top`) and the passed test - and class 47's follower uses it too.
`Body::glide` is `0x407960`. `EnemyDef` carries both sound names, and the
level no longer re-reads line 24 for itself. A transport that leaves is
taken out of the fight as well as the picture - the engine's `+0x1c` is the
hit points, so no shot tests it afterwards - and the class 17 ships that
leave now go the same way. `Mission::fail` is the one way to set the lost
flag.

The test runs every shipped transport on its own level's terrain: the ten
class 52 escape, the three class 50 leave, the three class 51 land once and
circle.

**Still unknown:** the burst (`0x4017d0`) and the particle effects around
it at `0x401000`-`0x402000`, which the port does not draw, so a transport
that leaves simply vanishes; `FX4`'s routine; what `0x407960`'s last
argument, 1 here, is for - it is never read.
