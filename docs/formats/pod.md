---
title: The .POD archive
status: solid
covers: system/GAME.POD, system/STARTUP.POD
worklog: 1, 2
---

# The .POD archive

Terminal Reality's container. Hellbender ships two, both POD1: a count, a
comment, a flat directory, then every file body back to back. Nothing is
compressed, encrypted or aligned.

## Layout

All little-endian. Offsets are absolute from the start of the file.

```
header
  0x00  u32       entry_count
  0x04  char[80]  comment                NUL-padded

directory   at 0x54, entry_count * 40 bytes
  +0x00  char[32]  name                  see below
  +0x20  u32       size
  +0x24  u32       offset

bodies      from 0x54 + entry_count * 40 to end of file
```

There is no trailer. The lowest entry offset equals the end of the directory
and the sum of every entry's size equals the file length minus that offset, for
both archives, so every byte is accounted for.

## Fields

`name` - a backslash-separated path with exactly one directory component, for
example `ART\AFTR0000.RAW`. It is NUL-terminated, and the remaining bytes of
the 32 are not always zero: `.RAW` entries carry a second NUL-terminated string
naming the palette the texture was authored against. The engine never reads
past the first NUL. See [.ACT](act.md) for what that second name points at.

`size` - exact, no padding to any boundary. Zero is legal: 21 `.RAW` and two
`.ACT` entries are zero-length placeholders and a reader must tolerate them.

`offset` - absolute file offset of the body.

## How the engine addresses entries

The open helper at `HELLBEND.EXE:0x00473a80` takes a directory, a filename and
a mode - `open("data", "float.ra0", "rb")` - rather than a path. The directory
is a namespace, and the same basename exists under more than one of them. The
ten directories in use:

```
ART      textures, cockpit art, full-screen images
DATA     per-level terrain grids and placement lists
MODELS   .BIN and .TXT models
LEVELS   .LVL manifests
FOG      .FOG, .LTE, .MAP, .MIX colour tables
MUSIC    .MOD ProTracker modules
SOUND    .WAV effects
UI       .BMP front-end art
DEMO     .DMO recorded demos
STARTUP  the font and two registration blobs
```

## Worked example

`STARTUP.POD` begins:

```
00000000  b6 04 00 00                          entry_count = 1206
00000004  53 74 61 72 74 75 70 20 ...          "Startup Hellbender 1.0"
00000054  41 52 54 5c 41 46 54 52 30 30 30 30  "ART\AFTR0000"
00000060  2e 41 43 54 00 ...                   ".ACT", then NULs
00000074  00 03 00 00                          size   = 768
00000078  c4 bc 00 00                          offset = 0xbcc4
0000007c  41 52 54 5c 41 46 54 52 30 30 30 30  the next entry's name
00000088  2e 52 41 57 00 56 47 41 2e 41 43 54  ".RAW\0VGA.ACT"
```

`0xbcc4` is 48,324, which is `0x54 + 1206 * 40` - the first body starts the
byte after the directory.

The two comments are build labels: `Startup Hellbender 1.0` and `Game Pod -
Hellbender Full Version - RC 1.0 JRS`.

## Contents

```
             entries   bytes        largest groups
STARTUP.POD    1,206   14,778,978   RAW 574, WAV 315, ACT 187, BIN 107
GAME.POD       4,341   42,714,189   RAW 3,343, BIN 238, ACT 101, LTE 51
```

## Unknown

Nothing in the container itself. Whether POD1 permits deeper paths or per-entry
compression cannot be answered from these two files, and Hellbender needs
neither.
