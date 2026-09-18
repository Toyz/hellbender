---
number: 32
title: Eighty units: how far the engine draws, fogs and thinks
date: 2026-09-18
area: decomp,render,engine,port
files: crates/hb-render/src/scene.rs,crates/hb-render/src/raster.rs,crates/hb-render/src/level.rs,crates/hb-sim/src/combat.rs,crates/hb-fly/src/battle.rs,crates/hb-fly/src/main.rs
---

# 32. Eighty units: how far the engine draws, fogs and thinks

Looking for what gates the flying enemies' AI - phase 200 steers at the
player from anywhere, which would bring every flyer in a level at once - led to
the actor loop, and the answer to that question turned out to be the answer to
three others.

## Actors think within 80 units

The class dispatch `0x40bb00` has one caller, the actor loop at `0x406650`,
which first calls the draw-and-cull routine `0x40da00` and **skips the update**
if it reports the actor unseen. `0x40da00` asks `0x42f710`, which returns 2 -
out of range - when the actor is more than `0x500000` (80 units) from the eye
on x or on z (`0x42f7b9`, `0x42f7d4`). So an actor draws and thinks only inside
an 80-unit box around the eye. Worklog 28 had read the inner routine alone and
recorded that every actor thinks every frame; that was wrong, and turrets in
the port were firing from across the map. `hb_sim::combat::in_range` is the
test, and turrets and course followers now use it.

## The engine draws ten cells each way

The ground drawer's vertices come from a projected grid at `0x526038`, 22
entries wide, indexed `(x - eye_x + 10) & 0x7f` (`0x414c1d`): the terrain is a
square of ten cells each way of the eye's cell - the same 80 units. The port
drew every cell within 220 units.

## Fog is 48 to 64

Each ground vertex's fog value (`0x59d380` and its siblings) is
`clamp(depth - [0x75acd8], 0, [0x6edfb4]) / [0x6edfb4]` (`0x414774`), and the
terrain loader's companion `0x412b70` sets those to `0x300000` and `0x100000`.
Fog starts at 48 units and is total by 64. The port ran its fog ramp from 0 to
220 units, so its world was hazy near and visible far - the opposite of the
engine's, which is crisp and then gone.

## Texture resolution falls with distance

Both terrain texture setups end by averaging the four vertices' depths and
calling `0x48a510(0xffff - (depth + 16) * 0xffff / 80)` (`0x413cdc`,
`0x4150cc`). That routine is not fog, as it first looked: it indexes a
texture's levels by the value and loads that level's size from `0x5112d0` -
16, 32, 64, 256 - into `0x5d29e0` along with its pixel pointers. So a
64-texel texture is drawn at full size to about 11 units, at 32 to about 37,
and at 16 beyond. The port always sampled full size, which aliases badly on
the far ground - much of the "crunchy" look. `Level` now makes half and quarter
copies by averaging 2x2 blocks in colour and looking the result up in the
level's `.MAP`; how the engine made its copies is not read.

## Also found on the way

- **Line 42 of the `.LVL`** (15.0, 30.0) goes to the weather routine
  `0x49d220`, not to fog.
- The dogfight AI `0x4967b0` - classes 7 and 53, 1,108 placements - is a phase
  machine over actor `+0x64` with two cone tests (`0x492750`, `0x492940`: is
  the flyer within 30 degrees of the player's nose, and the player of the
  flyer's), break-off manoeuvres 16 units beside or above the player, attack
  and retreat ranges from `!NewAtakRet`, and firing through the same
  `0x407770` -> `0x406dc0` path as the turrets with the shot speed doubled from
  the flyer's own. Its steering `0x4944c0` is an x87 rigid-body integrator not
  yet read. Next.

Frame rate went up with the smaller square: `hb bench hoth` 154 to 283 frames a
second, `float` 220 to 624.

**Still unknown:** how the engine builds its smaller texture copies; whether
models are fogged and mip-mapped the same way (their draw path is separate);
`0x5125e8`'s role beyond the depth shift of 4.
