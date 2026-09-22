---
number: 104
title: How a level starts
date: 2026-09-21
area: decomp, world, port
files: crates/hb-sim/src/mission.rs, docs/formats/lvl.md
---

# 104. How a level starts

"The loading into the level is still wrong, you refuse to look at how the
camera is meant to enter levels." Fair. So I looked.

## The spawn, which the port already had right

`0x471333`, at the end of the `.NAV` loader's own additions, is the whole of
it:

```
if nav[0].kind == 6:
    ship.x = nav[0].x            ; 0x625140, 0x625144, 0x625148
    ship.y = nav[0].y
    ship.z = nav[0].z
    ship.y = groundUnder(...) + 0x100000      ; 0x41c300, then + 16.0
    ship.pitch   = nav[0]+0xb0   ; 0x6251fc
    ship.roll    = nav[0]+0xb4
    ship.heading = nav[0]+0xb8
    currentPoint = 1
    nav[0].done  = 1
```

The point's own height is written and then thrown away: what the ship gets is
the ground under the point plus exactly sixteen units. `0x41c300` takes the
cell out of bits 19 to 25 of two coordinates, which is the same query
`hb-world`'s `ceiling_of_solid` answers. So `morbos`'s start - `(-76, 50, 116)`
facing `0xc000` - puts the ship sixteen units over the ground there, facing
west, which is what the port has been doing since [[57]]. The comment saying so
now cites the address instead of asserting it.

`0x464800` follows, which builds the ship's matrix from those three angles and
hands the position to the view. No offset, no chase camera: the eye is the
ship.

## The animation, which is real and is not yet pinned down

There *is* a scripted flight sequence in the image, at `0x45a2a0`, and it is a
small game loop of its own - it draws the world, times its own frames, and
holds the player out of the controls while it runs:

- roll to zero, and pitch to zero if it was positive
- every frame: pitch down by a quarter of the frame time, clamped at
  `0xffffc100`, which is 88.6 degrees; a second angle at `0x51255c` rises by an
  eighth of the frame time to `0x3f00` and then falls back
- the throttle forced to `0xffff`, full, every frame
- `blast4.wav` at the ship, seven eighths of the way through
- three keyboard bytes checked each frame, any of them ending it early
- and it ends when its own clock passes `0x20000`

Whether that is the entry or the exit I have not established. Against its being
the entry: it starts from whatever attitude the ship is already in rather than
setting one, which is what levelling a player's ship before an exit looks like,
and `blast4.wav` is an explosion. For it: the throttle. The engine's throttle
is 0 in the image's `.data` and nothing in the level start sets it, so a level
that opened without this routine would open with the ship stopped in the air -
and this routine is the only thing that puts it at full.

It is reached from the command table at `0x503c68` rather than from the level
sequence directly, which is the thread to pull next.

**Still unknown:** which command dispatches `0x45a2a0`, and so whether it is
the way in or the way out. What the second angle at `0x51255c` is - it is not
the ship's, and it rises and falls where the ship's pitch only falls, which is
the shape of a camera swinging and coming back.
