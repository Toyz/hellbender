---
title: The sky
status: solid
covers: ART\SKY.RAW, ART\NEWSKY.RAW, ART\JURASKYY.RAW, ART\*SK*.ACT
worklog: 21, 25, 30, 49
---

# The sky

Line 11 of a [.LVL](lvl.md) names the sky texture and line 12 its palette. Six
of the 26 levels name a zero-length `.VOX` instead of a texture - `space.vox`
or `stars.vox` - and those are the ones set in space, where the engine draws
stars rather than a texture.

The other twenty share three textures between them: `SKY.RAW` in fourteen
levels, `NEWSKY.RAW` in the three Hoth levels and `JURASKYY.RAW` in the three
Chimera ones. All are 64 x 64, the same as a ground texture.

## A sky palette is a gradient band, not a palette

This is the part that is not obvious. A sky `.ACT` is 768 bytes like any other,
but almost all of it is black:

```
HOTHSK.ACT     15 non-black entries, 192 to 206
FLOATSK.ACT    32 non-black entries, 192 to 223
JURASKY.ACT    32 non-black entries, 192 to 223
```

and what is in them is a ramp. `JURASKY.ACT` runs from `(163, 3, 7)` at 192
through `(255, 117, 5)` - a volcanic sky darkening toward the horizon.

## The textures index 48 above the band

A sky texture's indices do not name entries in its own palette directly.
`SKY.RAW` uses 240 to 254, `NEWSKY.RAW` 244 to 251 and `JURASKYY.RAW` 246 to
253 - all above the band, and all landing inside it once 48 is subtracted:

```
SKY.RAW       240..254   ->  192..206
NEWSKY.RAW    244..251   ->  196..203
JURASKYY.RAW  246..253   ->  198..205
```

That holds for every one of the twenty levels with a sky texture. Why the bias
is 48 is not known; it is consistent enough to rely on.

## Getting it into the frame

A sky palette shares nothing with its level's. `FLOAT.ACT` and `FLOATSK.ACT`
agree on 17 of 256 entries, and 16 of those are the reserved 240 to 255. So the
sky cannot be blitted into a frame that is in the level's palette.

The [`.MAP`](colour-tables.md) of the level's palette is the table that
bridges it: 15-bit colour to the nearest index in that palette, which is
exactly the question "what is this sky colour called here".

A `.MAP` is named after the **palette**, not the level. All 26 levels resolve
that way - `IOWAH`, `IOWAH2` and `IOWAH3` use `IOWA.ACT` and so `IOWA.MAP` -
while naming it after the level finds only 10. It follows from what the table
is: an answer about a palette.

```
sky texture index  ->  minus 48  ->  sky palette RGB  ->  level .MAP  ->  frame index
```

Rendering `JURASIC` that way gives a red-orange sky over dark rock, which is
what its palette ramp says it should be.

## How the engine draws it: a ceiling at the level's own altitude

The sky is not a dome or a cylinder. It is a flat, textured plane just above
the highest the terrain goes, drawn by `0x44fd70`:

- The routine sets up a transform about `(0, [0x5055d4], 0)`, where
  `[0x5055d4]` is the level's sky altitude - [line 30](lvl.md) of the `.LVL`
  shifted up fifteen, which is 127.5 in 24 levels, 95.0 in `KREASH` and 65.0
  in `JURASIC`. The 128.0 sitting in `0x5055d4` in the executable's data is
  only the value before a level loads. It then divides the camera's offset
  from that point by 256 (`0x44fe2b`) and builds one quad at
  `(+-0x1fffff, 0, +-0x1fffff)`. Scaling the camera and the geometry together
  does not change a projection, so this is a quad 8,192 units each way of the
  world's origin, at the sky altitude.
- Its corners' texture coordinates are a scroll offset `+-0x3fffffff`, so in
  world terms `u = scroll + 2x`, `v = scroll + 2z`, in the 256-unit texture
  space: one tile of the 64 x 64 texture every 128 units, about two units a
  texel.
- The scroll grows every frame by a velocity times the frame time (`0x44fd8a`).
  The velocity is the `.LVL`'s [line 41](lvl.md), read into `0x6670d0` and
  `0x6670d4`: 10.0, 10.0 in eleven levels - a tile every 12.8 seconds - and 0 in
  the rest. So the clouds drift.
- It is drawn at full intensity (`0x48a510(0xffff)`), with no fog.

The background routine at `0x451180` chooses by altitude. `[0x5055d8]` is 2.0,
the layer's half thickness, and nothing ever writes it:

```
eye below height - 2.0      the plane, seen from below (0x44fd70), if the
                            INI's skyTextureFlag is set; a plain one if not
within 2.0 of the height    the frame is cleared - inside the cloud
eye above height + 2.0      0x450d10, the clouds from above
```

`0x450d10` is the same plane from the other side: it advances the same scroll
by the same line 41 drift (`0x450d38`) and sets up about the same
`(0, [0x5055d4], 0)`.

Below the layer with nothing to draw the frame is simply cleared: if the eye
is at or under zero and the level has neither box cells nor chambers,
`0x451180` fills with index -1 and returns before choosing at all.

When a level has no box cells and no chambers, `0x44fd70` also draws four
walls from the plane's edges down to a 10-unit square under the eye.

`hb-render` casts each pixel's ray onto the plane, which is what a
perspective-correct rasteriser makes of the quad, and draws nothing past the
quad's edge.

## The space levels draw stars instead

The sky setup compares the name from line 11 against `stars.vox` and
`space.vox` (`0x44f783`, `0x44f7bb`) and sets `0x59d144` to 1 or 2; either way
it also blacks out the sixteen palette entries at `0x5b3620`, so a space level
has no gradient band. The background routine then calls `0x450500` for
`stars.vox` and `0x4506e0` for `space.vox` in place of the plane.

Both draw the same fixed list of points at `0x655e40` with their own colours
from `0x65bc30`. The camera's position is zeroed before the transform
(`0x450517`) and only its rotation is used, so the stars are infinitely far
away and do not move as the ship flies.

The list is 2,000 stars, generated once at startup by `0x44f6f0`: x and z
are `(rand() - 0x4000) << 8`, which is +-64.0, y is `rand() << 8`, 0 to
128.0, negated for the second thousand so the field surrounds the eye, and
the colour is `rand() >> 10`, 0 to 31. Nothing about them is in a file -
which is why a `.VOX` is zero bytes long. It only has to be named.

`0x4506e0` is `0x450500` and then a second pass over the same list with the
axes swapped and one negated (`0x450747`), so `space.vox` is the same star
field drawn twice over, twice as dense.

`ROID`, `ROID2` and `SHIP` name `space.vox`; `ROID3`, `ROID4` and `SHIP2`
name `stars.vox`.

## Notes

Ground textures never use an index above 239 - across `HOTH`'s 205 textures and
839,680 pixels the highest is exactly 239, and none is 240 or above. That is a
third independent confirmation of the reserved range from
[colour tables](colour-tables.md), reached from the art rather than from the
ramps.

They do use 192 to 223, 2.33% of pixels, so the sky's band is not carved out of
the level's palette. The two only meet through the `.MAP`.

## Unknown

Why the index bias is 48 - it holds for all twenty levels with a sky texture
and is consistent enough to rely on, but nothing says where the number comes
from. `"Sky clip overflow!"` belongs to the polygon clipper at `0x4141a0`,
which clips ground polygons against altitude 128.
