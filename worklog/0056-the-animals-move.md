---
number: 56
title: The animals move
date: 2026-09-20
area: decomp,format,render,port
files: crates/hb-formats/src/anim.rs,crates/hb-formats/tests/against_the_game.rs,crates/hb-render/src/level.rs,crates/hb-fly/src/main.rs,crates/hb/src/main.rs,docs/formats/anim.md
resolves: 18, 19
---

# 56. The animals move

The `.TXT` animated models have been parsing since worklog 18 and drawing
since 19, but only as a heap of parts at their authored coordinates. The open
question - written down in `docs/formats/anim.md` as "the part transform is
not established" - was how a part is placed relative to its parent. Worklog 19
tried summing each part's `pivot` up the parent chain and got a `TREX` with
its head between its shoulders.

The answer is that there is no chain.

Finding it took following the data rather than the code. Class 14's aim
(worklog 55) reads a part's `+0x54` and puts it through a matrix at the
model's `+0x634`, so something writes both. `+0x634` turned out to be a
rotation matrix built from the model's own `angle` (`0x4681d0`), and `+0x54`
the output of `0x4684c0` - which turns the actor's clock into a frame index
and a fraction and interpolates every part's angle and centre between that
keyframe and the next. Then `0x467980` draws a part: it hands those six
values, and nothing else, to `0x42aa30`, the routine that pushes a transform,
and walks the vertices.

So a part's `centerList` entry is where it goes in model space. `pivot` and
`parent` are loaded and never read again - authoring data that stayed in the
file. Under everything sits the model's own angle, which in all eighteen
models is about -87 degrees about x: a z-up model being turned y-up.

## Checking it

A model is checked by looking at it, and then by writing down what you saw.
`hb view trex.txt out.png` now draws a Tyrannosaurus: tail, body, head at the
front with a jaw under it, little arms, two legs under the body.
`hb view trex.txt out.png 0.6` draws the same animal mid-stride with one leg
forward and one back. `PTERYL` is a pterosaur with three wing segments a
side, `DRAG66` a dragon with a segmented tail, `FX-4` a four-legged mech.

The test says it in numbers: `TREX`'s head is forward of its body and its
tail behind, its jaw under its head, its feet under its legs, and its arms
and legs mirror in x within 200 units; `PTERYL`'s wing segments mirror and
each is further out than the last. A second test poses all eighteen models at
every keyframe and checks nothing flies off.

One thing the pose does that looks wrong and is not: the parts push well past
the normalisation their vertices were scaled to. `0x4664e0` scales the
vertices and the keyframe centres together at load, once, and nothing
renormalises afterwards - so `FX-4` really is five model-widths across when
posed.

`hb-render` re-poses every animated mesh each frame off the level's clock, so
the dinosaurs in Chimera walk, the pterosaurs flap, and the spider towers
move. `hb view` grew an optional time so a single frame can be looked at.

**Still unknown:** what `magPower` and `legFlag` do, and what `pivot` and `parent` were for in the tool that wrote them; what the 24 bytes beyond an in-memory vertex's coordinates are; and where an actor's animation clock comes from.
