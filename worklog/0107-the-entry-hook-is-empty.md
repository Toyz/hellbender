---
number: 107
title: The entry hook is empty
date: 2026-09-21
area: decomp, port
files: crates/hb-fly/src/main.rs
---

# 107. The entry hook is empty

"I swear the game didn't just drop you into the cockpit." Chased it to the
level-start code, and the answer is better than a no: the hook is there and it
does nothing.

`0x4815ea` compares the level's name with the last one loaded, and when it has
changed:

```
[0x5126ac] = 1
call 0x45b9f0      ; story slot 3, then slot 1 - the film and the arrival
call 0x45bc30      ; story slot 4, which every shipped level leaves null
call 0x45a290      ; <- this
[0x5126ac] = 0
```

`0x45a290` is one byte:

```
0045a28e  c3 90                    ; the previous routine's ret, then padding
0045a290  c3                       ; and this
```

A function whose whole body is `return`. The compiler emitted it, the linker
kept the call, and it does nothing at all. Right after the movies, in the one
place a level's opening animation would go, and immediately before
`0x45a2a0` - the eight-second flight sequence [[105]] read, which is the way
*out*. The two sit next to each other in the source, one written and one
emptied.

So the retail build does drop you into the cockpit, and somebody meant it not
to.

## What the three view words are, since they came up on the way

- `0x5125f8`: which of four ways you are looking, 0 to 0xc000, which picks the
  cockpit picture ([[96]]).
- `0x512568`: a mode, cycled 0, 1, 2 by a key (`0x40ea9c`). The jump-out sets
  it to 2 and the view is then built from the camera's own two angles around
  the ship rather than from the cockpit, so at least one of the three is an
  outside view.
- `0x5125d4`: another three-way cycle on another key (`0x40ea4f`), which gates
  a HUD element at `0x4202dc`.

`keyChangeViews` writes `0x5127e4` (`0x42da6f`), and `0x40ea7c` reads that
binding beside both cycles. Which of the two it drives is not settled, and the
port currently turns the four cockpit angles with it, which may be the wrong
one of the three.

## And it is empty in the other build too

The disc carries a second executable: `Patch v1.02/upgrade.exe` is a Win32
cabinet self-extractor holding `HELLBNDX.EXE`, 16 December 1996 against the
disc's 6 September, with a `STARTUPX.POD` of its own labelled "Startup Trial
1.02" - 1,217 entries against 1,206, the difference almost all palettes.

The same three calls are at `0x480f17` in it, the two-movie routine having
moved to `0x45b2f0`, and the third call goes to `0x459b90`:

```
00459b80  01 00 00 00 5d 5f 5e 5b 81 c4 90 00 00 00 c3 90
00459b90  c3 8d 64 24 00 8d 64 24 00 8d a4 24 00 00 00 00
```

Byte for byte what the September build has. Three months later, still one
`ret`. There is nothing to restore: it was not compiled into either.

The trip did close [[105]]'s open question. `0x481792`, at the level start,
puts `0x20000` in `0x512558` - **two units** - and `0x480021` is what pushes
the eye off the ship by it. So the jump-out's camera now leaves the cockpit in
this port as well as turning.

**Still unknown:** what view modes 1 and 2 are, separately from the four
looking directions.
