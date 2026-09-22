---
number: 117
title: The lock is a box or a diamond, and the objective has a health bar
date: 2026-09-22
area: ui, port
files: crates/hb-render/src/hud.rs, crates/hb-render/tests/marks.rs, crates/hb-fly/src/main.rs, docs/engine/hud.md
---

# 117. The lock is a box or a diamond, and the objective has a health bar

`hb-fly` has drawn four corner brackets on the missile lock since worklog 38,
with a comment saying the engine's lock display was not read. The audit in 112
listed `drawTargetingBox3d` (`0x47c4b0`) and `drawTargetingDiamond3d`
(`0x47c910`) among the biggest live routines nobody had cited. They are that
display.

## Two marks, not one

The frame calls `0x47b700` after the weather. It marks the lock through
`0x47c2d0` in `0x97` at size 17, and then calls `0x472ae0`, which marks the
current objective: it reads the current nav point (`[0x6283bc]`, records of 256
bytes whose kind is at `+0xc`) and switches on the kind through a byte table.
Destroy marks the first of its targets still standing, the single-target
Destroy its one target, both in `0x8f`; Escort marks the escorted ship in
ramp 2 green. Size 20, and a health bar. So a player always sees what the
mission wants destroyed, not only what they have locked.

## What a mark is

`0x47c2d0` projects the actor and picks by `0x40dc00` - the radar's target
test - a box for a ground target and a diamond for anything else. Both draw
three outlines, black, the colour, black, one pixel apart. The half-size is
the size given halved at 320 across or 200 down: the lock's box is 17 pixels
across at 320x200 and 35 at 640x480. The health bar under an objective is
hit points now over hit points at the start, green above half, `0x8f` above a
quarter, `0x97` below.

The box routine takes a flag for the health bar and another for a player's
name; the lock passes neither, the objective passes the first, and only the
network game's `0x47bda0` passes the second.

## An aside the tools caught

`0x47b700`'s body has a hole at `0x47b8c3`, which `tools/funcs.py` lists as a
separate unreached function. A brute-force search finds one branch into it,
from inside itself; the code before it ends in a `jmp`. It is a dead loop in
the middle of a live routine, and the port has nothing to do with it.

**Still unknown:** what `0x12d` is as an actor phase - `0x472b48` skips a
target in it as though dying - and whether the port's destroyed flag and it
ever disagree.
