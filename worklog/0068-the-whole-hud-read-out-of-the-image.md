---
number: 68
title: The whole HUD, read out of the image
date: 2026-09-20
area: decomp, ui, port
files: crates/hb-render/src/hud.rs, crates/hb-fly/src/main.rs, crates/hb/src/main.rs, docs/engine/hud.md
---

# 68. The whole HUD, read out of the image

The font was wrong and [[67]] fixed it, but the layout under it was still
mine: boxes I had put where they looked right. The way to stop guessing was
to read the routine that draws them, so I did, and it turned out to be three
routines and two tables.

## Twelve boxes, ten of them alive

`0x5052d0` is twelve records of x, y, width, height, authored in 640x480 and
scaled into whatever view is being drawn - `x * width / 640`, `y * height /
480`, at `0x44e673`. That is why the same numbers serve the game's three
screen sizes.

Each of the ten live boxes is read by its own absolute address, which made
them easy to attribute: box 0 is the objective code, 2 the distance, 4 the
weapon, 1 its ammunition, and 6 to 11 the gauges. Boxes 3 and 5 are read by
nothing at all. They are dead.

I had the objective and the weapon the right way round by luck: what settles
it is that the switch at `0x44e5fe` picks between `TGT`, `TUN`, `CHK`, `JMP`,
`EXT`, `GRD`, `STR`, `EST`, `MSG`, `RES`, `PLY` and `BCN` and writes the
result through `Obj: %s` into box 0. The distance is `Dist:%4d` - an integer,
which the port was printing as a rounded float.

## What the gauges are called

`0x44f000` was the find of the sitting. It is the cockpit labels: a mode,
toggled by the key bound to `keyCockpitLabel`, that names every element on
the screen with a framed box and a leader line. Which means the game tells
you what its own gauges are:

    Hull Integrity   Speed   Turbo Fuel   Weapon Energy   Ship Energy
    Shield Energy    Objective Direction   Objective Abbreviation
    Distance to Objective   Current Weapon   Shots left

The port had five of those six gauges under names I had made up. The callouts
are a second table at `0x5053a0`, in the same 640x480, and the three on the
right of the screen hang their boxes to the left of the callout instead of
the right - that is the whole reason `0x44eec0` exists beside `0x44ee10`.
The table's third value, a 200 in every row, is never read; a label is as
wide as its text and four pixels more.

## Two ways to draw a string

`0x484ad0` takes a colour and a box, wraps, and writes only the set pixels
through a remap at `0x606a20`. `0x485da0` takes no colour and writes the
clear pixels too, in `[0x606a20]`, with the set ones in `[0x606a20 + 0x38]` -
it paints its own background. The readouts use the first, the labels and the
countdown the second. Both draw a seven-row cell from a six-row glyph, so
there is always a clear row under a line.

## A countdown nobody was drawing

`0x512620` is a 16.16 timer that `0x464f7d` decrements by the frame's elapsed
time and clamps at zero. While it runs, `0x44e9b8` writes the whole seconds
as a bare number against the bottom right corner of the view, on a box filled
with 8 and framed with 16. Nothing in the port knew about it.

## Weapon and ammunition hide

The weapon pair is its own routine, `0x44eb0f`, and the first thing it does
is return if `0x674d68` is set. That flag means a message is up in the
top-left panel - `0x420080` clears a 236x56 area there at 16, 3 and writes
into it seven pixels a line. The panel and the two lines are the same corner
of the screen, so the lines get out of the way.

## What the port does now

`hb-render::hud` is the table, the codes, the gauges with their real names,
the framed-box primitive, the countdown, and the labels with the leader lines
for the six gauges. `hb-fly` binds L to the labels. `hb fly <level> <out.png>`
draws the HUD over its frame and `--labels` turns the labels on, which is how
I checked it: render, look at the PNG, compare with a shot of the original.
Weapon and ammunition top left, objective and distance top middle, three
gauges bottom left and three bottom right, in the right colours.

**Still unknown:** the radar in the top right of the cockpit and the reticle
in the middle of the view. Neither is in any of the three routines, and
neither the string table nor the primitives they would have to use led
anywhere - `keyCrosshair` binds a key that toggles `0x512584`, which gates a
palette index oscillating between 32 and 63 at `0x465215`, and that index
goes into `0x5b3888`, which the rasteriser reads all over. So the crosshair
may be drawn in the 3D pass rather than over it. Also unread: the leader-line
geometry for the five labels that do not name a gauge, and whether the
top-left panel's message system is the same one as `0x481030`'s.
