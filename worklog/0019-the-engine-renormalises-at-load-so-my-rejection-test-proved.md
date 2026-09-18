---
number: 19
title: The engine renormalises at load, so my rejection test proved nothing
date: 2026-09-17
area: format, decomp, port
files: docs/formats/anim.md
---

# 19. The engine renormalises at load, so my rejection test proved nothing

[18](0018-the-animated-model-format-and-a-t-rex-whose-head-will-not-si.md)
ruled out four candidate part transforms by measuring how far each pushed a
model past its normalisation bound of 32,767, and concluded that `pivot` cannot
be a translation because adding it gives an extent of 274,257.

The test was invalid. `0x4664e0` walks every part of a loaded model, finds the
largest absolute vertex component across all of them, and rescales the whole
model so that maximum becomes `0x7fff`:

```asm
mov  ecx, 0x80000000              ; running maximum
lea  ebx, [eax+0x660]             ; first part
mov  ebp, [ebx+0x84]              ; its vertex count
mov  esi, [ebx+0x8c]              ; its vertices, 0x24 bytes each
  ... |x|, |y|, |z| against ecx ...
add  ebx, 0xdc                    ; next part
  ... then a second pass scaling against 0x7fff
```

The engine renormalises after the transforms are applied. So a transform that
takes the model past 32,767 is not thereby wrong - it is simply scaled back
afterwards, which is exactly what the function exists to do. Four
measurements, none of which meant what I said they meant.

The conclusion happened to survive - summing `pivot` up the parent chain and
then renormalising does place `TREX`'s body, two mid sections and tail in order
along x, at -6235, -4924, 2382 and 15645, and still leaves the head at -213,
between the mid sections instead of beyond the nose. But it survived by luck,
and the reasoning that got there was wrong. The real reason the translation is
not enough is that the per-frame angles have to be applied about each part's
pivot before the chain composes, and that is the open question.

The lesson is narrow and worth keeping: a bound that the authored data respects
is not necessarily a bound the runtime respects. I read "every model measures
exactly 32,767" as a constraint on the format when it is a property of the
exporter, and the loader's first act is to recompute it.

## What the detour did establish

The in-memory model accounts for its size exactly, which is a better kind of
evidence than an extent:

```
0x0000  header, 1,632 bytes
0x065c  partCount
0x0660  parts, 0xdc = 220 bytes each
```

`1632 + 64 * 220 = 15712`, and 15,712 is the size the
[MRGL](0005-mrgl-the-node-stream-inside-a-model-and-its-size-table-as-a.md)
jump table gives a type 0x26 node. So a model may have at most **64 parts**,
the largest shipped has 25, and the type 0x26 size is no longer a bare number
in a table.

The part struct came out of the text writer at `0x46c3cd`: name at +0, pivot at
+0x10, the angle list pointer at +0x1c and the centre list at +0x20, parent,
hit points and leg flag at +0x24, min and max at +0x30 and +0x3c, and the
vertex count and pointer at +0x84 and +0x8c. An in-memory vertex is 36 bytes
where the file gives 12, so 24 bytes of it are working space.

`0x466680` interpolates between keyframes against `0x7fff`, so the animation is
tweened, not stepped.

**Still unknown:** how a part is placed relative to its parent, which now needs
the rotation rather than a better translation. What the 24 bytes beyond an
in-memory vertex's coordinates are.
