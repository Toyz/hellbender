---
title: What is in the archives
status: solid
covers: system/STARTUP.POD, system/GAME.POD, system/Story/
worklog: 1, 2
---

# What is in the archives

Counts, not descriptions. Regenerate with `tools/pod.py stats`.

## The disc

```
HELLBEND.EXE                1,685,504   the game
HELLBEND.HLP                9,451,149   WinHelp 3.1 manual, not game data
SMACKW32.DLL                   60,928   RAD's Smacker decoder
system/STARTUP.POD         14,778,978   front end and cockpit
system/GAME.POD            42,714,189   the levels
system/Story/*.SMK        ~280,000,000  31 cutscenes
system/HELLBEND.INI             2,760   settings, written back by the game
system/HELLBEND.INF            23,425   per-sound-card mixer tuning
system/DP*.DLL                        DirectPlay serial/socket transports
SOUND/TAUNT1-6.WAV                    multiplayer taunts, outside the PODs
Setup/directx/                        the DirectX 2 redistributable, not needed
Goodies/                              Internet Explorer 2.0, joystick profiles
Patch v1.02/upgrade.exe     7,045,632   a later build, not yet opened
```

## STARTUP.POD - 1,206 entries, 14,778,978 bytes

```
RAW    574    6,192,290    cockpit art, HUD, front-end images
WAV    315    6,351,470    every sound effect
ACT    187      142,080    palettes
BIN    107      536,123    models, including the player ship
BMP     15      898,118    front-end dialog art
DMO      3      133,965    recorded demos
MOD      1      373,852    front-end music
LTE      1        4,096    VGA light ramp
MAP      1       32,768    VGA 15-bit to index table
MIX      1       65,536    VGA blend table
NDX      1          356    font glyph widths
```

## GAME.POD - 4,341 entries, 42,714,189 bytes

```
RAW   3,343   14,012,416   textures, 3,762 of them 64x64
BIN     238    2,117,612   models
ACT     101       77,568   palettes
LTE      51    3,084,288   light ramps
TXT      27      922,663   mission briefings and animated models
CL0      26    5,111,808   box set A textures
CL1      26    1,703,936   chamber textures
CL2      26    5,111,808   box set B textures
CLR      26      851,968   ground colour
DEF      26    1,024,805   enemy and object definitions
NAV      26      117,169   navigation courses
PUP      26          172   powerup placement
QKE      26      401,433   moving geometry
RA0-RA5  26 each  425,984 each   altitude layers
TEX      26       63,658   texture name lists
TTY      26           78   (empty in every level)
LVL      26       13,118   level manifests
FOG      25      102,400   fog ramps
CRS      24       55,029   courses
WAV      17      469,424   level-specific sounds
MOD      14    4,358,231   music
TDF      13          282
ANI      11        9,454   animated textures
MAP      11      360,448   15-bit to index tables
MIX      11        7,936   blend tables, all but one zero-length
GLT      10        6,857
VOX       2            0   both zero-length
```

## The 26 levels

Eight themed sets plus three multiplayer maps.

```
FLOAT  FLOAT2                   Eyrie, floating platforms
HOTH   HOTH2   HOTH3            snow (weather 1)
IOWAH  IOWAH2  IOWAH3           rain and lightning (weather 6)
JURASIC JURASIC2 JURASIC3       Chimera
KREASH KREASH2  KREASH3
MORBOS MORBOS2  MORBOS3         rain and lightning (weather 6)
ROID   ROID2   ROID3   ROID4    asteroids, sky is space.vox
SHIP   SHIP2                    Shivan ship interior, sky is space.vox
NETLVL1 NETLVL2 NETLVL3         multiplayer
```

Per-level scale, from `FLOAT`: 507-byte manifest, 274 textures, 83 enemy and
object definitions, 19 navigation points, 13 terrain grids totalling 606,208
bytes. Across all 26 levels: 1,848 enemy and object definitions.

## The 31 cutscenes

```
MSLOGO      644 KB   Intro    22.8 MB   CREDITS  25.0 MB   Eject   25.5 MB
IOWAH1     26.3 MB   EYRIE1   18.4 MB   Kresh1   16.3 MB   SNOWOUT 15.3 MB
ASTROUT    15.7 MB   MORBOS1  14.9 MB   SHIV2DIE 14.1 MB   CHIMERA1 13.5 MB
SHIV2OUT   13.3 MB   YOUDIE    9.7 MB   SHIV1OUT  9.8 MB   ASTROID1  8.8 MB
TRI         6.9 MB   HELL      4.9 MB   ENDING    4.1 MB   ASTRIN    3.5 MB
SNOWIN      3.2 MB   MORBIN    3.1 MB   EYRDIE    3.0 MB   SNOWBRF   2.5 MB
MORBBRF     2.4 MB   SHIV1IN   2.3 MB   EYRBRF    2.3 MB   IOWAHDIE  2.7 MB
CHIMBRF     1.9 MB   IOWBRF    1.9 MB   KRESHBRF  1.8 MB   KRESHDIE  1.7 MB
```

The naming is regular: `*BRF` is a mission briefing, `*DIE` is the failure
movie, `*IN` and `*OUT` bracket a level.

## Models

342 `.BIN` models across both archives - 238 in GAME.POD, 104 in STARTUP.POD -
plus 18 `.TXT` animated models. All 342 parse cleanly; see
[MRGL](../formats/mrgl.md).
