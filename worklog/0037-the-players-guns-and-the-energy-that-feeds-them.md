---
number: 37
title: The player's guns, and the energy that feeds them
date: 2026-09-18
area: decomp,engine,port
files: crates/hb-sim/src/weapons.rs,crates/hb-sim/tests/weapons.rs,crates/hb-sim/src/powerup.rs,crates/hb-sim/src/combat.rs,crates/hb-fly/src/battle.rs,crates/hb-fly/src/main.rs,docs/engine/simulation.md
slug: the-players-guns-and-the-energy-that-feeds-them
---

# 37. The player's guns, and the energy that feeds them

Powerups filled stocks that nothing spent, and the port fired a laser ten
times a second from the eye, a rate it had made up. Both were waiting on the
fire routine.

## Which gun

The player starts on weapon 23, not 1: `0x426ebf` writes 0x17 to the
selection. That is the Valkyrie Cannon, 128 units a second and an eighth of a
hit - twice the laser's damage at four times its speed. The weapon table's
rows turned out to begin 28 bytes before the speed the earlier reading
anchored on: a model name, an unused word and a short code come first. With
that alignment the boss weapons 9 to 16 get `wboss1.bin` to `wboss8.bin`,
which is the check that it is right.

## The trigger and the barrels

`0x47db11` keeps a rate accumulator and calls the fire routine each time it
passes 1.0, at the row's `+0xc` volleys a second: six for the guns, two for
the dispersion cannon, one for missiles. The fire routine `0x47d520` switches
on the weapon. The guns look at weapon energy, a third energy pool at
`0x62d678` alongside the main energy and the shield: none left, one barrel
alternating sides; up to half, two; more, four, at the corners of a unit
square. Two and four barrels cost a 256th and two 256ths of weapon energy a
volley, so at the start's half the cannon fires pairs for about twenty
seconds of held trigger and then drops to one barrel.

The dispersion cannon was the surprise: it fires straight back as well as
forward, a pair at a time with one barrel, and with four barrels nine shots
scattered ahead.

## Energy

Main energy is a reservoir. The player moves it an eighth at a keypress into
weapon energy or the shield (`,` and `.` in `HELLBEND.INI`); it and the hull
both creep back at 0x48 a second, a quarter of an hour from empty to full.
The afterburner is weapon 22 in the same trigger loop, burning a sixteenth of
its tank a second, and the tank refills from weapon energy. So the three
pools trade: flying fast costs gun barrels later.

## In the port

`hb_sim::weapons` has the table, the trigger, the barrels, the dispersion
pattern, the transfers, the per-frame creep and the afterburner's fuel.
`hb-fly` fires through it, plays each weapon's own sound, gates the
afterburner on fuel, binds the backquote and 1-3 to the four guns, `=` to the
next weapon and `,` and `.` to the transfers, and shows the weapon, its stock,
hull, shield, weapon and main energy on the HUD. Seven tests cover the table,
the rate, the barrels and their cost, the dispersion pattern, running out,
the transfers and the afterburner.

The missiles and mines go through `0x477890` with a locked target and are
next.
