---
number: 35
title: Powerups, and the sprite models they are drawn with
date: 2026-09-18
area: decomp,format,engine,render,port
files: crates/hb-sim/src/powerup.rs,crates/hb-sim/tests/powerup.rs,crates/hb-sim/src/mission.rs,crates/hb-formats/src/text.rs,crates/hb-formats/src/mrgl.rs,crates/hb-formats/tests/against_the_game.rs,crates/hb-render/src/level.rs,crates/hb-render/src/scene.rs,crates/hb-fly/src/battle.rs,crates/hb-fly/src/main.rs,crates/hb/src/main.rs,docs/engine/simulation.md,docs/formats/mrgl.md,docs/formats/level-text.md
slug: powerups-and-the-sprite-models
---

# 35. Powerups, and the sprite models they are drawn with

The mission work left `MORBOS3` unwinnable: its message pod is a powerup, and
there were none. Looking for where the pod lives led to the whole system.

## Where they come from

The array is at `0x66fd60`, 24 bytes a powerup. `0x426540` fills it from the
level's `.PUP` - `x,y,z,kind` lines - and in the whole game that file lists one
thing, `MORBOS3`'s message pod. Everything else is dropped: the destroy routine
rolls a percentage against `.DEF` line 2's third number and drops the kind in
its fourth, `-1` meaning any. That closes half of a line the format page had
as "not read yet"; the weapon bunkers, all `100,<kind>`, are where the weapons
are.

## What they do

`0x426760` is a 31-way switch. The engine's names for the kinds are a table at
`0x502308` ("Damage 25%", "Energy 100%", "Super Weapon Piece 3") and the voice
lines each plays come from the phrase table found in worklog 34. Several names
are stale: kind 11, "Energy Can (Not used)", repairs the hull; kind 3, "Dead-On
Missiles", is announced as Sledgehammer Rockets. Repairs are refused when the
hull is above 0xea60 - about 91.6% - and the powerup stays put; energy is
refused when full. Eight Bion pieces, collected in any order into a bit mask,
make the super weapon and select it. Picking up a message pod finds the mission
point naming it and marks it done, which is how the pod point completes.

The hull, energy and shield globals came out along the way: `0x5b39ec`,
`0x62d63c`, `0x62d6e0`. The first explains a condition from the mission work:
killing the kill-point target ends the level only while the hull is above
zero. Reading that handler again (`0x4727ef`), it never advances either - a
kill point is `ROID4`'s way out, and the jump zone listed after it is never
reached. `hb_sim::mission` now does the same, which worklog 34 had wrong.

## The sprite models

The powerups' models, `F6*.BIN` in `STARTUP.POD`, drew as nothing: four
vertices and no polygons. They are made of record types the parser skipped.
`0x04` gives texels per vertex, `0x1d` is a material that flips through eight
frames every 0.244 seconds, and `0x0f` is a polygon that names vertices and
takes their texels. The draw handlers settled all three (the table at
`0x50c448`), and one more thing: the `0x0f` span routine skips texel 0, so the
quad is a cut-out. The engine turns the model to face the eye.

The same reading found `0x19`, a flat indexed polygon coloured by a `0x0a`
record, in 34 level models - 215 polygons the port never drew - and that the
`0x17` record the port takes flat colours from is not what the engine reads.
Both are for next time.

A powerup's size is its model's own scale. The mesh-start record's second word
is the model unit, which the bounds routine divides by; for a placed actor the
engine uses the type's radius instead, but a powerup has no type. The `f6`
family comes out 2.6 units across the diagonal, and that is the pickup box.

## In the port

`hb_sim::powerup` has the kinds, the effects with their refusals and voice
lines, the stores (the single-player loadout of `0x426e90`), drops, and the
pickup test. `hb-render` loads the 31 models after the wrecks, animates
flipbook materials from a clock on the scene, and draws `0x0f` polygons with
texel 0 clear. `hb-fly` puts the `.PUP` powerups down, drops them from what the
player destroys, picks them up, and shows the energy on the HUD. `hb look
<level> <n> <out> --powerup K --at S` draws one.

Nine tests cover the effects, the drop rate, the pickup box, a refused
powerup speaking once, and the message pod finishing its mission point; the
format tests check every model's indexed polygons and flipbook frames resolve
and that the `.DEF` drop fields stay in range.

Still out: the ceiling limit on where a drop lands, which needs a ceiling query
the port does not have; the engine repeating a refusal's line every frame; and
anything that spends the stocks.
