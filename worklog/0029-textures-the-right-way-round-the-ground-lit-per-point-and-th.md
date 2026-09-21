---
number: 29
title: Textures the right way round, the ground lit per point, and the light in the manifest
date: 2026-09-17
area: decomp,render,format,port
files: crates/hb-formats/src/terrain.rs,crates/hb-formats/src/lvl.rs,crates/hb-render/src/raster.rs,crates/hb-render/src/scene.rs,crates/hb-render/tests/world.rs,crates/hb/src/view.rs,crates/hb/src/main.rs,Cargo.toml
slug: textures-the-right-way-round-the-ground-lit-per-point-and-th
---

# 29. Textures the right way round, the ground lit per point, and the light in the manifest

"I don't think we render the world right." Four things were wrong, all read out
of the engine's ground and box drawers this time rather than guessed.

## How a texture lies on a cell

The orientation routine `0x413940` had been found but not pinned, because its
corner order was unknown. Its callers pin it. `0x413c20` - the ground's texture
setup, one caller, `0x4148e6` - writes four vertices' coordinates before calling
it with `(0, 1, 2, 3, word)`:

```
vertex   u     v        lo = 0x20000 (2.0), hi = 0xfe0000 (254.0)
0        lo    hi
1        hi    hi
2        hi    lo
3        lo    lo
```

The vertices are copied from the projected-grid array at `0x526038` (22 wide,
built by `0x414bf0` around the eye) at `(gx, gz)`, `(gx+1, gz)`,
`(gx+1, gz+1)`, `(gx, gz+1)`, and the shade indices at `0x414e05` agree. So u
runs with x and **v runs against z**. The port had v running with z: every
ground texture in the port was mirrored.

`0x413940` itself: `r = code >> 2` shifts the four corners' coordinates round
by `r` (a quarter turn per step, since the corners run round the square);
`code & 2` swaps a/b and d/c, a mirror in u; `code & 1` swaps a/d and b/c, a
mirror in v. The port ignored the code entirely. The top-down map now shows
it: FLOAT's walkways, which came out as sideways-offset segments, run straight
and join (`hb ground float out.png`, against `--authored`).

The data backs the base convention hard. Laying every untouched tile the
engine's way makes neighbours' shared edges match with a mean texel difference
of 46.9 in HOTH; the best of the other seven ways a square can lie scores 77.2
(KREASH 73.1 against 91.8, JURASIC 53.7 against 78.6) -
`untouched_tiles_meet_best_the_engines_way_round`.

It backs the codes more weakly, and I spent a while on why. Averaged edge
segments rank the code's own reading first or second of eight in 59 per cent
of 2,031 turned cells, against 25 by chance. Code 4 agrees best; codes 8 and
12 are often matched as well by a mirror. Most turned tiles are noise textures
turned to hide repetition, which fit any way round, and a texel-by-texel
comparison even ranked the engine's reading below the authored one on FLOAT -
a test I wrote, watched fail, and took out. The transition tile that decides
it by eye, KREASH's `KMDRIBDT` at (100, 72), grey along its first row and
brown along its last between brown and grey neighbours, is right only under
the code's reading. The disassembly is not ambiguous, and the port follows it.

## Box faces, with signs

The box drawer calls `0x414f60` from six sites, one per face, each with its
own slot word, back-face normal, neighbour check and quad of box vertices
(0-3 bottom, 4-7 top in the ground's corner order; 8-15 copies made by a
`rep movs` after each face):

```
slot 0  +4   normal (0,0,-1)  neighbour z-1  quad 0 1 5 4    -z
slot 1  +6   (0,0,+1)         z+1            2 3 7 6         +z
slot 2  +8   (+1,0,0)         x+1            1 2 6 5         +x
slot 3  +10  (-1,0,0)         x-1            3 0 4 7         -x
slot 4  +12  (0,+1,0)                        12 13 14 15     top
slot 5  +14  (0,-1,0)                        8 9 10 11       bottom
```

`0x414f60` gives its quad the same starting coordinates as the ground and the
same orientation routine. That closes the sign question the measurement in
worklog 20 could not (81 against 77). A side is skipped when the neighbouring
box covers it, and the ground drawer skips a cell whose four corners lie
inside its box A (`0x414d43`).

## The ground is lit per point, and the light is in the `.LVL`

Each of a ground cell's four vertices takes the shade of the cell whose origin
it is (`0x414e05` to `0x414ea7`): `byte << 8`, or `[0x525c5c]` when bit 8 of
the shade word is set. The port lit each cell flat. Now it Gouraud shades, and
the mountains that had a saw-tooth of light and dark triangles are smooth.

`[0x525c5c]` is copied from `0x666f40` (`0x419fda`), and `0x666f34..54` turned
out to be lines 18-22 of the `.LVL`: the parser at `0x44bb88` reads line 18
into level-struct `+0x284` - the struct is at `0x666cb0`, after sixteen 40-byte
filename slots from `+4` - so line 18 is `0x666f34`. Line 18 is a light
direction, the unit vector (-0.707, -0.707, 0) in every level; line 19 an
ambient intensity, and exactly each level's floor in the shading database
(HOTH 16384 and 64, FLOAT 40960 and 160, ROID 24576 and 96); lines 20 and 21
the same pair for the chambers (read at `0x41d161` beside the chamber array).
The manifest doc had them as "a position" and "a heading". Bit 8 is therefore
"this point is in shadow".

A box's `.LTE` byte, which the port had used as an intensity and which drew
every box black, is eight shadow bits: `0x41c8cd` clears it and for each
corner casts 48 units toward the light through `0x413580`. The box drawer does
the same test and gives a shadowed corner the ambient and a lit one full light
(`0x415e45`).

The chamber floor and ceiling go through the ground's cell routine from
`0x41911a` (word at chamber `+4`) and `0x41957a` (`+6`, flip flag set), which
confirms the floor-first order the port had assumed.

## Also

- The rasteriser interpolates texture, light and depth perspective-correctly.
  FLOAT's floor, a smear of stretched streaks under affine mapping, is a
  tiled floor.
- The frame is filled with the fog ramp's last-row colour before the sky, so
  the band past the draw distance is fog.
- `hb bench <level>` draws a turn in place: HOTH at 320x200 is 148 frames a
  second in release, 24 unoptimised. The workspace now optimises the hot
  crates in dev builds too (114 a second), which is the likely answer to the
  frame-rate complaint.

Tests: four orientation unit tests in `hb-formats`,
`texture_coordinates_are_perspective_correct`,
`light_is_interpolated_across_a_triangle`,
`untouched_tiles_meet_best_the_engines_way_round`. 100 in all.

**Still unknown:** the chamber's 24-bit shade; which box shadow bit is which
corner past the first two; the sky's projection; what the `[0x5d29e0] == 256`
case is.
