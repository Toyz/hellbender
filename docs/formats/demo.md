---
title: The .DMO recorded flight
status: solid
covers: DEMO\DEMO1.DMO, DEMO\DEMO2.DMO, DEMO\DEMO3.DMO, HELLBEND.EXE:0x44d180, 0x44d470, 0x4558e0
worklog: 25, 123
---

# The .DMO recorded flight

Three attract-mode demos under `STARTUP\DEMO\`. CRLF text: a record count, the
level it was recorded on, then that many tagged records.

## Layout

```
1025                           record count
iowah2.lvl                     the level
0,0                            tag 0, time 0          a pose
12845056,2686976,7077888         x, y, z
0,0,16384,0                      pitch, roll, heading, flag
0,2131                         tag 0, time 2131
...
1,232086                       tag 1, time 232086     a key press
57                               scan code
```

A record's first line is `tag,time`. Tag 0 is followed by a position line and
an angle line; tag 1 by one line holding a key's scan code.

All three parse consuming every line, with exactly as many records as the first
line declares and the time never decreasing:

```
DEMO1.DMO   iowah2.lvl     1025 records   890 poses   135 keys   50.8 s
DEMO2.DMO   red.lvl        1025 records   693 poses   332 keys
DEMO3.DMO   atmos-t2.lvl    811 records   640 poses   171 keys
```

## Fields

**time** is 16.16 seconds: `DEMO1` runs to 3,331,473, which is 50.8 s. The
player's clock (`0x505130`) is zeroed at the level's start (`0x44cec0`) and
advanced by the frame time after each frame (`0x44d48c`), and poses are
compared with it directly.

**x, y, z** are a world position in the world's signed, centred coordinates -
see [terrain](terrain.md).

**The angles** are in the engine's 16-bit circle, and two of them are measured
against the recorded motion itself:

- The **third** is the **heading**. It equals `atan2(dx, dz)` of the direction
  the ship travels between poses with a median error of 0.8 degrees over 790
  samples; every other pairing is 60 to 135 degrees off. So heading 0 points
  along +z and the heading increases toward +x.
- The **first** is the **pitch**, and **positive is nose down**: its sign is
  opposite to the climb in all 637 samples steep enough to test, and its
  magnitude matches the climb angle to a median 0.8 degrees.
- The second is the **roll**: the player writes it to the pose's `+0x10`
  (`0x5b3840`).
- The fourth is the **fire button**: 1 while it is held. The player writes it
  into the key array at `fireKey`'s scan code (`0x44d3f4`).

**Scan codes**: 57 is the space bar, which `HELLBEND.INI` binds as `fireKey`,
and accounts for 636 of the 638 key presses. The other two are a single 2 (the
`1` key, `keyDispersionCannon`) and a single 59 (F1).

## Playing one back

The attract mode (`0x4558e0`) always loads `demo1.dmo`, sets the attract flag
`0x512630` and the playing flag `0x50512c`, and runs the level. With the
attract flag set the level start skips its whole introduction - the briefing,
the arrival, and the opening camera (`0x4815e1`, `0x481693`).

Each frame `0x44d180`:

- finds the first **pose** whose time is past the clock; with none, "Demo
  Play Done" and playback stops
- takes the pose before it - or the first record at time zero - and puts the
  player between the two: each value the earlier one's plus the difference
  times the time since over the time between, in integers (`imul`, `idiv`),
  the three angles the short way round (`shl 16; sar 16`) and back into the
  circle
- rebuilds the ship's orientation from the angles (`0x464800`)
- clears the key array, and holds `fireKey` if the later pose's fourth value
  is set
- when the pose it plays toward has moved on, presses the key records
  between the two before it (`0x505138`, `0x50513c`) - so a key press lands a
  pose late

Everything else runs as in play: the actors think and shoot, and the
player's shots fly.

## Two demos were recorded on levels that did not ship

`DEMO2.DMO` names `red.lvl` and `DEMO3.DMO` names `atmos-t2.lvl`. Neither is in
GAME.POD. They are development levels, cut before release, and the demos
recorded on them shipped anyway - the attract mode can only have played
`DEMO1`.

## Ground truth for the terrain

A recorded demo is the original engine's own motion, so it is a check on the
port's world that no static file can give. In `DEMO1`, **none of the 890 poses
is below the port's reconstructed `IOWAH2` ground**, and the closest approach is
about five units.

That would be weak on its own, so the same check runs against deliberately
wrong terrain:

```
the port's reading                  0 of 890 underground
twice the height scale            265
x and z swapped                    99
HOTH's terrain instead            746
```

So the demo confirms the height scale, which axis is which, and the centred
coordinate system all at once. It does **not** confirm the triangle split - a
uniform diagonal also passes, because the pilot never flies close enough to a
cell's surface for the two to differ.

## Unknown

What the engine does after "Demo Play Done" - it returns to the attract
screen, which is not read; the port plays the demo again. Whether a demo
flight can be killed: the actors shoot at it as at any player, and the port
keeps it alive.
