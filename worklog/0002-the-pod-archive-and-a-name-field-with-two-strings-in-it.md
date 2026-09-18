---
number: 2
title: The .POD archive, and a name field with two strings in it
date: 2026-09-17
area: format, tooling
files: tools/pod.py, docs/formats/pod.md
---

# 2. The .POD archive, and a name field with two strings in it

POD is Terminal Reality's container, and Hellbender uses the first version of
it: a count, a comment, a flat directory, then the bodies. Nothing is
compressed and nothing is aligned.

```
u32       entry_count
char[80]  comment            NUL-padded, at 0x04
entry_count * 40 at 0x54:
  char[32]  name             backslash path, NUL-padded
  u32       size
  u32       offset           absolute, from the start of the file
```

Little-endian throughout. The proof that there is nothing else in the format is
arithmetic: for both archives the lowest entry offset is exactly the end of the
directory, and the sum of every entry's size is exactly the file length minus
that offset. No gaps, no padding, no trailer.

```
                 file size   count   dir ends   sum of sizes   slack
STARTUP.POD     14,778,978   1,206   0x00bcc4     14,730,654       0
GAME.POD        42,714,189   4,341   0x02a69c     42,540,465       0
```

The comments are build labels and worth keeping: `Startup Hellbender 1.0` and
`Game Pod - Hellbender Full Version - RC 1.0 JRS`. The game shipped a release
candidate.

## The 32-byte name holds two names

The name field is not one string. It is NUL-terminated at the path, and for the
art entries a second NUL-terminated string follows in the same 32 bytes:

```
ART\10PLATE.RAW\0ROID.ACT\0\0\0\0\0\0\0\0
ART\5PLATE.RAW\0SHIP.ACT\0\0\0\0\0\0\0\0\0
ART\A-3.RAW\0FLOAT.ACT\0...
ART\AHNK.RAW\0KREASH.ACT\0...
```

The second string is a palette. It varies per entry and it is not monotonic, so
it is not a buffer the packer failed to clear between iterations - it is data
that the build tool wrote and the runtime ignores.

Only `.RAW` entries carry it, and no other extension does in either archive. In
STARTUP.POD all 574 `.RAW` entries name `VGA.ACT` and nothing else. In GAME.POD
3,314 of the 3,343 `.RAW` entries name a level palette - `KREASH.ACT` 470,
`MORBOS.ACT` 463, `FLOAT.ACT` 444, `SHIP.ACT` 441, `JURASIC.ACT` 388 and so on.
The 29 without one are the per-level terrain grids under `DATA\`, which are
index planes rather than pictures and have no palette to be authored against.

Nothing in `HELLBEND.EXE` reads past the first NUL - the loader at `0x00473a80`
takes a directory and a filename and compares the path only. So the second name
is a provenance record, useful for a viewer that wants to show a texture in the
palette it was drawn in, and useless to the engine.

## Paths are directories

Entry paths use a backslash and one level of directory: `ART\`, `DATA\`,
`MODELS\`, `LEVELS\`, `FOG\`, `MUSIC\`, `SOUND\`, `UI\`, `DEMO\`, `STARTUP\`.
The engine addresses files as (directory, name) rather than as a path - the
open helper at `0x00473a80` is called as `open("data", "float.ra0", "rb")` - so
the directory is effectively a namespace, and the same basename can and does
exist under two of them.

**Still unknown:** whether a POD directory may hold more than one level of path
(nothing here does), and whether POD1 has a variant with per-entry compression
that a later Terminal Reality title used. Neither matters for Hellbender.
