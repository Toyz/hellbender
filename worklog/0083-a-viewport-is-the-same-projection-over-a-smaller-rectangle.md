---
number: 83
title: A viewport is the same projection over a smaller rectangle
date: 2026-09-21
area: decomp, render
files: crates/hb-render/src/camera.rs
---

# 83. A viewport is the same projection over a smaller rectangle

Four entries now end on the same wall - [[70]], [[71]], [[72]] and [[80]] -
because the reticle, the objective arrow, the weapon picture and the
briefing's globe are all drawn the same way, through `0x42c9d0`, and nobody
had read it.

I had it down as `setViewport(x, y, w, h)`, which was a guess from the call
sites. Half right.

The first fifty instructions do not touch the arguments at all: they copy the
camera's angles, its position, and half a dozen more globals into parallel
stacks at `0x53a4a8`, `0x53a4d0`, `0x53a4f8`, `0x53a520` and up, indexed by a
depth at `0x5029d4` which it then increments. It is a **push**. That is why
these little scenes can be drawn in the middle of a frame without disturbing
the one behind them.

Then it takes its four arguments and does this:

```
0x5b3658 = (x + w/2) << 16      the centre
0x5b3650 = (w/2)     << 16      the scale
0x512550 = 0x10000              and the zoom back to 1
```

and the same pair for y and h. Centre in the middle of the rectangle, scale
half of it.

Which is exactly what `Camera::screen` has been doing for the whole screen
since [[16]] - `sx = half(width) - 1`, `cx = sx + 1`. A viewport is not a
different projection. It is the same projection over a different rectangle,
and that is why one function serves both.

**Still unknown:** the depth, which is the half I went in for. The arrow's
model lies in a plane through the origin and the camera is put at the origin
with it (`0x42c9a0(0, 0, 0)`), so a projection would divide by zero. Nothing
between `0x42c980` and `0x459a10` obviously parts them: `0x48a640(0x8000)`
clamps a scalar into `0x51138c` and `0x48a670(0, 0, 0)` sets a second
position that the state push also saves. One of those is carrying the eye
back, or the model transform at `0x42aa30` does, and I have not got it yet.
The port still turns those four corners in two dimensions.
