---
title: The .LVL manifest
status: partial
covers: LEVELS\*.LVL
worklog: 3, 29, 30
---

# The .LVL manifest

CRLF text, positional, exactly 43 lines in all 26 shipped levels. It names the
files a level is made of and sets the numbers that are not worth a file of
their own. Lines 2 to 17 are bare filenames; the directory for each slot is
fixed, because the engine addresses [POD](pod.md) entries as (directory, name).

## Layout

```
line  value                     slot
   1  4                         format version, 4 in every shipped level
   2  float.txt                 object placement            DATA\
   3  float.raw                 ground altitude 128x128     DATA\
   4  float.clr                 ground colour 128x128       DATA\
   5  float.act                 ground palette              ART\
   6  float.tex                 texture name list           DATA\
   7  float.qke                 moving/shaking geometry     DATA\
   8  float.pup                 powerup placement           DATA\
   9  float.ani                 animated texture list       DATA\
  10  float.tdf                 unidentified                DATA\
  11  sky.raw                   sky texture, or a .VOX      ART\
  12  floatsk.act               sky palette                 ART\
  13  float.def                 enemy and object definitions DATA\
  14  float.nav                 navigation courses          DATA\
  15  float8.mod                music, ProTracker module    MUSIC\
  16  float.fog                 fog table                   FOG\
  17  float.lte                 light table                 FOG\
  18  -46333,-46333,0           light direction             (see notes)
  19  40960                     ambient light
  20  -46333,-46333,0           chamber light direction
  21  32768                     chamber ambient light
  22  255                       unidentified
  23  ;New story stuff          sentinel
  24  null                      in-level story movie 1      Story\
  25  null                      in-level story movie 2
  26  eyrie1.smk                in-level story movie 3
  27  null                      in-level story movie 4
  28  null                      in-level story movie 5
  29  !New ground additions     sentinel
  30  255                       unidentified
  31  float.crs                 course data                 DATA\
  32  float.glt                 unidentified                DATA\
  33  0                         unidentified
  34  @Redbook Audio Track      sentinel
  35  3                         CD audio track number
  36  =New Cinematic Info       sentinel
  37  eyrbrf.smk                mission briefing movie      Story\
  38  eyrdie.smk                mission failed movie        Story\
  39  { Weather (...)           sentinel, with its own comment
  40  0                         weather bitmask
  41  655360,655360             weather parameters
  42  983040,1966080            weather parameters
  43  (empty)                   trailing newline
```

## Fields

`null` in any filename slot means the slot is unused.

Line 11 takes two kinds of file. Ground levels name a `.RAW` sky texture; the
two space levels, `ROID` and `SHIP`, name `space.vox`.

Line 40, the weather bitmask, is the one numeric field whose meaning the file
states itself. The sentinel on line 39 reads
`{ Weather (none=0,snow=1,2=rain,4=lightning,6=rain+lightning)`, and the data
agrees: `HOTH`, `HOTH2`, `HOTH3` set 1, and `IOWAH`, `IOWAH2`, `IOWAH3`,
`MORBOS`, `MORBOS2`, `MORBOS3` set 6. The other 17 levels set 0.

Lines 41 and 42 are pairs of 16.16 fixed-point values. Line 41 is the sky's
drift, texture units a second in u and v: the parser reads it to `0x6670d0`
and `0x6670d4`, and the sky routine adds it times the frame time to its scroll
(`0x44fd8a`). It is `0,0` in 15 levels and 5.0, 10.0 or 20.0 in the other 11 -
see [the sky](sky.md). Line 42 is `983040,1966080` - 15.0 and 30.0 - in all 26
levels, read to `0x6670d8` and `0x6670dc` with those same values as defaults
(`0x44bf52`).

Lines 18 to 21 are light, not placement. The parser reads them at `0x44bb88`
into the level struct at `0x666cb0` - line 18's three values to `+0x284`, which
is `0x666f34`, line 19 to `0x666f40`, line 20 to `0x666f44`, line 21 to
`0x666f50`, and line 22 to `0x666f54`. The terrain shading computation at
`0x41c5d0` lights the ground and box set A from `0x666f34` and `0x666f40`, and
the chambers from `0x666f44` and `0x666f50`.

- Lines 18 and 20 are 16.16 unit vectors: `-46333,-46333,0` is (-0.707,
  -0.707, 0). Line 18 is that in every level; line 20 is too, except `HOTH2`
  and `HOTH3`, which give `0,0,0`.
- Lines 19 and 21 are ambient intensities, 16.16: from 16384 (0.25) to 60000.
  Line 19 is each level's floor in the [ground shading](terrain.md) - 16384 in
  `HOTH`, whose shading bottoms out at 64 - and the value a shadowed ground
  vertex or box corner takes. A lightning flash swaps it for a moment
  (`0x49d399`).

Line 35 is a CD track number, 0 to 9. `NETLVL1` through `NETLVL3` set 0.

## The sentinels

Five lines are literal English rather than data, each with a distinct leading
punctuation mark, and each heads a block appended to the format after the
original 22 lines:

```
 ;  story hooks         lines 24-28
 !  ground additions    lines 30-33
 @  Redbook audio       line  35
 =  cinematics          lines 37-38
 {  weather             lines 40-42
```

They are the format's version history, written into every file. A parser should
check them and refuse a file whose sentinels are out of place.

## Notes

Lines 18 and 20 were first read here as positions, then ruled out as the
player's start because line 18 is the same in every level. They are light
directions - see Fields above and worklog 29.

## The 26 levels

```
FLOAT FLOAT2              HOTH HOTH2 HOTH3        IOWAH IOWAH2 IOWAH3
JURASIC JURASIC2 JURASIC3 KREASH KREASH2 KREASH3  MORBOS MORBOS2 MORBOS3
ROID ROID2 ROID3 ROID4    SHIP SHIP2              NETLVL1 NETLVL2 NETLVL3
```

`NETLVL1-3` are the multiplayer maps and share `netlvl.txt`, `netlvl.tdf`,
`netlvl.crs` and `netlvl.glt`, differing only in terrain and palette.

## Unknown

Lines 22, 30 and 33 - all three take only 0, 130, 190 or 255, so they read as
flags or indices rather than counts. The purpose of the `.tdf` and `.glt` slots.
Whether line 1's version 4 has any earlier form the parser still accepts.
