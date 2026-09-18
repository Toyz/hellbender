---
title: The sky
status: partial
covers: ART\SKY.RAW, ART\NEWSKY.RAW, ART\JURASKYY.RAW, ART\*SK*.ACT
worklog: 21, 25
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

## Notes

Ground textures never use an index above 239 - across `HOTH`'s 205 textures and
839,680 pixels the highest is exactly 239, and none is 240 or above. That is a
third independent confirmation of the reserved range from
[colour tables](colour-tables.md), reached from the art rather than from the
ramps.

They do use 192 to 223, 2.33% of pixels, so the sky's band is not carved out of
the level's palette. The two only meet through the `.MAP`.

## Unknown

Why the index bias is 48. How the engine projects the texture - it has a
`skyTextureFlag` setting and a `"Sky clip overflow!"` diagnostic and nothing
else legible. What it draws for the six levels whose sky is a zero-length
`.VOX`.
