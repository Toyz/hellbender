---
number: 33
title: The flyers make passes, and a tracer for the x87 code
date: 2026-09-18
area: decomp,engine,port,tooling
files: crates/hb-sim/src/flyer.rs,crates/hb-sim/src/turret.rs,crates/hb-sim/tests/flyer.rs,crates/hb-fly/src/battle.rs,crates/hb-render/src/scene.rs,crates/hb-formats/src/text.rs,tools/x87.py
---

# 33. The flyers make passes, and a tracer for the x87 code

Most of what moves in a level flies: classes 7 and 53 alone are 1,108
placements, and 56, 59 and 60 add 565. They all come back to `0x4967b0` or
routines shaped like it.

## The decisions, read

Actor `+0x64` is a phase. Phase 0 initialises (`+0x174` to 2048.0, `+0x158`
to the type's shot speed) and moves to 200. The rest, from `0x496b74` on:

- **Cone tests.** `0x492750` transforms the flyer's offset into the player's
  frame (`0x487c50` with the pose at `0x59d160`), takes `atan2` across and up,
  and answers 1 if both are under 30 degrees (`0x4ef510`, 0.5236), 2 if the
  flyer is ahead but outside, -2 if behind. `0x492940` is the mirror. A third
  flag is set when headings and pitches are within 30 degrees of each other.
- **Situations.** Both in each other's sights: head-on, and under 32 units
  (`0x200000`) it goes to 201. Aligned with the player on its tail and aiming
  (`4`): it breaks - to 2011 (16 units above the player) if its roll is under
  30 degrees (`0x4ef598`, 5,461.33), else to 2008 or 2009 by the roll's sign,
  16 units to the player's left or right - and flies there (2012) until the
  distance stops shrinking (`+0x154` holds the least), then 201. With the player
  ahead but not in its sights (`6`) and inside the attack range it matches the
  player's speed (`0x50cc50`).
- **Phases.** 200 flies at the player; inside `sqrt((R + r + 2)^2 - r^2)` -
  radius plus turning radius plus two, the turning radius being `+0x164` over
  the turn rate times `2 pi / 65536` - it goes to 2000, and inside the retreat
  range to 201. 2000 and 201 fly with mode 1, at the larger of the type's speed
  and twice the player's, 201 with an eighth of the turn rate, until past the
  attack range; then 200. The ranges are `!NewAtakRet` (`+0x1f8`, `+0x1fc`).
- **Mode 1 flies away.** In the steering, `mode == 1` scales the direction to
  the target by -1.0 (`0x49485f`, `0xbf800000`). That is what turns the phases
  into passes.
- **Firing** is `0x407770` - the turrets' interval accumulator and weapon -
  when the player is inside the attack range and `+0x190` says aimed, with the
  type's shot speed set to twice the flyer's own for the call and put back
  after (`0x496f86`).

Class 7 differs only in calling `0x4089e0` after, which is the common
visibility tail.

## The steering, not read

`0x4944c0` runs from `0x4944c0` to `0x495722`, about 1,250 instructions,
nearly all x87. What is clear: it predicts the next position and raises the
target to the ground plus `type+0x98` when that would be underground (`0x41c300`
at `0x49479d`), computes the target's heading and pitch, and integrates a
rigid body with velocity at `+0x15c` and angular state at `+0x168` onward.
`hb_sim::flyer` stands in: heading and pitch turn toward the target at the
type's turn rate and the flyer moves along its nose. A HOTH hornet run
against a still player makes a pass every five to six seconds - in, two
shots, past, out to 43 units, back (`a_flyer_makes_passes_and_shoots_on_the_way_in`).

## `tools/x87.py`

Following an eight-deep FPU stack through hundreds of lines by eye is how
mistakes get made, so there is now a tool for it. `tools/x87.py <start> <end>`
walks a stretch of the executable, decodes each x87 instruction from its
opcode bytes - so `fsubr`, `fdivr` and their popping forms mean what the Intel
manual says rather than what objdump prints - tracks integer moves, adds and
the `imul`/`shrd` fixed-point idiom well enough to follow values through the
stack frame, and prints every floating-point store as an expression. On the
turret's aim at `0x408c30` it prints exactly the lead formula worklog 28 read
by hand. It is straight-line only; at a branch target the stack is whatever it
was.

## In the port

`hb_sim::flyer::Flyer` has the phase machine and decisions as read and the
stand-in steering; `Turret::trigger` is split out so flyers share the weapon.
`hb-fly` runs flyers for classes 7, 53, 56, 59 and 60 within the 80-unit box,
and no longer sends them along courses (the routine does not read one). The
renderer now turns meshes by pitch and roll as well as heading.
`EnemyDef::attack_retreat` carries `!NewAtakRet`.

**Still unknown:** `0x4944c0` in full; the routines of classes 56, 59 and 60;
the second weapon's range test (`+0x210`, `+0x214`) and its `0x4922a0` /
`0x408460` calls; phase 900's trigger.
