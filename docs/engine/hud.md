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
| 0 | 312, 3, 80, 17 | `Obj: %s`, three letters |
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
kind and writes its three-letter code into box 0, from the jump table at
`0x44ead0`:

| Kind | Code | Kind | Code |
| --- | --- | --- | --- |
| 0 destroy | `TGT` | 8 drop beacon | `RES` |
| 1 enter tunnel | `TUN` | 11 beacon | `BCN` |
| 2 checkpoint | `CHK` | 12 escort | `EST` |
| 3 jump zone | `JMP` | 13 message pod | `MSG` |
| 4 exit tunnel | `EXT` | 14 kill | `TGT` |
| 5 guardian | `GRD` | 15 network player | `PLY` |
| 6 start | `STR` | 7, 9, 10 | none |

Three of the sixteen entries fall through to the rest of the routine, so
those kinds leave whatever the box already said. It is the code that goes in
the box, never the long line the nav code builds at `0x625110` - that is
"Destroy Target" and the box is eighty pixels. Where the long line is drawn,
if anywhere, is not known: it is written twice in the image and read nowhere.

Then the distance into box 2, then the six gauges. A `GRD` point adds the
guardian's health as a percentage. Last it plays or stops the missile
warning.

**`0x420080`** is the top-left panel, which is where the game writes to you:
16, 3, 236 by 56 in 640x480, built out of the view's size, cleared and
written seven pixels a line - eight lines. While something is in it
`0x674d68` is set, and that is the flag the weapon pair reads, because the
panel is the same strip of screen as the weapon lines and the weapon's
picture.

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

## The weapon's picture

The left end of the top-left panel is a picture of the weapon you are
holding, in a box at 14, 3, 63 by 56 in 640x480 (`0x41f8fa`). The message
panel is 16, 3, 236 by 56 - the same strip - which is why a message hides
the weapon lines and the picture together.

`0x420e70` draws it. The selected weapon indexes a table at `0x501868` to
get one of twelve pictures named at `0x501838`, each a 64x64 `.RAW` in the
art: `valk`, `d4s`, `skl`, `f6rfl1`, `dom4s`, `c4s`, `vip4s`, `cls4s`,
`m4s`, `mg4s`, `f6mine1`, `f6super`. The names follow the weapons - `dom4s`
is the Dead On Missile, `cls4s` the Cluster, `mg4s` the Guided MIRV - and a
weapon with no picture of its own gets the Valkyrie's.

The engine does not blit it. It sets the viewport to the box, hangs the
picture on a quad as a texture and draws it through the same pipeline the
reticle and the objective arrow go through, which is how a 64x64 picture
ends up filling a 63 by 56 box.

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
`0x474bb8`. With the cockpit off and the view zoomed the top moves to the
view's own top instead.

`0x475100` walks the placements once a frame and drops the ones with hit
points at or below zero, the ones in the flyers' dying phase 0x12d, and
classes 18 and 33. What survives goes to `0x474f00`, which wraps the offset
at the world's edge - the `shl 6`, `sar 6` pair - turns it by the player's
heading, and shifts it down 11. That is where the range lives: the offset is
16.16, so down 11 is world units times 32, and the box's width multiplies
before another shift down 11, which makes a unit `width / 64` pixels. Sixty
four units across the box. Before any of that it tests the squared offset
against 0xef420, so the edge is round at about 31 units, not square.

A blip (`0x4746a0`) is five pixels: a three-pixel bar with one above and one
below, outlined in the paper colour. For something below the player the top
and bottom go, leaving the bar. Its colour is 0x3f when `0x40dc00` says the
class is one the guns count as a target - the same question the missile lock
asks - and 0x94 when it does not.

The arrow to the objective is on the dish too. `0x475290` sets the viewport
to that square, points a camera straight down, turns `navtarg.bin` by
`0x8000` less the bearing and draws it - a four-cornered dart, a tip, two
shoulders and a base. Its colour is the shade global: -101 for an objective
ahead, -152 for one roughly behind, between 0x7800 and 0x8800, and a
negative shade is a palette index outright. When it is behind, a few pixels
in 138 go at the top of the box as well.

The whole thing is skipped while `0x512744` is clear, which is the flag the
"Radar destroyed." message goes with.

**Not the radar:** `0x436fe0` walks the same objects and plots them through
`0x474830` at a different scale, drawing each one's name rather than a blip,
and `0x474c3b` runs it only when `0x5b3340` is set, with the viewport set to
the whole view. That is a debug overlay.

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
the five labels that do not name a gauge. `hb fly
<level> <out.png>` draws the HUD over its frame, and `--labels` turns the
cockpit labels on, which is how the layout is checked without a window.
`hb-fly` binds L to the labels and G to the reticle.

## Not read yet

How the viewport the objective arrow is drawn through is set up - the
camera at `0x42c9a0` and `0x42c980` sits at the origin looking down and the
model lies in a plane through it, so something else must part them. The port
turns the model's four corners in two dimensions instead.

What the debug overlay at `0x436fe0` is for, and where the string it draws
per object comes from - a field at `0x667128 + n * 112` that `0x448298` fills
from a table by index from 0x1393, which is not in the executable's string
resources.
