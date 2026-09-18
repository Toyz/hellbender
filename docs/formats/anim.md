---
title: The .TXT animated model
status: partial
covers: MODELS\*.TXT
worklog: 18
---

# The .TXT animated model

A skeleton of parts, each with its own mesh and a keyframe per frame. CRLF text,
keyword-delimited. The engine parses it at `HELLBEND.EXE:0x0046cd60` and writes
it back at `0x0046c240`, and every field name below is the game's own - they
come from the writer's `fprintf` calls.

Not to be confused with `DATA\*.TXT`, which is a mission briefing. A parsed
`.TXT` model becomes an [MRGL](mrgl.md) node of type 0x26.

## Layout

```
frameCount,timePerFrame        41,16384
materialCount                  7
materialList                   materialCount texture names, one per line
angle                          x,y,z
center                         x,y,z
magPower                       5461
partCount                      12

per part:
  partName                     towr
  pivot                        x,y,z
  angleList                    frameCount lines of x,y,z
  centerList                   frameCount lines of x,y,z
  parent,partHP,legFlag        -1,32768,0
  min                          x,y,z
  max                          x,y,z
  vertexCount,faceCount        2,0
  vertexList                   vertexCount lines of x,y,z
  material and faceList        faceCount pairs of lines
```

Each face is two lines: a material index, then

```
corners,nx,ny,nz,plane,i0,u0,v0,i1,u1,v1,...
```

which is **exactly** the payload of a binary [polygon node](mrgl.md) - a unit
normal in 16.16, a plane constant, and `(vertex, u, v)` per corner with the
texture coordinates as texels shifted left 16.

All 18 models in GAME.POD parse consuming every line but the trailing blank.

## Fields

`timePerFrame` is 16.16 seconds: 1,310 is 0.02 s and 16,384 is 0.25 s.

`parent` is the index of the part this one hangs off, or -1 for a root. 123 of
the 219 parts are roots. No parent index is out of range.

`partHP` is 65,535 for 156 parts and larger for the rest - 204,800 and 409,600
are the next commonest. `legFlag` is 0 for 190 parts and 1 for 29.

A face's material index of **255** means no texture. Twelve faces use it, in
`KRACKEN`, `GMRADAR` and `PROCES2`, and no model has anything near 255
materials.

## Normalisation

Every one of the 18 models has a maximum absolute vertex component of exactly
**32,767**, with no part offsets applied. That is twice the binary models'
16,384, so the two formats normalise to different bounds and a `.TXT` mesh has
to be halved before it can be drawn at a [placement's](level-text.md) scale.

## The contents

```
model      frames  materials  parts  vertices  faces
FX-4          125         23     25       495    664
KRACKEN       121         31     21       378    660
PTERYL         81          9      8       140    248
TREX           61         43     20       243    362
DRAGROW        66         11     20       188    122
SHIVANI        41         20     21       294    312
SIMIOD         41         26     16       399    532
FMDISH         41          7     12       120     52
```

and ten more. 219 parts, 917 keyframes and 4,392 faces in total.

## The part transform is not established

The raw vertices are already in model space for most parts. `TREX`'s body, two
mid sections and tail span x as `-21296..5049`, `-4484..12153`, `8769..21695`
and `19195..30618` - each starting where the previous ends, which is a
Tyrannosaurus laid out along its own axis. And with no offsets applied every
model's extent comes to exactly the normalisation bound, which it would not if
parts still had to be moved into place.

But not every part. `TREX`'s head and jaw are centred on the origin
(`-8505..8505` and `-7108..7107`) and clearly belong at the front of the body.
Three candidate placements were tried and all three are wrong:

| tried | result on `TREX` |
| --- | --- |
| raw vertices | max component 32,767 - the normalisation bound, correct |
| plus `pivot` | 274,257 - eight times the bound |
| plus `pivot` summed up the parent chain | 420,304 |
| plus `pivot / 2` | 153,512 |
| plus `centerList[0]` | 34,384 - close, but over |

So `pivot` is a rotation origin rather than a translation, `centerList` is a
per-frame offset from the rest pose rather than the pose itself, and something
else places the head. Rendering the rest pose gives a body and tail that
assemble correctly with the head and jaw floating above them.

## Unknown

How a part is placed relative to its parent. The order the three per-frame
angles compose in. What `magPower`, the model-level `angle` and `center`, and
`legFlag` do.
