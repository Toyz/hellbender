---
title: The HUD
status: partial
covers: HELLBEND.EXE:0x44e5e0, crates/hb-render/src/hud.rs
worklog: 67, 68
---

# The HUD

Everything the game writes over the cockpit - the weapon, the ammunition, the
objective, the distance to it, the six gauges, the countdown and the labels
that name them all - comes from three routines and two tables in
`HELLBEND.EXE`. It is drawn in [the small font](../formats/font.md) that lives
in the image, never in `FONT.BIN`.

## The box table

`0x5052d0` is twelve boxes, four 32-bit values each - x, y, width, height -
authored in 640x480. Nothing draws at those numbers directly: every use
scales them into the view it is drawing into, `x * view_width / 640` and
`y * view_height / 480` (`0x44e673`), so one table serves all three of the
game's screen sizes.

| # | Box | What it holds |
| --- | --- | --- |
| 0 | 312, 3, 80, 17 | `Obj: %s` |
| 1 | 140, 36, 110, 17 | `Ammo: %d` or `Ammo: inf` |
| 2 | 312, 24, 80, 17 | `Dist:%4d` |
| 3 | 514, 96, 88, 17 | nothing - no reference in the image |
| 4 | 140, 13, 110, 17 | `Weapon: %s` |
| 5 | 310, 7, 70, 17 | nothing - no reference in the image |
| 6 | 50, 373, 14, 50 | Speed |
| 7 | 21, 373, 14, 50 | Hull Integrity |
| 8 | 79, 373, 14, 50 | Turbo Fuel |
| 9 | 547, 373, 14, 50 | Weapon Energy |
| 10 | 576, 373, 14, 50 | Ship Energy |
| 11 | 605, 373, 14, 50 | Shield Energy |

Boxes 3 and 5 are dead: the ten others are all read by absolute address, and
those two by nothing.

A line goes in its box centred both ways, on a background cleared to index 0,
in index 47 (`0x44e6f4`). A gauge fills its box from the bottom: the part
above the level in index 1, the rest in `steps` bands of a palette ramp,
bottom band first.

| Gauge | Box | Ramp | Bands |
| --- | --- | --- | --- |
| Speed | 6 | 0x61 | 14 |
| Hull Integrity | 7 | 0x81 | 14 |
| Turbo Fuel | 8 | 0x21 | 30 |
| Weapon Energy | 9 | 0x91 | 10 |
| Ship Energy | 10 | 0xa1 | 14 |
| Shield Energy | 11 | 0xb1 | 14 |

## What draws what

**`0x44e5e0`** is the frame's HUD. It looks up the current mission point's
kind and writes its three-letter code into box 0 - `TGT`, `TUN`, `CHK`,
`JMP`, `EXT`, `GRD`, `STR`, `EST`, `MSG`, `RES`, `PLY`, `BCN`, from the jump
table at `0x44ead0` - then the distance into box 2, then the six gauges. A
`GRD` point adds the guardian's health as a percentage. Last it plays or
stops the missile warning.

**`0x44eb0f`** is the weapon pair. The name comes from a table at `0x50e798`
indexed by the selected weapon times 0x44; the count from `0x61bce0` indexed
by the same weapon, where -1 means unlimited and prints `inf`. Both lines are
skipped entirely while `0x674d68` is set, which is the flag for a message
being up in the top-left panel - the panel and the two lines share that
corner of the screen.

**The countdown.** `0x512620` is a 16.16 second timer that `0x464f7d` takes
the frame's elapsed time out of each frame and clamps at zero. While it is
not zero `0x44e9b8` writes it as a bare integer against the bottom right
corner of the view - `x = width - text - 2`, `y = height - 9` - on a box
filled with index 8 and framed in index 16.

## The cockpit labels

`0x44f000` names every element on the screen, and the key bound to
`keyCockpitLabel` turns it on and off (`0x40eaed` flips `0x512754`). Each
label is a box as wide as its text and four pixels more, filled with index 0,
framed in index 151, with the text two pixels in; a gauge's label also gets a
vertical line from the callout down to the top of the gauge, in the same
index 151.

The callouts are a second table at `0x5053a0`, in the same 640x480:

| Callout | Label |
| --- | --- |
| 23, 250 | Hull Integrity |
| 51, 290 | Speed |
| 81, 330 | Turbo Fuel |
| 562, 330 | Weapon Energy |
| 590, 290 | Ship Energy |
| 621, 250 | Shield Energy |
| 380, 135 | Objective Direction |
| 200, 77 | Objective Abbreviation |
| 200, 77 | Distance to Objective |
| 20, 85 | Current Weapon |
| 20, 85 | Shots left |

The three on the right of the screen hang their boxes to the left of the
callout, which is what the second helper, `0x44eec0`, exists for; the rest
run right from it (`0x44ee10`). The table's third value, 200, is never read -
a label's width comes from its text.

## The radar

The dish in the top right of the cockpit art is a square the engine draws
into: 507, 6, 101 by 101 in the same 640x480, built out of the view's size at
`0x474a70`. With the cockpit off and the view zoomed the top moves to the
view's own top instead.

`0x437148` walks the object table once a frame and plots every object that is
not the player, has hit points above zero and does not have bit 1 of its
flags set. Each offset is wrapped at the world's edge - the `shl 6`, `sar 6`
pair that appears everywhere - turned by the player's heading, and scaled:
the offset is 16.16, shifted down 13, multiplied by the box's width and
shifted down 11 again, which puts 256 world units across the box. So the
radar reaches about 128 units, an eighth of the world.

A blip is not a dot. `0x4749a0` draws a short string, six pixels right of the
plotted point, from a field in the object's own record at `0x667128 + n *
112`, which a loader at `0x448298` fills from a table of strings by index
from 0x1393. That table has not been found: it is not in the executable's
string resources.

Everything is skipped while `0x512744` is clear, which is the flag the
"Radar destroyed." message goes with.

## The reticle

The crosshair in the middle of the view is a model. `target.bin` in
`STARTUP.POD` is nine vertices fifty units ahead and two across - a centre
and an octagon - and four triangles from the centre out to the up, left,
down and right pairs of the ring, which makes a four-armed cross.
`0x4652bd` draws it with the camera at the origin and no rotation, so it
lands in the middle of whatever the view is.

Its colour is a palette index walked between 32 and 63 and back, a step a
frame (`0x465223` moves it, `0x50cc74` is the direction), negated into the
shade global at `0x5b3888` - and a negative shade is a palette index
outright rather than a band. 32 to 63 is the green the rest of the HUD is
drawn in.

The key bound to `keyCrosshair` turns it off and on (`0x512584`).

## Drawing a string

Two routines put text on the screen and they are not the same. `0x484ad0`
takes a colour and a bounding box, wraps on a newline or a literal `\n`,
stops at the bottom of the box, and writes only the glyph's set pixels
through a 256-entry remap at `0x606a20`. `0x485da0` takes no colour and
writes both: set pixels in `[0x606a20 + 0x38]` and clear pixels in
`[0x606a20]`, so it paints its own background. The readouts use the first;
the labels and the countdown use the second.

Both step seven pixels a line and draw a seven-row cell from a six-row glyph,
so the last row is always clear.

## What the port has

`crates/hb-render/src/hud.rs` is all of the above except the leader lines for
the five labels that do not name a gauge, and what a blip says. `hb fly
<level> <out.png>` draws the HUD over its frame, and `--labels` turns the
cockpit labels on, which is how the layout is checked without a window.
`hb-fly` binds L to the labels and G to the reticle.

## Not read yet

What a blip says - the string table indexed from 0x1393 that `0x448298`
reads - so the port draws a mark instead.

The objective arrow is half read. `0x475290` takes the heading
to the objective and draws a model - `0x6210c0` - into the same rect the
radar uses, with the whole view as the rect instead when `0x474af0` set it.
When the objective is behind, between 0x7800 and 0x8800, it plots a handful
of pixels in index 138 instead of the model.
