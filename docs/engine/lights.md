---
title: Lamps and the lights they give
status: partial
covers: DATA\*.GLT, HELLBEND.EXE:0x48bd60, 0x48b6b0, 0x48ae30, 0x48c800, 0x48b550, 0x413d80, 0x418a60, 0x4144b0
worklog: 119, 120
---

# Lamps and the lights they give

A lamp is a face of the world painted with a light's texture: a window on a
building, a strip on a tunnel ceiling. [`.GLT`](../formats/scenery.md) says
which textures those are. The lamps light what is near them, blink when
damaged, and go out when shot. Shots and missiles carry lights of their own
while they fly.

## The record

A `.GLT` record is 92 bytes at `0x5d05f0 + 92 * n`: three sixteen-byte names,
lit, unlit and broken; then the texture indices the loader finds for them at
`+0x38`, `+0x3c` and `+0x40`; and the eight numbers from `+0x30`, the first
two before the indices and the other six after.

```
 #  offset  shipped              what
 0  +0x30   131072..655360       reach: times eight, units (2.0 -> 16, 4.0 -> 32)
 1  +0x34   90000, 65535         strength, 1.0 full
 2  +0x44   131072, 32768        a blinking lamp's time on, 16.16 seconds
 3  +0x48   6                    and off, compared raw: a frame
 4  +0x4c   1                    which lights a face gives: 1, 2 and 4
 5  +0x50   32768                a blinking-from-the-start lamp's phase
 6  +0x54   0, 4, 6              shots to break it; 0 breaks on the first
 7  +0x58   1..8                 it blinks with this many shots left or fewer
```

## Finding them

As the level loads (`0x44c729`), `0x48bd60` walks every cell and every face in
it - the ground, the chamber floor and ceiling, the six faces of box set A and
of box set B - and compares the texture's low twelve bits with every record's
lit and unlit index. Each face that matches goes to `0x48b6b0` with its
surface (0 ground, 1 box A, 2 floor, 3 ceiling, 4 box B), its cell, its face
and where it is, and becomes up to three lamps of 80 bytes at `0x5c13a0`,
counted at `0x5cafe0`, 500 at most ("Too many light sources!!"):

- bit 2 of the fifth number: a **cone**, the record's reach, 45 degrees about
  the face's normal (`cos(0x2000)`)
- bit 4: a **flat** light of eight units
- bit 1: a **round** light of the record's reach

Every shipped record sets only bit 1. A lamp starts lit or unlit by which of
the two textures its face wears, and blinking if the record has a blink time
and its eighth number is at least its seventh. It gets the seventh number as
its hits.

## Each frame

`0x48a770` empties the frame's lights - 44 bytes each at `0x5caff0`, counted
at `0x511388`, 500 at most ("Too many lights1!") - and `0x48ae30` walks the
lamps. One more than ten cells from the eye on either axis is skipped. A lit
lamp goes into the frame's lights. A blinking one counts its clock: off, it
comes on once the clock passes the fourth number and its face is painted
lit; on, it goes off once the clock passes the third and its face is painted
unlit - the top four bits of the texture word kept. The shipped numbers make
that on for two seconds, off for a frame.

Things in flight add their own through `0x48a7f0`: every fourth shot drawn
(a counter at `0x50f028`) a round light of reach `1.414 * 8` and strength a
quarter (`0x476e99`), and each missile one of the same reach. `0x48a9e0`, from
the player's step, adds another; which, is not read.

`0x48b3c0` then sorts the frame's lights into a 21-by-21 grid of cells around
the eye (`0x58c448`), each cell holding up to ten, so that a question about a
point only walks the lights in its cell.

## What they light

`0x48b550` answers "how much light is at this point": for each light in the
point's cell that reaches it on every axis, `0x48b1e0` weighs it by kind -

- round: strength times `(reach - distance) / reach`
- cone: full strength if the point is within 45 degrees of the normal, else none
- flat: full strength

- and the sum is clamped to 0..1. The box test is per axis, so a round light's
weight goes negative in its box's corners.

The object draw (`0x42f4f0`) asks it for any object **below the ground**: the
light at the object is added to the ambient and the sun is turned off
(`0x48a670(0, 0, 0)`) while the object is drawn. That is what lights the
tunnels' machinery.

The world's own corners ask too, and take the answer on top of their shade:

- `0x413d80` asks at every ground vertex from ten cells before the eye to
  eleven after, on both axes, at the ground's height there, and keeps the
  answers in rows of 22 indexed from the eye's own vertex at `0x52f390` - the
  array starts 230 entries before it, at `0x52eff8`, which is where the
  ground's cell draw (`0x414bf0`) reads its four corners from.
- `0x418a60` does the same for the chamber floor (eye at `0x52a7f0`) and
  ceiling (`0x525680`) at their heights, read by the floor's draw
  (`0x418d70`, from `0x52a458`) and the ceiling's (`0x419230`).
- The box draw (`0x415420`) asks at a box's eight corners as it draws it
  (`0x415825`), bottom then top, each in the ground's order.

`0x4144b0`, the cell routine the ground and chambers draw through, adds each
corner's answer to its light (`+0x14`) before drawing; the box draw adds them
itself for each face (`0x4165ab`). A corner's light is its shade shifted up
eight, so a whole light adds 256 to a shade of 0 to 255. Nothing clamps the
sum there.

## Shooting one

A player's shot or missile that stops against the world calls `0x48c800`
(`0x476d7b`, `0x478910`). It finds which surface the point is on (`0x41ba50`,
`0x41bc00`: below the ground the chamber floor, box set B's underside or the
ceiling; above it box set A or the ground) and, for a box, which face
(`0x4294c0`). If that face wears a light with a broken texture named, every
lamp on that surface of that cell that is not already broken loses a hit. At
none left it is broken and the face is painted with the broken texture - or
the lit one when the record names none - and the shot's mark becomes a burst
of size 1.0 (`0x476db4`). With the eighth number or fewer hits left it starts
to blink.

## What the port does

`hb_sim::lights` is all of the above but the grid, which it does without -
the per-axis test is the same, and only the ten-per-cell cap is lost.
`hb-render` loads the lamps with the level and lights objects below the
ground with them, and every ground, chamber and box corner near them;
`hb-fly` steps them, paints their faces, adds every fourth shot's light and
sends shot hits to them. The port asks at each corner as it draws rather than
filling the arrays.

## Unknown

What `0x48a9e0` adds; a missile's light's strength; what the span routine
does with a light past full; and `0x48bb50`, which the scan asks where a box
face is - the port takes the middle of the face.
