---
number: 7
title: The Rust port's data layer, and four claims the tests knocked down
date: 2026-09-17
area: port, format, test
files: crates/hb-pod, crates/hb-formats, crates/hb, crates/hb-formats/tests/against_the_game.rs
slug: the-rust-port-s-data-layer-and-four-claims-the-tests-knocked
---

# 7. The Rust port's data layer, and four claims the tests knocked down

The first stage of the port is a workspace of three crates with no external
dependencies at all, so it builds on a machine with no network and no registry
cache:

```
crates/hb-pod       the POD container
crates/hb-formats   act, raw, lvl, terrain, mrgl, colour, text, png
crates/hb           the `hb` command line tool
```

`hb-formats` has a module per page under `docs/formats/`, and where a field is
still unnamed the struct carries it as `unknown_22` rather than guessing. The
`png` module is a 60-line PNG writer emitting stored deflate blocks, which is
the cheapest way to look at a `.RAW` without pulling in a crate.

`hb check` parses everything in both archives:

```
startup: 104 models, 187 palettes, 574 images, 0 levels, 1 ramps, 1 colour maps, 1 blend tables
game:    238 models, 101 palettes, 3317 images, 26 levels, 76 ramps, 11 colour maps, 1 blend tables
```

Two entries fail, both already documented as anomalies: `ART\INFOBAR.RAW` is
8,000 bytes, which no known shape explains, and `FOG\KREASH.MIX` is 7,936
bytes where a blend table is 65,536. `Shape::guess` deliberately refuses
`INFOBAR.RAW` rather than write 320 x 25 into code on an inference.

## The tests are the point

`crates/hb-formats/tests/against_the_game.rs` is fifteen tests that each check
a claim from the documentation against the shipped archives. They skip rather
than fail when `HB_GAME` does not point at a copy of the game, so the workspace
still builds for someone without the disc.

Four of them failed on first run, and all four were the documentation being
wrong rather than the code.

### A group node's names are 16 bytes apart, not 32

[5](0005-mrgl-the-node-stream-inside-a-model-and-its-size-table-as-a.md)
recorded the group node's child names as eight 32-byte slots from +0x18, which
left 36 bytes of the 344 unexplained. The stride is 16: in `ALIENSH.BIN`,
`aliensh1.bin` begins at 0x18 and `aliensh2.bin` at 0x28. With sixteen 16-byte
slots the arithmetic closes exactly:

```
0x18 + 16 * 16 = 0x118     the runtime pointer array, where the free
                           routine at 0x473e70 says it is
0x118 + 16 * 4 = 0x158     = 344, the node's size from the jump table
```

Nothing is left over. A group holds at most sixteen children, not eight, and
`ALIENSH.BIN`'s bytes from 0x98 to 0x158 - its eight unused name slots and all
sixteen pointer slots - are zero, as they should be.

### Indices 240 to 255 are reserved, and three tables say so

The claim that a `.LTE`'s last row collapses everything to index 0 was checked
against `FLOAT.LTE` and failed. The row has seventeen distinct values:

```
row 15[0..239]   0
row 15[240..255] 240 241 242 ... 255
```

The same boundary shows up in `VGA.MIX`, where `mix[0][i] == i` holds for i
below 240 and `mix[0][i] == 0` for i at or above it. So indices 240-255 are
never shaded and never blended. Together with index 0 being the transparent
colour, the renderer's range is 0 to 239 and the top sixteen belong to whatever
must appear in exactly its authored colour - the HUD, the instruments, the
front end.

`.FOG` is the exception and it is a clean one: `FLOAT.FOG` row 15 holds a
single distinct value across all 256 entries, 255. Light spares the reserved
colours; fog does not.

While checking this: across `VGA.LTE`, 3,815 of the 3,840 level-to-level steps
are equal or darker in luminance measured against `VGA.ACT`. It is a real
brightness ramp with 25 exceptions, not a lookup that happens to get darker.

### The colour map is worse than "plausible"

`VGA.MAP` was described as returning a plausible colour for every input. Over a
grid of samples the worst squared distance is 65,025 - a colour that is as
wrong as it is possible to be. The mean is the claim that survives: about 3,500
against an exhaustive nearest-colour search's 1,800. The test asserts the mean.

### And one that was just arithmetic

`only_raw_entries_carry_a_palette_name` passes, and pins the count at 3,314 of
GAME.POD's 3,343 `.RAW` entries. Worth having as a test rather than a sentence,
because it is the kind of number that rots.

## What the tool can already do

```
hb level float        the manifest, with every named file checked against the archive
hb terrain float      all thirteen grids, with altitude spans
hb model cube.bin     the MRGL node walk
hb png game art/ahnk.raw out.png
```

`hb png startup art/ckpt200.raw` renders the cockpit correctly on the first
try, which is what confirmed index 0 as transparent: its viewport region is
index 0 and nothing else, 32,051 of the image's 64,000 pixels.

**Still unknown:** everything above the data layer. There is no renderer, no
simulation and no window yet - see `docs/port/plan.md`.
