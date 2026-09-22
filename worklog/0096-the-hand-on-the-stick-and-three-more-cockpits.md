---
number: 96
title: The hand on the stick, and three more cockpits
date: 2026-09-21
area: decomp, render, port, content
files: docs/engine/cockpit.md, crates/hb-render/src/cockpit.rs, crates/hb-render/src/raster.rs, crates/hb-render/src/lib.rs, crates/hb-render/tests/cockpit.rs, crates/hb-fly/src/main.rs, crates/hb-fly/src/keys.rs, crates/hb/src/main.rs
---

# 96. The hand on the stick, and three more cockpits

"The joystick on the cockpit is missing" has been on the list twice. The art
was found long ago - `KNOB200.RAW`, `KNOB400.RAW`, `KNOB480.RAW`, 6 by 7 and up
- and the draw site never was. It never was because there isn't one. The thing
that moves in the cockpit is a hand, and there are 27 pictures of it.

## What is actually in ART

`knob%d.raw` is at `0x501ad8` and the routine that loads it, `0x41fb6e`,
computes three numbers for it - 542 and 629 across a 640-wide layout, 462 down
a 480-tall one - and puts the picture in `0x52f788`. Every reference to those
three numbers in the image is the write, and the only other mentions of the
buffer are the frees. The knob is loaded and placed every time the screen mode
is set and then nothing draws it.

Two names up from it is `%s%d.raw`, and the `%s` comes from a table at
`0x5018f4`:

```
hnbl hnbm hnbr  hnml hnmm hnmr  hnfl hnfm hnfr
hpbl ...                                  hpfr
htbl ...                                  htfr
```

27 pictures, all of them in `STARTUP.POD`, 140 by 46 at mode 200 and 280 by 110
at 480. The second pair of letters is where the stick is: back, middle or
forward crossed with left, middle or right.

## Choosing one

`0x41fde0` takes two numbers and answers with an index:

```
column = x < -0x40 ? 0 : x > 0x40 ? 2 : 1
row    = y < -0x40 ? 0 : y > 0x40 ? 2 : 1
index  = row * 3 + column
```

Its caller (`0x464147`) is what makes the scale readable: each axis arrives as
`control << 8 / 0x2492`, and `0x2492` is the input scale the flight model
divides every key by - `SEVENTH` in `hb-sim`, worked out in [[31]]. So the
arguments are the control deflection normalised to a full push of 0x100, and
the hand moves at a quarter of one.

Then the set. The weapon key held picks `ht`, the fire key held picks `hn`, and
neither picks `hp` - read out of the keyboard array at `0x5b36c0` at the two
scan codes `weaponKey` and `fireKey` bind, with two joystick buttons beside
them.

`0x420607` draws the chosen picture with its left edge at 180 and its bottom on
the bottom of the frame, and only when the view is ahead and `[0x51265c]` is
1 - which is `[Game]`'s `cockpitHandFlag`, whose default in the image is 1.

The box the layout gives the hand is a row shorter than the picture in two of
the three modes - `110 * 200 / 480` is 45 where `HNMM200.RAW` is 140 by 46 - so
the hand hangs a row past the bottom of the frame. The port lets it, and a test
says so.

## And four cockpits, not one

Chasing the knob turned up something else the port did not know: `ckpt%d.raw`
has three siblings. `0x4209a8` switches on the view angle and picks
`ckpt%d.raw` at 0, `ckpt%dr.raw` at 0x4000, `ckpt%db.raw` at 0x8000 and
`ckpt%dl.raw` at 0xc000, and calls anything else `"Bad vview!"`. All twelve are
in `STARTUP.POD` at full screen size. So `0x5125f8` is the view, which is also
what the hand is gated on, and `keyChangeViews` is what turns it.

## In the port

`hb-render` gains a `cockpit` module - the names, the quarter-push chooser, and
the box scaled out of the 640x480 layout - and `Target` gains `overlay_at`, the
positioned version of the blit it already had. `hb-fly` loads all 27, honours
`cockpitHandFlag`, and picks one each frame from `ship.keys`, the flight
model's own ramped inputs, which is the same number the engine hands the
chooser. `hb fly <level> <out.png>` draws the resting centred hand so the
placement can be looked at without flying.

`hb-fly` binds `keyChangeViews` and turns a quarter at a time: the world is
drawn from the ship's camera turned by the view, the matching picture goes over
it, and the hand is drawn only looking ahead. The engine eases its view toward
the target at `0x8000` a second (`0x420793`) where this snaps.

**Still unknown:** whether the HUD is drawn over the three other views, and
what a side cockpit's own parts are. What the eased angle is for, given the
picture changes the moment the view does.
