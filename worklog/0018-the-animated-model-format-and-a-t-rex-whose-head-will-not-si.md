---
number: 18
title: The animated model format, and a T-Rex whose head will not sit down
date: 2026-09-17
area: format, render, port
files: crates/hb-formats/src/anim.rs, docs/formats/anim.md
slug: the-animated-model-format-and-a-t-rex-whose-head-will-not-si
---

# 18. The animated model format, and a T-Rex whose head will not sit down

`MODELS\*.TXT` is the last format the levels need and the easiest one so far,
because it is keyword-delimited text and the engine's own writer at `0x46c240`
names every field. All 18 models in GAME.POD parse consuming every line but the
trailing blank, which is the check that nothing was skipped or double-counted.

```
frameCount,timePerFrame / materialCount / materialList / angle / center /
magPower / partCount

per part: partName / pivot / angleList / centerList / parent,partHP,legFlag /
          min / max / vertexCount,faceCount / vertexList /
          material and faceList
```

A face is two lines - a material index, then
`corners,nx,ny,nz,plane,i0,u0,v0,...` - which is **exactly** the payload of a
binary [polygon node](0009-the-polygon-node-and-a-ship-that-looks-like-a-ship.md).
Same normal, same plane constant, same texel coordinates. The text format is
the binary one written out.

219 parts, 917 keyframes and 4,392 faces across the 18. `FX-4` has 125 frames
and 25 parts; `TREX` has 61 frames, 20 parts and 43 materials.

## Two small findings

A face's material index of **255** means no texture - twelve faces use it, in
`KRACKEN`, `GMRADAR` and `PROCES2`, and no model has anything near 255
materials. The same situation as the binary models'
[flat-coloured polygons](0017-materials-bind-to-polygons-by-position-and-four-models-say-n.md),
reached by a different mechanism.

And the two model formats **normalise differently**. Every `.TXT` model has a
maximum absolute vertex component of exactly 32,767; every `.BIN` model has
16,384. A factor of two, so a `.TXT` mesh has to be halved before it can be
drawn at a placement's scale. Nothing announces this; it only shows up if you
measure both.

## The part transform, and three wrong guesses

The interesting part is what I could not work out.

Most parts' vertices are already in model space. `TREX`'s body, two mid
sections and tail span x as `-21296..5049`, `-4484..12153`, `8769..21695` and
`19195..30618` - each starting where the previous ends, which is a
Tyrannosaurus laid out along its own axis. And with no offsets applied every
model's extent comes to exactly the normalisation bound, which it would not if
the parts still had to be moved into place.

But the head and jaw are centred on the origin and plainly belong at the front.
So something places them, and it is none of these:

```
raw vertices                          32,767   the bound, correct
plus pivot                           274,257
plus pivot summed up the parent chain 420,304
plus pivot / 2                       153,512
plus centerList[0]                    34,384   close, but over
```

My first implementation used `pivot + centerList[frame]`, which is the obvious
reading of "a pivot and a keyframe centre", and it renders a T-Rex as twenty
fragments scattered across the frame. The rest pose - raw vertices, nothing
added - renders a body and a tail that assemble correctly with a head and a jaw
floating above them.

That is the honest state: `pivot` is a rotation origin, `centerList` is a
per-frame offset from the rest pose rather than the pose itself, and how a part
is placed relative to its parent is **unknown**. I am recording the three
measurements that rule out the easy answers so the next attempt starts past
them.

The port draws the rest pose, which is right for the majority of parts and
visibly wrong for a few. `FLOAT` now has all 293 of its placements drawable
where it had 269, and `JURASIC` all 406 where it had 298.

**Still unknown:** how a part is placed relative to its parent, and the order
the three per-frame angles compose in. What `magPower`, the model-level `angle`
and `center`, and `legFlag` do.
