---
number: 49
title: The sky is not always at 128
date: 2026-09-20
area: decomp,engine,port
files: crates/hb-render/src/level.rs,crates/hb-render/src/scene.rs,docs/formats/sky.md
---

# 49. The sky is not always at 128

Worklog 30 read the sky plane out of `0x44fd70` and found it built about
`(0, [0x5055d4], 0)` with `[0x5055d4]` holding 128.0. That was the value in
the executable's data, before a level loads. Worklog 48 found what writes it:
the `.LVL`'s line 30, shifted up fifteen. So the plane is per level - 127.5 in
24 of them, 95.0 in `KREASH`, 65.0 in `JURASIC` - and `hb-render` had 128.0
as a constant.

It is a level's ceiling as much as its sky. The background routine at
`0x451180` picks by where the eye is relative to it: below it less two units,
the plane seen from underneath; within two units, a cleared frame, because the
eye is inside the cloud; above it, `0x450d10`, which is the same plane from
the other side, advancing the same scroll by the same line 41 drift. The two
units are `[0x5055d8]`, a constant nothing ever writes.

The renderer now takes the altitude from the manifest. Chimera's sky sits
where it should, half the height of everyone else's.

## The space levels

The other half of the sky page was what the six levels with a `.VOX` draw.
The sky setup compares line 11 against `stars.vox` and `space.vox` and sets
`0x59d144` to 1 or 2, and blacks out the sixteen palette entries the sky
gradient would use; the background routine then calls `0x450500` or
`0x4506e0` instead of the plane.

Both walk a list of points at `0x655e40` with their own colours at
`0x65bc30`, and both zero the camera's position before transforming, so only
its rotation reaches the stars: the field is infinitely far away and never
moves. `0x4506e0` is `0x450500` and then a second pass over the same list
with the axes swapped and one negated, which is how `space.vox` gets twice
the stars out of one list.

The list itself is generated once at startup by `0x44f6f0`, and this is the
part that explains the `.VOX` being zero bytes long: 2,000 stars, x and z
`(rand() - 0x4000) << 8` and so +-64.0, y `rand() << 8` and negated for the
second thousand so the field surrounds the eye, colour `rand() >> 10` - 0 to
31, which in every level palette is the grey ramp from black to white. A
`.VOX` holds nothing because there is nothing to hold. It only has to be
named.

`hb-render` generates the same field with the same C runtime `rand` and draws
it, one pixel a star, before anything else. `ROID` has a sky now.

Two things left on that page: why the sky texture's indices sit exactly 48
above the palette band they name, and `"Sky clip overflow!"`, which belongs to
the ground clipper rather than to the sky.
