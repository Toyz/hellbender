---
title: Snow, rain and lightning
status: partial
covers: LEVELS\*.LVL line 40, HELLBEND.EXE:0x49c9b0, 0x49ce80, 0x49d220, 0x49d290
worklog: 114
---

# Snow, rain and lightning

The `.LVL`'s line 40 is a bitmask, and the file says so itself:
`{ Weather (none=0,snow=1,2=rain,4=lightning,6=rain+lightning)`. The parser
puts it at level struct `+0x41c`, `0x6670cc`, and four routines read it. `HOTH`,
`HOTH2` and `HOTH3` snow; `IOWAH` and `MORBOS` and their sequels rain and have
lightning. See [the .LVL](../formats/lvl.md).

## Snow and rain are a box around the eye

The frame (`0x4811e0`) draws snow (`0x49c9b0`, bit 0) and then rain
(`0x49ce80`, bit 1) after the world and before the cockpit, the radar and the
HUD. Both are the same machine with different numbers:

```
           pool                       fall             drawn as
snow       300 x 24 bytes, 0x5be4b0   0xfffb1e00  4.88   a dot
rain       200 x 36 bytes, 0x5bc890   0xfff63c00  9.77   a streak
```

As the level starts (`0x49c8d0` for snow; `0x49ce70`, a jump to `0x49cd30`,
for rain) every particle is put at the eye plus `((rand() << 8) & 0xfffff) -
0x80000` on each axis - anywhere in a 16-unit cube centred on it - and given a
velocity of the fall in y and `(rand() & 0x7ff) - 0x400` in x and z, a drift of
under a sixty-fourth of a unit a second. Both pools are filled whatever the
level's weather.

Every frame, each particle moves by its velocity times the frame time. Then,
axis by axis, if it is more than eight units from the eye it moves sixteen
units back the other way, and on a frame when that happened it is not drawn.
So the cube travels with the eye and never empties.

None of that happens - the routine returns before moving anything - unless
the eye is at or above the ground (`y >= 0`), at or below the sky layer
(`0x5055d4`, [line 30](../formats/lvl.md)), and nothing is over it: `0x41c4d0`
at the eye must answer `0x1000000`, which above the ground means the eye is
not below the underside of a box set A cell. Fly under an overhang and it
stops falling; fly into a tunnel and it is gone.

**A flake** is projected through the view matrix, dropped if it is behind the
eye or outside the square frustum, and written by `0x486e20`
(`draw320x200SizeDot`) as one 320x200 pixel whatever the mode: 1x1 at 320x200,
1x2 at 320x400, 2x2 at 640x480. No depth test.

**A drop** keeps a second vector at `+0x18`, the opposite of its fall as a unit
vector - straight up. `0x49d070` draws a line from the drop to the drop plus
`0x4e20` (0.305 units) times the unit vector of that up vector plus how far the
eye moved this frame (`0x62d6d4..dc`, which the flight step at `0x464cbe`
writes as the camera's position less its copy from before the step). At rest
the streak points up; flying, it leans along the travel - the drop's motion as
the eye sees it. The line goes through `0x459a40`, the same clipped
two-corner line the rest of the engine uses.

Both take their colour from `0x4566c0(1, t)`, which picks a palette index
`t` of the way along colour ramp 1 - indices 0 to 15, from the tables at
`0x50c4e8` and `0x50c528`. Snow asks for `0xffff`, index 14; rain for
`0x6000`, index 5. The pixel writes then go through the 256-byte remap at
`0x606a20`, as every dot and line does.

## Lightning

`0x49d220` runs as the level loads with the two numbers of [line
42](../formats/lvl.md), 15.0 and 30.0 in 16.16 in every level, and when bit 2
is set arms one of five 40-byte slots at `0x5bc7c0`: a countdown of `base +
rand() % range + 1`, done on the 16.16 values as they are. `rand()` never
reaches 30.0 in 16.16, so that is 15 seconds and a random part under half a
second.

`0x49d290` runs each frame the eye is at or above the ground. For each armed
slot, when its countdown runs out:

- the countdown restarts at `base + rand() % range` - again 15 to 15.5 seconds
- the strike is placed at the sky layer's altitude, within 40 units of the eye
  in x and z: `((rand() << 8) % 0x500000) - 0x280000`
- `lghtng.wav` plays there (`0x41f0b0`, priority 4)
- the thunder is timed: the distance to the strike (`0x42b960`) times
  `7.3982e-7` seconds, which is sound at 20.6 units a second
- the models' light floor goes to full: the level's ambient (`0x666f40`, line
  19) is saved and replaced with `0xffff`, and `0x48a640` puts that in
  `0x51138c`, which the polygon lighter `0x48a6a0` reads
- the sky brightens: `0x451450` copies a lit copy of the sky tile into the
  texture page over the real one

While the flash lasts, half a second (`+0x1c = 0x8000`), a bolt is drawn at
the strike each frame through the draw callback `0x49d490`, which the strike
puts in the draw list (`0x42fa90`) at the strike's x and z and the eye's
height. When it ends, the ambient goes back and `0x4514e0` copies the saved sky
tile back. When the thunder's delay runs out, `thun-c.wav` plays at the strike.

**The bright sky** is made once, as the level's sky is set up:
`initBrightSky` (`0x451290`, which refuses a sky tile over 64 on a side) walks
the tile in the texture page, saves each texel to `0x53d988`, and writes a
lit one to `0x53c988`. Lit is `0x451570`: the texel's colour scaled by up to
twice, but only so far as takes its brightest channel to 255, so the hue
holds; then `0x485080` finds the nearest palette index, measuring
`29|dr| + 58|dg| + 15|db|` and keeping the first of equals. The storm levels'
skies are so dark - `PURPSKY.ACT` and `CAVSKY.ACT` come out as black and one
dark red or grey in their levels' palettes - that twice as bright is still the
same index, and a flash leaves them as they were.

**The bolt** is `0x49d640`, drawn only while the eye is below the sky layer.
From the strike it steps down to the floor (`0x41c300`): each step drops
`(rand() << 16) & 0x3ffff` - 0 to 3 whole units - and moves `((rand() << 16)
& 0x1ffff) - 0x10000` in x and in z. The mask keeps only `rand()`'s lowest bit,
so a step moves nowhere or one unit toward -x and -z, never the other way,
and every bolt leans the same way. Each step is a line in palette index 110
or 111, picked per step, and the whole bolt is new each frame, so it
flickers. A recursive version that forks (`0x49d4c0`) is in the binary and
never called.

## What the port does

`hb_sim::weather` is all of the above: the pools, the scatter, the move and
wrap, the gate, the streak's direction, the strike's timing and place, and the
bolt. `hb-render`'s `draw_flake` and `draw_streak` draw snow and rain straight
into the frame after the world with no depth test, the way the engine does;
`Level::sky_remap_lit` is the bright sky, and during a flash `hb-fly` swaps it
in and lights models at full ambient.

Three differences are the port's. The remap at `0x606a20` is taken as the
identity, as the HUD already takes it. A streak leans by how far the eye moved
in one of the port's frames, which at a higher frame rate than the original's
is less far, so the rain stands straighter. And the bolt is drawn after the
world with a depth test on each pixel, where the engine sorts it into its
draw list at one point and paints.

## Unknown

How the remap at `0x606a20` is built.
