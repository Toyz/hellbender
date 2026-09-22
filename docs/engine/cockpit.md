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

`0x420607` draws it, at x 180 and with its bottom on the bottom of the frame -
370 in the 640x480 layout - but only when `[0x51265c]` is 1, which is
`[Game]`'s `cockpitHandFlag`, and its default is 1. The port draws it on the
same terms, out of `hb-render`'s `cockpit` module.

## The knob

`knob%d.raw` is 12 by 17 in the layout, which is the 6 by 7 of `KNOB200.RAW`.
`0x41fb46` computes a box for it - 542 and 629 across, 462 down - and loads the
picture into `0x52f788`.

Nothing reads any of it again. The three numbers have no other reference in the
image, and the only other mentions of the buffer are the frees. So the knob is
loaded and placed every time the mode is set and never drawn.

## Unknown

What `0x5125f8` is. It is tested before the hand is drawn and skips it when
set, and it is not any of the flags this page names.

Whether the HUD is drawn over the three other views, and what the parts of a
side cockpit are.
