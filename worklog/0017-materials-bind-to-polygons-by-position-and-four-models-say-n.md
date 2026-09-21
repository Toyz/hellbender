---
number: 17
title: Materials bind to polygons by position, and four models say nothing at all
date: 2026-09-17
area: format, render, port
files: crates/hb-formats/src/mrgl.rs, crates/hb-render/src/scene.rs, docs/formats/mrgl.md
slug: materials-bind-to-polygons-by-position-and-four-models-say-n
---

# 17. Materials bind to polygons by position, and four models say nothing at all

[16](0016-the-instance-list-was-in-the-same-file-all-along.md) shipped object
rendering with a material-to-polygon binding that was a running index, and said
so in `docs/engine/rendering.md` rather than pretending otherwise. This closes
it. The node stream already answers the question: a material applies to every
polygon after it, until the next material.

Measured over the game archive: every polygon is preceded either by another
polygon or by a material, 24,077 and 3,013 respectively. So "the most recent
material" is always defined once the first has appeared. Runs are four polygons
long at the median and 168 at the longest.

The parser now records the binding on each polygon as it walks, which is a
three-line change that the old separate-lists shape had made impossible.

## 1,030 polygons have no material at all

Asserting the binding was total is how I found out it is not. 1,030 polygons
across seven models have no material before them:

```
GLOBE.BIN 576    IRIS1.BIN 168    IRIS4.BIN 168    SHELL.BIN 32
JAW1.BIN   30    FANBODY.BIN 28   JAW2.BIN   28
```

These are untextured models, and they carry a node type 0x17 instead - the last
of the common types that had no name.

## 0x17 is a flat colour

12 bytes: a zero `i32` at +4 and a value at +8. 267 of the 270 in both archives
are between 9 and 239, which is **exactly** the shadeable palette range from
[7](0007-the-rust-port-s-data-layer-and-four-claims-the-tests-knocked.md) - not
one falls in the reserved 240 to 255. That range agreeing on its own, from a
completely different part of the format, is the evidence that it is a palette
index.

The three that do not fit are 270, 360 and 482, whose low bytes are 14, 104 and
226 and whose bit 8 is set. An index with a flag above it, the same shape as a
terrain texture word - which is at least a familiar pattern, if not an
explanation.

It binds to polygons exactly as a material does, and `GLOBE.BIN` has one node
for all 576 of its polygons.

## Four models say nothing

The second thing asserting totality found: of those seven untextured models,
four have no colour node before their polygons either. `FANBODY`, `JAW1`,
`JAW2` and `SHELL` begin

```
0x14  0x02  polygon  polygon  ...
```

with nothing between - 118 polygons in all, with neither a material nor a
colour. The polygon header is fully accounted for (type, corner count, three
normal components, plane constant, in exactly 24 bytes), so there is nowhere
else for it to be hiding.

Either the engine carries a current colour across draws, or these four are
special-cased by whatever draws them - they are parts of a creature, and jaws
and a shell and a fan body sound like limbs of something assembled elsewhere.
The port skips them rather than invent a colour, and the test pins the exact
list so the decision is visible rather than silent.

## The globe

`GLOBE.BIN` is the planet in the mission briefing, and it renders as a sphere:
482 vertices, 576 polygons, 288 drawn and 288 back-facing. Exactly half culled
is what a closed convex surface should give, and it is a better check on the
winding rule than any count.

**Still unknown:** what colour those 118 polygons are. What distinguishes
polygon node 0x18 from 0x0e. Ten record types still have no semantics: 0x04,
0x05, 0x06, 0x0a, 0x0c, 0x0f, 0x12, 0x19, 0x1d and 0x1f.
