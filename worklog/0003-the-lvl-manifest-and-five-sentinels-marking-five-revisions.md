---
number: 3
title: The .LVL manifest, and five sentinels marking five revisions
date: 2026-09-17
area: format, world
files: docs/formats/lvl.md
---

# 3. The .LVL manifest, and five sentinels marking five revisions

A level is not a file. `LEVELS\FLOAT.LVL` is 507 bytes of CRLF text that names
the sixteen files the level is actually made of and sets a handful of numbers.
All 26 levels have exactly 43 lines, so the format is positional, not keyed.

```
 1  4                      format version
 2  float.txt              object placement list      (DATA\)
 3  float.raw              ground altitude, 128x128   (DATA\)
 4  float.clr              ground colour, 128x128     (DATA\)
 5  float.act              ground palette             (DATA\)
 6  float.tex              texture name list          (DATA\)
 7  float.qke              "quake" / moving geometry  (DATA\)
 8  float.pup              powerup placement          (DATA\)
 9  float.ani              animated texture list      (DATA\)
10  float.tdf              ?                          (DATA\)
11  sky.raw                sky texture, or a .VOX     (ART\)
12  floatsk.act            sky palette                (ART\)
13  float.def              enemy/object definitions   (DATA\)
14  float.nav              navigation courses         (DATA\)
15  float8.mod             music, a ProTracker module (MUSIC\)
16  float.fog              fog table                  (FOG\)
17  float.lte              light table                (FOG\)
18  -46333,-46333,0        a position, but not the start one - see below
19  40960                  a heading
20  -46333,-46333,0        a second position
21  32768                  a second heading
22  255                    ?
```

Lines 2 to 17 are filenames without a directory, because the engine addresses
POD entries as (directory, name) and the directory is fixed per slot. Line 11
is the one slot that takes two different kinds of file: the ground levels name
a `.RAW` sky texture, the two space levels (`ROID`, `SHIP`) name `space.vox`.

## The sentinels

Five lines are literal English text rather than data:

```
23  ;New story stuff
29  !New ground additions
34  @Redbook Audio Track
36  =New Cinematic Info
39  { Weather (none=0,snow=1,2=rain,4=lightning,6=rain+lightning)
```

Each one heads a block that was appended to the format after the fact, and each
carries a different punctuation mark, so the parser can check it is where it
thinks it is. They are a version history written into every file: story hooks
first, then ground extras, then CD audio, then cinematics, then weather. The
`{` line even keeps the enumeration of its own field as a comment - the only
documentation the format has.

The blocks are:

```
24-28  five Smacker names or `null`     in-level story triggers
30     255                              ?
31     float.crs                        course data       (DATA\)
32     float.glt                        ?                 (DATA\)
33     0                                ?
35     3                                Redbook CD audio track number
37     eyrbrf.smk                       mission briefing movie
38     eyrdie.smk                       mission failed movie
40     0                                weather bitmask
41     655360,655360                    weather parameters
42     983040,1966080                   weather parameters
```

The weather field reads correctly against its own comment. Nine levels set it:
`HOTH`, `HOTH2` and `HOTH3` set 1, which the comment calls snow and which is
what the Hoth levels are; `IOWAH`, `IOWAH2`, `IOWAH3`, `MORBOS`, `MORBOS2` and
`MORBOS3` set 6, rain plus lightning. That is a field whose meaning is
confirmed rather than inferred.

Line 42 is `983040,1966080` in all 26 files, so it is a constant the format
never varies. Line 41 is `0,0` in 15 levels and one of `327680,327680`,
`655360,655360` or `1310720,1310720` in the other 11 - which is two more than
have weather, `FLOAT` and `FLOAT2` setting it with weather off. Read as 16.16
fixed point these are 5.0, 10.0 and 20.0, against line 42's 15.0 and 30.0.

## Lines 18 and 20 are not the start position

The obvious reading of lines 18 to 21 is a start position and heading and a
respawn position and heading. The data says otherwise: line 18 is
`-46333,-46333,0` in all 26 levels, and line 20 is the same in 24 of them and
`0,0,0` in the other two. No two levels start the player in the same place, so
whatever these are, they are not that. The headings do vary - line 19 takes
five distinct values and line 21 five - so the pair is probably an extent or a
bound with an orientation, and the real spawn lives in the `.DEF` or `.TXT`
placement file. Left as a question for whoever reads the level loader.

Line 19's 40960 is 0.625 of a turn if angles are a 16-bit circle, which matches
the model format's angle convention.

## The 26 levels

```
FLOAT FLOAT2            HOTH HOTH2 HOTH3        IOWAH IOWAH2 IOWAH3
JURASIC JURASIC2 JURASIC3   KREASH KREASH2 KREASH3   MORBOS MORBOS2 MORBOS3
ROID ROID2 ROID3 ROID4  SHIP SHIP2  NETLVL1 NETLVL2 NETLVL3
```

Twenty-three single-player levels in eight themed sets plus three multiplayer
levels. `NETLVL1-3` share `netlvl.txt`, `netlvl.tdf`, `netlvl.crs` and
`netlvl.glt` between them and differ only in terrain and palette.

**Still unknown:** lines 22, 30 and 33, all three of which are 0, 130 or 255 and
smell like flags or indices rather than counts. Line 10's `.tdf` is three bytes
in most levels (`0\r\n`) so its purpose cannot be read from the data alone.
