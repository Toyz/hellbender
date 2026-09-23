---
number: 124
title: What a demo presses, and the hand it shows
date: 2026-09-22
area: decomp, port
files: crates/hb-fly/src/main.rs, docs/engine/cockpit.md
supersedes: 123
---

# 124. What a demo presses, and the hand it shows

123 wired the recorded fire button - the pose's fourth value - to the guns,
and the recorded key presses to the port's "pressed" test. That missed the
commonest way a demo fires: 57, the space bar, is `fireKey`, and a key record
of it holds the trigger for the frame just as the flag does. `DEMO1` holds
the flag on 250 of its 890 poses and presses 57 135 times; `DEMO2` and
`DEMO3` never set the flag and fire only by key records, 331 and 170 of them.

And the cockpit hand never showed a demo firing, because it read the fire
and weapon keys off the window itself, a second copy of the trigger's test.

## The key array

The engine has one keyboard state, the array at `0x5b36c0`, and everything
that reads a key reads it. A demo's player (`0x44d180`) clears it every frame
and writes only the recording into it. The port now builds that frame's
keys once: in a demo, the recorded trigger and key presses and nothing from
the keyboard; in play, the keyboard and the stick's buttons. The guns and
the hand both read it.

## The hand in a demo

The hand is chosen in the player's own update (`0x463aa0`, the call at
`0x464178`), which the main loop runs every frame before the demo's player
(`0x481d8b`, then `0x482307`) - there is no demo test on the way. Its stick
position comes from the ramps the arrow keys drive, and a demo never sets
those keys: the recording holds the ship's pose, not the stick. So in the
original's attract mode the hand stays centred and changes to the firing
hand while the trigger is held. With a joystick configured the ramps come
from the device, so a stick held during the demo would move it.

The chooser tests the trigger before the weapon key (`0x41fe27`); the port had
them the other way round.

**Still unknown:** nothing new.
