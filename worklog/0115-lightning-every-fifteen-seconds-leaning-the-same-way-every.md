---
number: 115
title: Lightning every fifteen seconds, leaning the same way every time
date: 2026-09-22
area: render, port
files: crates/hb-sim/src/weather.rs, crates/hb-sim/tests/weather.rs, crates/hb-render/src/level.rs, crates/hb-render/src/scene.rs, crates/hb-render/tests/weather.rs, crates/hb-fly/src/main.rs, docs/engine/weather.md, docs/formats/lvl.md
---

# 115. Lightning every fifteen seconds, leaning the same way every time

114 read the strike and left the bolt. This reads the bolt and the bright sky
and puts lightning in the port - and corrects `lvl.md` on how often it comes.

## Fifteen seconds, not fifteen to forty-five

The level load hands `0x49d220` the two values of line 42 (`0x44bfae`):
`[0x6670dc]` and `[0x6670d8]`, 30.0 and 15.0 in 16.16. The spawner computes
`base + rand() % range + 1` on them as they are. `rand()` tops out at 32,767 and
the range is 1,966,080, so the modulo never bites: the random part is under
half a second. The strike's re-arm does the same. `lvl.md` had read it as
seconds and said 15 to 45; it is 15 to 15.5, every time.

## The bolt

`0x49d490`, the callback the strike puts in the draw list, calls `0x49d640`
while the eye is below the sky layer. It walks down from the strike to the
floor, a line per step in palette 110 or 111. The step's jitter is
`((rand() << 16) & 0x1ffff) - 0x10000` - and `rand() << 16` has nothing in its
low sixteen bits, so the mask keeps one bit of `rand()`. Every step goes
nowhere or one unit toward -x and -z. Every bolt the game ever drew leans the
same way. The drop, `(rand() << 16) & 0x3ffff`, is 0 to 3 whole units.

`0x49d4c0` beside it is a bolt that forks - it calls itself - and
`tools/funcs.py` says nothing reaches it.

## The bright sky

`initBrightSky` (`0x451290`) runs as the sky is set up and builds a lit copy of
the sky tile: each texel's colour through `0x451570`, which scales it by up to
two but no further than puts its brightest channel at 255, then back to the
nearest palette index by `0x485080`'s `29|dr| + 58|dg| + 15|db|`. The strike
blits the lit tile into the texture page; the end of the flash blits the saved
one back. The port does the same as a second sky remap.

On `IOWAH` and `MORBOS` it does nothing visible. Their sky palettes are so dark
that the level palettes call the whole sky black and one dark red or grey, and
twice that dark red is still nearest the same red. `HOTH`'s sky, lit, would
come out twice as bright; it has no lightning.

## In the port

`Lightning` in `hb_sim::weather` arms from line 42 as stored, steps only with
the eye at or above the ground, and reports the strike, the thunder and the
dark; `hb-fly` plays `lghtng.wav` and `thun-c.wav` with distance falloff,
lights models at full ambient and swaps the sky while the flash lasts, and
draws the bolt with a depth test. A render test draws a flash on `IOWAH`.
