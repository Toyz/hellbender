---
title: The .TXT animated model
status: solid
covers: MODELS\*.TXT
worklog: 18, 19, 56
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

## Normalisation, and why it is not a constraint

Every one of the 18 models has a maximum absolute vertex component of exactly
**32,767** as authored, with no part offsets applied. That is twice the binary
models' 16,384, so a `.TXT` mesh has to be halved before it can be drawn at a
[placement's](level-text.md) scale.

The engine does not treat 32,767 as a bound the data must respect. `0x4664e0`
walks every part of a loaded model, finds the largest absolute vertex
component across all of them, and scales every vertex **and every keyframe
centre** by `0x7fff` over that maximum - so the two are in one space, and
since the shipped files already max at 32,767 the factor is one and nothing
moves.

It runs once, at load, before any pose. A part's centre then pushes it
outside the bound and nothing pulls it back, which is why `FX-4` poses five
model-widths across: "the extent is too large" is not evidence that a
transform is wrong.

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

## How a part is placed

Each part is drawn with a transform built from its own interpolated angle and
centre, and **nothing else**. `0x4684c0` turns the actor's clock into a frame
index and a fraction, interpolates every part's angle and centre between that
keyframe and the next into the part record at `+0x48` and `+0x54`, and
`0x467980` hands the six values straight to `0x42aa30`, the routine that
pushes a transform. Then it walks the part's vertices.

So a part's `centerList` entry is **where it goes in model space**, not an
offset from its parent. `pivot` and `parent` take no part in the draw at all -
they are the authoring tool's, kept in the file and loaded but never read
again. That is why summing the pivot chain put `TREX`'s head between its
shoulders: there is no chain.

Under all of them sits the model's own `angle` and `center`: `0x4681d0` builds
a matrix from the angle into the header at `+0x634`, and the parts are drawn
inside it. `TREX`'s is `49664,0,0` - about -87 degrees about x, which is a
z-up model being turned y-up, and every shipped model's is within a degree or
two of the same.

The interpolation is worth one detail: an angle's difference between
keyframes is sign-extended from sixteen bits before it is scaled
(`0x468585`), so an angle takes the short way round rather than unwinding
through the long side. Centres interpolate straight. The last keyframe
interpolates back to the first, so the animations loop.

`hb_formats::anim::pose` is this, and it is checked the way a model is really
checked: `TREX`'s head comes out in front of its body, its jaw under its head,
its legs under it, and its arms and legs mirrored either side; `PTERYL`'s
three wing segments come off each side, each further out than the last.

The parts do push past the normalisation the vertices were scaled to, and the
engine does not renormalise afterwards, so `FX-4` and `DRAG66` are several
model-widths across when posed. That is the data, not an error.

## The in-memory model

The engine's loaded form is 15,712 bytes, which is the size the
[MRGL](mrgl.md) table gives a type 0x26 node, and it accounts exactly:

```
0x0000  header, 1,632 bytes
0x0004  frameCount            0x0010  timePerFrame
0x0018  materialCount         0x0024  material names, 24 bytes each
0x061c  angle                 0x0628  center
0x0658  magPower              0x065c  partCount
0x0660  parts, 220 bytes each, 64 of them
```

`1632 + 64 * 220 = 15712`. So a model may have at most **64 parts**; the
largest shipped has 25.

A part:

```
+0x00  name
+0x10  pivot x, y, z
+0x1c  pointer to the angle list
+0x20  pointer to the centre list
+0x24  parent, partHP, legFlag
+0x30  min x, y, z           +0x3c  max x, y, z
+0x48  this frame's angle     0x54  this frame's centre    (runtime)
+0x84  vertexCount           +0x88  faceCount
+0x8c  pointer to the vertices, 36 bytes each in memory
```

The two runtime fields are what `0x4684c0` writes each frame and `0x467980`
draws with. They are also how the rest of the engine asks where a part is:
class 14's aim takes a part's `+0x54` through the model matrix (`0x46edd0`)
to find its gun.

## Unknown

What `magPower` and `legFlag` do, and what `pivot` and `parent` were for in
the tool that wrote them. What the 24 bytes beyond the coordinates of an
in-memory vertex are. Where an actor's animation clock comes from - this port
runs every model off the level's clock, which is right for the eighteen
models that have one animation each.
