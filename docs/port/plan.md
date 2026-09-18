---
title: The Rust port
status: partial
covers: crates/
worklog: 7, 8, 9, 10, 11, 12, 13, 14, 15, 16
---

# The Rust port

The goal is Hellbender playable on a modern machine from the original disc's
data, with no reimplementation of DirectDraw, DirectPlay or the 1996 sound
card matrix.

This page is the only place in `docs/` that describes intent rather than
observed fact. Everything else describes what the data and the binary do.

## Principles

- **The disc is read-only input.** Nothing is converted ahead of time and
  checked in. The port reads `GAME.POD` and `STARTUP.POD` directly, the way the
  engine does.
- **Bug-for-bug where it is visible.** The colour tables are authored, not
  computed, so the port uses them rather than recomputing shading in true
  colour. Where the original is wrong in a way a player can see, the port is
  wrong the same way.
- **No unverified constants in code.** If a field's meaning is a guess it keeps
  a numbered name. `Shape::guess` refuses `INFOBAR.RAW` rather than encode an
  inference.
- **Every documented claim gets a test.** `crates/hb-formats/tests/` checks the
  documentation against the shipped archives, and skips when the archives are
  not present.
- **No dependencies until one is unavoidable.** Every crate that reads or draws
  the game's data has none, so that half of the workspace builds offline and
  will keep building. `hb-fly` is the exception and the only one: a window and a
  keyboard are not worth writing by hand.

## Crates

```
hb-pod       the POD container                      done
hb-formats   act raw lvl terrain mrgl colour text   data layer done
hb-world     cell geometry and height queries       started
hb-render    the software rasteriser, plus the       terrain done
             level loader both binaries share
hb           the `hb` inspection tool, no deps       growing
hb-fly       a window and a keyboard, via minifb     flies
```

Planned, neither started:

```
hb-sim       flight model, weapons, enemy logic phases
hb-audio     the mixer, .MOD playback, .WAV effects
```

## Stages

**1. Data layer.** Done. Everything in both archives parses, or is a documented
anomaly. `hb check` is the gate.

**2. Look at it.** Mostly done. `hb png` renders a `.RAW` through its palette,
`hb view` renders a model flat shaded, and `hb heightmap` renders a level's
ground lit by the engine's own per-triangle normal. Between them they proved
the polygon node and the terrain right - the ship looks like a ship and FLOAT
looks like floating platforms. Still to do: the colour ramps as strips, models
to OBJ.

**3. The world, headless.** Mostly done. `hb-world` has the cell geometry -
wrapping indices, the 8.0-unit cell, the parity-dependent triangle split, the
point-in-triangle test and the surface normal - plus `heightAtGrid` and the box
span query. Still to do: the plane evaluation as a height lookup at an
arbitrary point, the box and chamber intersection tests, and collision.

**4. A picture.** Done for the terrain, and it moves. `hb ground` draws a level
from above, `hb fly` draws one frame from inside it, and `hb-fly` opens a window
and flies through it at 60 frames a second in any of the game's three screen
sizes, with the levels' placed objects in it. Still to do: chambers, sprites,
the sky, the cockpit, and the `.TXT` animated model format.

**5. Models.** Mostly done. The MRGL meshes draw, at the position, heading and
scale each level's instance list gives them. Still to do: bind a mesh's
materials to its polygons properly rather than by running index, and parse the
`.TXT` animated model format, which about 5% of placements need.

**6. Flight.** `hb-sim`: the ship, the controls from `HELLBEND.INI`'s bindings,
and collision against `hb-world`. Playable in the sense of flying around an
empty level.

**7. The game.** Enemies from `.DEF`, their seven logic phases, courses from
`.CRS`, weapons, powerups, the HUD. Mission success and failure.

**8. The trimmings.** Music through a `.MOD` player, effects, Smacker
cutscenes through a decoder binding, the front end, demos, saves.

Multiplayer is out of scope until everything above works.

## What is still missing

Two of the four blockers listed here earlier are closed. The cell triangulation
is read out of `groundTriangleMidpoint` and reproduced in `hb-world`, and the
box texture slots are pinned to axes by measurement - slots 0 and 1 face along
z, 2 and 3 along x, 4 is the top and 5 the bottom.

What remains:

- Which member of each box axis pair faces which way. The exposure test that
  settled the axes gives no signal on the sign, 81 against 77 and 82 against
  63. Settling it needs perspective rendering, or a rendered level compared
  with a screenshot.
- Which of the four `.CLR` orientation bits is which mirror and which rotation.
- Bit 8 of the ground shading word, and what the chamber's 24-bit shading value
  decomposes into.
- What separates polygon node 0x18 from 0x0e.

None of these block a first terrain render; all of them would make it wrong in
some detail.

## Running it

```
cargo test                      needs the disc; skips without it
cargo run -p hb -- check
cargo run -p hb -- level float
HB_GAME=/path/to/disc cargo run -p hb -- terrain hoth

cargo run --release -p hb-fly -- hoth --mode 480 --scale 1
```

`HB_GAME` points at the directory holding `system/GAME.POD`. It defaults to
`original/`, which in this repository is a symlink to the mounted disc.
