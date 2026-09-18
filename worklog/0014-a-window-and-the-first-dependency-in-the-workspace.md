---
number: 14
title: A window, and the first dependency in the workspace
date: 2026-09-17
area: port, render, build
files: crates/hb-fly, crates/hb-render/src/level.rs, docs/port/plan.md
---

# 14. A window, and the first dependency in the workspace

`hb-fly` opens a window and flies through a level. Arrows steer, `w` and `s`
work the throttle, tab cycles the 26 levels, escape quits. It holds 60 frames a
second in all three of the game's screen sizes - 320x200, 320x400 and 640x480 -
which is the frame rate cap rather than a limit of the renderer.

The number worth writing down is the original's own floor: `autoMinFrameRate=8`
in `HELLBEND.INI`. The engine's authors thought eight frames a second was the
point at which to start dropping detail. That is the bar this is clearing by a
factor of seven, on a software rasteriser, in a debug-friendly language, thirty
years later.

## One dependency, kept in one crate

`minifb` is the first external crate in the workspace, and it is confined to
`hb-fly`. Everything that reads or draws the game's data - `hb-pod`,
`hb-formats`, `hb-world`, `hb-render` and the `hb` tool - still has none, so
that half of the workspace builds with no network and no registry and will keep
doing so.

That split is worth keeping deliberately rather than by accident. The data layer
is the part that has to outlive anything; a window is a commodity.

## The level loader moved

`hb` and `hb-fly` both need the same fifteen or so files out of the archives -
the manifest, the thirteen terrain grids, the texture list resolved to images,
the palette, the two ramps - so that assembly moved into
`hb-render::level::Level` and both binaries call it. `hb`'s copy had already
drifted: its `cmd_ground` and `cmd_fly` each opened the archives separately and
resolved textures slightly differently.

Splicing the old loader out deleted `cmd_fly` along with it, because the two
functions were adjacent and I cut from one marker to another without checking
what lay between. The compiler caught it immediately - `cannot find function
cmd_fly in this scope` - which is the argument for doing this kind of surgery
in a language that will not let a missing function through.

## What it shows

Flying `HOTH` from the middle of the map: snowfields, the ice canyon, the metal
compound rising as you approach it, the ring road. `JURASIC` at 640x480: black
rock, lava, and the plating around the installations. The terrain shading
changes as the camera turns, because it is per cell and baked, exactly as the
`.LTE` database holds it.

The flight model is not the game's. It is a free-flying eye with a throttle -
enough to move through the world and see whether the world is right. The real
one is `hb-sim`, when `HELLBEND.EXE`'s has been read.

**Still unknown:** everything above the terrain. No chambers, no models, no
sprites, no sky, no cockpit, no enemies, no collision. Fog is wired up and has
not been checked against anything.
