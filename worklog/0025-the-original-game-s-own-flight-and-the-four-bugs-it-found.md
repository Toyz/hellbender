---
number: 25
title: The original game's own flight, and the four bugs it found
date: 2026-09-17
area: format, world, render, port, test
files: crates/hb-formats/src/demo.rs, crates/hb-world/tests/demo.rs, crates/hb-render/src/camera.rs, crates/hb-world/src/grid.rs
---

# 25. The original game's own flight, and the four bugs it found

Everything checked so far has been checked against files - the format parses,
the counts add up, the picture looks right. `DEMO\DEMO1.DMO` is something else:
50.8 seconds of the original engine flying over `IOWAH2`, recorded pose by pose.
It is the first ground truth about the game's *behaviour* rather than its data,
and it found four bugs in code that had passed every other test.

## The format

A record count, the level, then that many tagged records. `tag,time` first;
tag 0 is followed by a position and four angles, tag 1 by a key's scan code.

My first parse assumed every record was a pose and broke after 70 records, on
`1,232086` followed by a lone `57`. 57 is `fireKey` in `HELLBEND.INI`. Once the
tag is honoured all three demos parse consuming every line, with exactly as
many records as declared and the time never running backwards. 636 of the 638
key presses are the fire button.

Two of the three demos name levels that are not in GAME.POD - `red.lvl` and
`atmos-t2.lvl`. Development levels that were cut, whose demos shipped anyway.

## Nothing underground, and a test that can fail

Of `DEMO1`'s 890 poses, none is below the port's reconstructed ground, and the
closest approach is about five units. I first wrote "exactly 5.00" from a
two-decimal printout; no pose is within a sixty-five-thousandth of five, so the
test asserts a range.

A pass is only worth something if the test can fail, so it runs against
deliberately wrong terrain too:

```
the port's reading          0 of 890 underground
twice the height scale    265
x and z swapped            99
HOTH instead of IOWAH2    746
```

That confirms the `<< 15` height scale from
[8](0008-half-the-world-was-in-the-wrong-hemisphere-and-the-diagonal-a.md), the
axis assignment, and the centred coordinates from
[15](0015-the-world-is-centred-and-def-is-a-table-of-types-not-a-list-.md) -
against the running engine rather than against each other. It does not confirm
the triangle split: a uniform diagonal passes too, because the pilot never gets
close enough to the surface for it to matter. Saying what a test does not prove
is part of the test.

## Bug one: the height was wrong for half the world

The Rust version of that check failed where the Python one passed. `height_at`
took a position's fraction across its cell as `x - cell.origin()`, and
`origin()` is the masked index shifted up, so that is only right for
coordinates in 0 to 1024. The world's coordinates are signed. For anything
negative the fraction came out as something like -127.5 and the height was
nonsense.

The engine does `and eax, 0x7ffff` - takes the low bits directly - and that is
the fix. My synthetic ramp test never went negative, so it never saw it; the
demo spends most of its time at negative x. The same query drives `hb-fly`'s
collision, which was therefore wrong whenever you flew out of the positive
quarter of the map. There is now a synthetic test that asserts a signed
position and the same position one world over give the same height.

## Bug two: the camera turned the wrong way

The third recorded angle is the heading: it equals `atan2(dx, dz)` of the
direction of travel to a median 0.8 degrees over 790 samples, and every other
pairing is 60 to 135 degrees off. So the engine's heading is 0 along +z and
increases toward +x. Entry 13 had recorded "nothing ties the heading to a world
axis" and picked a convention; this is the tie.

The port's camera had it backwards - a quarter turn looked along -x. `hb-fly`'s
motion followed the engine's convention, so after turning ninety degrees the
ship flew sideways while the camera looked ahead. Nobody noticed because nobody
turned ninety degrees and then checked which way the ground went past. The
camera now rotates by +heading, and a test asserts it looks the way a ship on
each of eight headings travels.

The first angle is pitch, and positive is **nose down** - the sign is opposite
to the climb in all 637 samples steep enough to test, no exceptions - which the
camera already had right.

## Bug three: the wrong colour map for a third of the levels

Replaying `DEMO1` nose-up showed a black sky. `IOWAH2` has no `IOWAH2.MAP` and
no `IOWAH.MAP`; its palette is `IOWA.ACT` and its colour map is `IOWA.MAP`.

A `.MAP` answers "the nearest index in this palette", so it is named after the
palette. All 26 levels resolve that way; naming it after the level finds 10,
and the family fallback I had written from
[21](0021-a-sky-palette-is-not-a-palette-and-the-cockpit-needs-no-remap.md)
finds 21. I had tested the sky on `HOTH` and `JURASIC`, whose palettes happen to
share their level's name.

## Bug four: a negative pitch was nearly a whole turn

With the map fixed the sky was still black. The sky is drawn by elevation, and
`elevation - pitch` is a linear use of the pitch. A nose-up pitch of -5219
stored as a `u16` is 60317, which `to_radians` reads as 331 degrees. Sine and
cosine do not care - which is why the terrain and the camera were fine - but a
subtraction does.

`Angle::to_signed_radians` reads it as -28.7 degrees, and the sky uses it. The
doc comment says when to use which, because the failure mode is invisible
anywhere the angle only goes into trigonometry.

## Replaying it

`hb-fly --demo 1` puts the camera wherever the original recorded it, looping,
with the level's music playing. `hb demo 1 38 out.png` renders a single instant.
At 38 seconds the ship is climbing with its nose 29 degrees up into `IOWAH2`'s
dark purple storm sky; at 5 seconds it is diving on a structure.

Four bugs, and every one of them was in code that passed its tests. The tests
were right about what they checked. What they checked was a world I had built,
and the demo is a world the original built.

**Still unknown:** whether the second angle is roll, and what the fourth
value's 0 and 1 mean. Whether the time unit really is 16.16 seconds rather than
merely giving a plausible length.
