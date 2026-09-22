---
title: The .LVL manifest
status: solid
covers: LEVELS\*.LVL
worklog: 3, 29, 30, 48, 79
---

# The .LVL manifest

CRLF text, positional, exactly 43 lines in all 26 shipped levels. It names the
files a level is made of and sets the numbers that are not worth a file of
their own. Lines 2 to 17 are bare filenames; the directory for each slot is
fixed, because the engine addresses [POD](pod.md) entries as (directory, name).

## The campaign

A `.LVL` does not say what follows it, and `GAME.POD` holds them
alphabetically. The order is the engine's: `0x482720` builds it when a new
game starts, copying twenty three file names into forty-byte slots at
`0x6106c0` and filling two parallel arrays - the chapter at `0x60ff60` and
the mission within that chapter at `0x60fc30`.

| # | Chapter | Levels |
| --- | --- | --- |
| 1 | Morbos | `morbos`, `morbos2`, `morbos3` |
| 2 | Float | `float`, `float2` |
| 3 | Iowah | `iowah`, `iowah2`, `iowah3` |
| 4 | Kreash | `kreash`, `kreash2`, `kreash3` |
| 5 | Jurasic | `jurasic`, `jurasic2`, `jurasic3` |
| 6 | Roid | `roid`, `roid2`, `roid3`, `roid4` |
| 7 | Hoth | `hoth`, `hoth2`, `hoth3` |
| 8 | Ship | `ship`, `ship2` |

So the game starts on `morbos`, not on the archive's first entry. The three
`NETLVL` levels are not in the table; `netlvl.ini` lists those for network
play.

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
  10  float.tdf                 tunnel definitions          DATA\
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
  22  255                       fog colour, a palette index
  23  ;New story stuff          sentinel
  24  null                      the arrival movie           Story\
  25  null                      the departure movie
  26  eyrie1.smk                the chapter's story movie
  27  null                      story movie 4
  28  null                      story movie 5
  29  !New ground additions     sentinel
  30  255                       sky layer altitude
  31  float.crs                 course data                 DATA\
  32  float.glt                 ground light table          DATA\
  33  0                         read and never used
  34  @Redbook Audio Track      sentinel
  35  3                         CD audio track number
  36  =New Cinematic Info       sentinel
  37  eyrbrf.smk                mission briefing movie      Story\
  38  eyrdie.smk                mission failed movie        Story\
  39  { Weather (...)           sentinel, with its own comment
  40  0                         weather bitmask
  41  655360,655360             sky drift, u and v
  42  983040,1966080            lightning interval
  43  (empty)                   trailing newline
```


### The five story slots

Only eleven of the 26 levels name a movie at all, and what they name says what
each slot is for:

| slot | levels | what they are called |
| --- | --- | --- |
| 1 | `HOTH`, `MORBOS`, `ROID`, `SHIP` | `snowin`, `morbin`, `astrin`, `shiv1in` |
| 2 | `HOTH3`, `ROID4`, `SHIP`, `SHIP2` | `snowout`, `astrout`, `shiv1out`, `shiv2out` |
| 3 | `FLOAT`, `IOWAH`, `JURASIC`, `KREASH`, `MORBOS`, `ROID` | `eyrie1`, `iowah1`, `chimera1`, `kresh1`, `morbos1`, `astroid1` |

Every level with a slot 1 is the first of its chapter and every one with a
slot 2 is the last, so slot 1 is arriving and slot 2 is leaving. Slot 3 is the
chapter's own film, one a planet. Slots 4 and 5 are `null` in all 26.

The engine plays them from five short routines around `0x45b9f0`, each one
comparing a name against `"null"` before handing it to the player at
`0x49f920`, and all of them gated on `0x512650`. Which routine runs when has
not been read; the port plays slot 1 as a level opens and slot 2 as it is
left.
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
(`0x44bf52`), and the pair is handed to the lightning spawner as the level
loads (`0x44bfae`, `0x49d220`). There it sets the countdown to the next
strike: the first value plus a random amount up to the second
(`0x49d26e`), so a level with lightning flashes every 15 to 45 seconds. The
spawner does nothing unless bit 4 of line 40 is set, and it keeps five
strikes at most (`0x5bc7c0`).

Line 22 is the fog colour, as an index into the level's palette. When the
engine cannot open the `.FOG` named on line 16 it builds the table itself
(`0x486199`): `0x485ed0` takes the index, reads the three bytes at
`0x5b3350 + 3*index` as the colour to fade to, and the last two of the ramp's
rows are filled with the index itself, so everything past the draw distance
comes out that one colour. The table is then written back to `FOG\` for next
time. It is 255 in 22 levels, 190 in `KREASH`, 130 in `JURASIC` and 0 in `HOTH`
and `SHIP`.

Line 30 is the altitude of the sky layer. It is passed as the third argument to
the sky setup (`0x44c512`, `0x44f770`) which stores it shifted up fifteen at
`0x5055d4` - the same scale a [terrain](terrain.md) height byte is read at, so
255 is 127.5 units, just under the sky plane at 128. Three things use it:

- Drawing. `0x42f4f0` compares the camera's altitude and the object's against
  it and draws nothing that is on the other side of the layer from the eye.
- Powerup drops. A dropped [powerup](../engine/simulation.md) starts at the
  height of whatever dropped it, clamped to twice the stored value, which is
  the line itself in units (`0x426fd4`).
- Dying. The wreck animation is only taken when the ship is below half of it
  (`0x465541`), and then only half the time.

It is 255 in 24 levels, 190 in `KREASH` and 130 in `JURASIC`.

Line 33 is read into `0x667074` and nothing reads that address. It is 255 in
the three `JURASIC` levels and 0 in the other 23.

Line 10 names a tunnel definition file, and the shipped game does not read it.
`JURASIC.TDF` and `JURASIC3.TDF` are the only two with anything in them, and
they are the same 129 bytes: a count of 1, the level `artic-t1.lvl`, two world
positions - (440.0, 22.7, 216.5) and (311.9, 31.3, 40.6) - and the textures
for the hole at each end, `icehole.raw` over `DBROWN.RAW`. No such level
ships. The loader the level sequence calls for this slot (`0x4624b0`) is ten
bytes that set the tunnel count at `0x648b48` to zero and return without
looking at the name; the matching writer (`0x4624c0`, "Unable to open tunnel
list") is dead code. The count's only other reader is the end-of-level tally,
which skips its `Tunnels found: %d%%` line when it is zero. Eight of the other
eleven files hold `0`, three are empty, and thirteen levels have no `.TDF` at
all.

Line 32 names the ground light table - see [.GLT](scenery.md). The loader
(`0x48c4c0`) takes the name, cuts it at the dot and appends `.glt`, and if
line 32 is empty it uses line 9's name instead (`0x44c60a`).

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

Whether line 1's version 4 has any earlier form the parser still accepts.
