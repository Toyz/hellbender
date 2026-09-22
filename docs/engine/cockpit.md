---
title: The cockpit and the hand on the stick
status: partial
covers: ART\CKPT*.RAW, ART\HN*.RAW, ART\HP*.RAW, ART\HT*.RAW, ART\KNOB*.RAW, HELLBEND.EXE:0x41f900
worklog: 96
---

# The cockpit and the hand on the stick

The cockpit is not one picture. `ART` holds four full-screen views of it per
mode, 27 pictures of a hand, and a knob, and `0x41f900` loads and places them
all when the screen mode is set.

## A 640x480 layout, scaled

Every position in the cockpit is written as a number out of 640 by 480 and
scaled to the mode: `x * width / 640`, `y * height / 480`. The port does the
same, so a layout number here is the 640x480 one.

`0x512604` and `0x512608` are the mode's width and height - 320x200, 320x400 or
640x480 - and `%d` in every name below is the height.

## The four views

`0x4209a8` switches on the view angle and picks the picture:

| angle | file | |
| --- | --- | --- |
| 0 | `ckpt%d.raw` | ahead |
| 0x4000 | `ckpt%dr.raw` | right |
| 0x8000 | `ckpt%db.raw` | behind |
| 0xc000 | `ckpt%dl.raw` | left |

Anything else is `"Bad vview!"`. All four are full-screen, and index 0 is the
hole the world is drawn through.

## The hand

27 pictures: three sets of nine, named from the table at `0x5018f4`.

```
hn bl bm br   ml mm mr   fl fm fr
hp ...
ht ...
```

The second pair of letters is where the stick is - back, middle or forward
crossed with left, middle or right - so `hnmm` is centred. 280 by 110 in the
640x480 layout, which is the 140 by 46 the mode-200 files are.

`0x41fde0` chooses one from the two control axes. Each comes in scaled so that
a full push is 0x100, and the test is a quarter of that:

```
column = x < -0x40 ? 0 : x > 0x40 ? 2 : 1
row    = y < -0x40 ? 0 : y > 0x40 ? 2 : 1
index  = row * 3 + column
```

Then the set: the weapon key held (or its joystick button) picks `ht`, the fire
key held picks `hn`, and neither picks `hp`. The keys are `weaponKey` and
`fireKey` from `[Control]`, read through the keyboard state array at
`0x5b36c0`.

`0x420ace` draws it, at x 180 and with its bottom on the bottom of the frame -
370 in the 640x480 layout - but only when the view is ahead (`0x420ab6`) and
`[0x51265c]` is 1, which is `[Game]`'s `cockpitHandFlag`, whose default in the
image is 1. That is inside `0x420a60`, the cockpit routine the frame calls.
`0x4205b0` just before it has the same test and the same draw (`0x4205ef`,
`0x420607`), but nothing calls it - an older copy left in the binary. The port draws it on the same terms, out of `hb-render`'s `cockpit`
module.

The height the engine gives the box is one row short of the picture in two of
the three modes: `110 * 200 / 480` is 45 where `HNMM200.RAW` is 140 by 46, and
`110 * 400 / 480` is 91 where the mode-400 one is 92. Only 480 comes out even.
So the hand hangs a row past the bottom of the frame, and the port lets it.

## The knob

`knob%d.raw` is 12 by 17 in the layout, which is the 6 by 7 of `KNOB200.RAW`.
`0x41fb46` computes a box for it - 542 and 629 across, 462 down - and loads the
picture into `0x52f788`.

Nothing reads any of it again. The three numbers have no other reference in the
image, and the only other mentions of the buffer are the frees. So the knob is
loaded and placed every time the mode is set and never drawn.

`0x5125f8` is the view, and the three globals around it are the same angle in
other stages: `0x5018f0` is the one the loaded picture is for, so the art is
only read again when the view changes, and `0x420793` eases a third toward the
target at `0x8000` a second - so the view swings rather than snapping. The port
snaps.

## Unknown

Whether the HUD is drawn over the three other views, and what the parts of a
side cockpit are. What the swing at `0x420793` is for, given the picture
changes at once.
