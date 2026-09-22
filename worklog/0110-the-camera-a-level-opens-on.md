---
number: 110
title: The camera a level opens on
date: 2026-09-21
area: decomp, port, ui
resolves: 107
files: crates/hb-fly/src/main.rs, docs/port/plan.md
---

# 110. The camera a level opens on

[[107]] said a level's opening animation was cut because the hook at
`0x45a290` is one `ret`. [[108]] filled that hook with something invented,
which was wrong twice over: it was not the engine's, and pitched 88 degrees
with the eye off the ship it left the game staring at the ground. Both are out.

The animation is real and it is at **`0x45a780`**. Found by asking what the
game actually does - it runs under Wine now - and then by looking for the right
thing in the image: a sequence that polls the same three skip keys the jump-out
polls. There are five of them, and `0x45a780` is the one that moves a camera.

## What it does

```
ground = groundAt(ship.x, ship.z)                 ; 0x41adf0
y      = ground + [0x5055d4] / 2                  ; 0x5055d4 is 128.0, so 64 up
0x42c980(ship.x, y, ship.z)                       ; the eye, over the ship
0x42c9a0(0x3fff, 0, 0)                            ; looking straight down
[0x512568] = 2                                    ; the outside view

each frame:
    spin += frame_time / 4                        ; a quarter turn a second
    y    -= 0x100000 * frame_time                 ; 16 units a second
    if ship.y + 0x20000 > y: done                 ; two units above the ship
    0x42c980(ship.x, y, ship.z)
    0x429f00(0x3fff, 0, spin)
    ...draw the world...
```

So: the eye starts 64 units over the ship looking straight down, falls at 16
units a second while turning a quarter of a turn a second, and stops two units
above it. From a ship sixteen units off the ground that is 46 units of fall,
about **2.9 seconds** and about three quarters of a turn. The ship does not
move; the loop draws and nothing else. Any of the three keys ends it.

`0x429f00` takes all three angles where `0x42c9a0` takes them one at a time,
which is the only difference between the setup and the loop.

## Why the first two answers were wrong

`0x45a290` really is an empty hook, and really is empty in the December build
too - that part of [[107]] stands. It is simply not where this lives.

And `0x45a2a0`, the eight-second sequence [[105]] read, really is not this one:
the disc's executable with that routine's first byte patched to `ret` still
plays the opening camera, which is what said to keep looking.

## In the port

`hb-fly` does the above, with the engine's numbers: 64 up, 16 a second, a
quarter turn a second, stopping two units above, the cockpit not drawn while
the eye is off the ship, any key skipping. Eve's welcome waits for it to land,
which is where the cockpit's own code plays it.

**Still unknown:** what the eye is meant to be looking *at*. The engine is
pointing it at the player's ship and this port does not draw the player's ship
at all, so the descent turns over empty ground.
