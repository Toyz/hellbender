---
title: How the game is put together
status: partial
covers: HELLBEND.EXE
worklog: 1
---

# How the game is put together

Hellbender is a 1996 Windows 95 flight-shooter over an 8-bit heightmap, with a
software rasteriser and an optional Direct3D path. This page is the map: what
the subsystems are, and where the evidence for each one sits in the binary.

Everything here is read out of `HELLBEND.EXE`'s diagnostic strings and the code
they are referenced from. The binary keeps 1,957 printable strings in `.data`
between `0x00500000` and `0x0050e000`, many carrying function names, so the
subsystem names below are the game's own.

## The binary

```
PE, i386, image base 0x00400000, entry 0x004add30, GUI subsystem
built 1996-09-04 22:02:18 UTC

.text    0x401000   958,057 bytes
.rdata   0x4eb000    84,136
.data    0x500000 3,638,112 virtual, 93,184 raw   - mostly BSS: the world
.idata   0x879000    12,924
.rsrc    0x87d000   368,920
.reloc   0x8d8000   104,282
```

The 3.5 MB of uninitialised `.data` is the game state. Two of its arrays are
already located: the 128 x 128 x 6-byte ground grid at `0x0073bcc0` and the
128 x 128 x 18-byte ground box grid at `0x00675fa0`.

## Subsystems, and where they are

### World and terrain

Loader at `0x00412c00`. The world is a 128 x 128 cell grid with a base surface,
two sets of extruded boxes and one set of chambers - see
[terrain](../formats/terrain.md). Queries into it are named in the
diagnostics:

```
heightAtGrid: bad value passed for parameter layer
groundTriangleMidpoint: bad value passed for parameter layer
groundTriangleInt: illegal layer passed
intersectingBoxSurface: bad layer passed
```

Each takes a *layer*, so the ground, the two box sets and the chambers are
addressed uniformly by index. A cell is two triangles, not a quad -
`groundTriangleMidpoint` and `groundTriangleInt` both say so - which is what a
port's collision and height query must reproduce.

### Enemies and their logic

Enemies come from the level's [.DEF](../formats/level-text.md) file, up to
1,848 definitions across the 26 levels. Each has a behaviour, and the behaviours
are named by the "unknown logic phase" diagnostics:

```
logicFollowPath              logicFollowPathAttitude
logicFollowGroundPath        logicPathDogfight
logicTransportDisappear      logicTransportTakeoffLand
logicTransportTakeoffLandLeave
```

Every one of them is a phase machine over a *course* - a numbered path from the
level's `.CRS` file, referenced by `.DEF` line 15 - and every one emits
`"Bad course ID for enemy"` when the id does not resolve. There is a
distinction between dynamic and static enemies: `enemyPtrToIndex: Bad dynamic
enemy pointer used`. Enemy state is saved and restored with the game
(`Unable to save enemy state`), so enemies are persistent objects, not
respawning spawners.

### Weapons and powerups

A 31-entry table, names at `0x00502518` and pickup models at `0x00502384`, in
the same order:

```
 1 Rapid-Fire Lasers          f6rfl.bin       17 Damage 25%        f6dam1.bin
 2 ServoKinetic Lasers (NU)   powerpla.bin    18 Damage 50%        f6dam2.bin
 3 Dispersion Cannon 14       f6dc.bin        19 Damage 100%       f6dam3.bin
 4 Dead-On Missiles           f6dom.bin       20 Energy 25%        f6eng1.bin
 5 Vipers                     f6vip.bin       21 Energy 50%        f6eng2.bin
 6 Bion Fury Missiles (NU)    powersup.bin    22 Energy 100%       f6eng3.bin
 7 Shield Restore (nu)        powershe.bin    23 Message Pod       powercan.bin
 8 Cloak (Multi only)         powervis.bin    24 Super Weapon Piece 1  f6swep.bin
 9 Invincibility              powerinv.bin    ...
10 Guided MIRV                f6gmv.bin       31 Super Weapon Piece 8  f6swep.bin
11 Turbo Thrust (NU)          powerzap.bin
12 Energy Can (NU)            powercan.bin
13 Cruise                     f6cru.bin
14 Cluster                    f6cls.bin
15 MIRV                       f6mirv.bin
16 MINE                       f6mine.bin
```

Five entries are marked "Not used" in their own display string - the game
shipped with dead weapon slots. The abbreviations confirm the pairing:
`rfl` rapid-fire lasers, `dc` dispersion cannon, `dom` dead-on missiles, `vip`
vipers, `gmv` guided MIRV, `she` shield, `vis` invisibility, `inv`
invincibility, `swep` super weapon piece. The pairing is by table index and is
an inference, not a measured link; entries 2, 6, 11 and 23 are the ones where
name and model do not obviously agree.

`HELLBEND.INI`'s `[Control]` section has a key binding per weapon, which gives
the player-facing set: vulcan cannon, dispersion cannon, SKL, RFL20, DOM,
cruise missile, viper missile, cluster missile, MIRV missile, guided MIRV,
mine - on `` ` `` and 1 to 0, in that order.

### Controls

`[Control]` is the whole of them, sixty four settings read with
`GetPrivateProfileInt` from `.\system\hellbend.ini` (`0x42d274` on). Each is
read with the setting's *current* value as the default, so the defaults are
the globals' initial values at `0x512758` and up, and a machine with no `.INI`
plays on exactly those. Every key is a set 1 scan code, the same numbers a
[`.DMO`](../formats/demo.md) records.

| Setting | Default | Setting | Default |
| --- | --- | --- | --- |
| `upKey` | up (72) | `keyCloak` | C (46) |
| `downKey` | down (80) | `keyMissileLock` | V (47) |
| `leftKey` | left (75) | `keyBeacon` | B (48) |
| `rightKey` | right (77) | `keyHeadlight` | L (38) |
| `rollLeftKey` | home (71) | `keyNaviComp` | N (49) |
| `rollRightKey` | pgup (73) | `keyNavChoose` | tab (15) |
| `throttleUpKey` | X (45) | `keyMap` | M (50) |
| `throttleDownKey` | Z (44) | `keyMapZoomIn` | [ (26) |
| `fireKey` | space (57) | `keyMapZoomOut` | ] (27) |
| `weaponKey` | F (33) | `keyTransferWeapon` | , (51) |
| `keySelectPrevWeapon` | - (12) | `keyTransferShields` | . (52) |
| `keySelectNextWeapon` | = (13) | `keyCrosshair` | T (20) |
| `keyChangeViews` | O (24) | `keyCockpitLabel` | / (53) |
| `keyInstrument` | I (23) | `keyEndGame` | esc (1) |
| `keyViewLeft` | ins (82) | `keyViewForward` | keypad - (74) |
| `keyViewRight` | del (83) | `keyViewBack` | keypad + (78) |

The weapons take `` ` `` (41) and 1 to 0 (2 to 11). `keyMultiTalk` and
`keyMultiTaunt1` to `7` are F5 to F12 and are for network play. The joystick's
buttons are `buttonFire`, `buttonWeapon`, `buttonThrottleUp`,
`buttonThrottleDown` and `buttonFunction0` to `7`.

### Rendering

Two paths. The software rasteriser is the default; Direct3D is opt-in through
`useDirect3D` in `HELLBEND.INI` and has its own error block at `0x005002c0`
(`Cannot lock back buffer`, `Bad begin scene`, `3D Adapter Error`,
`Texture load failed: ...`). The software path renders in 8-bit indexed colour
and does shading, fog and blending through the
[colour tables](../formats/colour-tables.md).

Three video modes - 320x200, 320x400 and 640x480 - selected by `gamePIXX` and
`gamePIXY`, with the art authored three times over. The `[Graphics]` section of
`HELLBEND.INI` is the full list of render switches: `perspectiveFlag`,
`airShadowFlag`, `skyTextureFlag`, `flatShadeFlag`, `useZFlag`, `ditherFlag`,
`blendFlag`, `filterFlag`, `antialiasFlag`, `textureResolution`,
`pixelDoubleBlit`, `useMMXFlag`, `maxD3DTextures`.

`autoQuality` with `autoMinFrameRate=8` means the engine drops detail to hold 8
frames a second - a useful datum about what its own authors thought the floor
was.

### Moving geometry

`processBoxQuake: no match for watchBox found` and the counters
`-------- Ground Quake Count -------` and `-------- Box Quake Count --------`.
The level's `.QKE` file drives it, and `.QKE` is the largest of the small
per-level text files, up to 72 KB. "Quake" here is animated terrain - doors,
lifts, shaking ground - and a box being watched implies the system tracks which
cells are in motion so collision stays correct.

### Sound

Mixed in software at `mixSpeed` (11,025 Hz by default) with a per-device
configuration table in `system/HELLBEND.INF` - 60-odd 1996 sound cards, each
with `WaveBlocks`, `WaveBlockLen`, `SamplesPerSec`, `Remix` and `GoodWavePos`.
Music is either a ProTracker module or a CD audio track; see
[audio](../formats/audio-video.md).

`[Sound]` of `HELLBEND.INI` carries `musicFlag`, `soundFlag`,
`advancedSoundOptions`, `preferredSoundDevice` and the two volumes,
`musicVolume` and `soundVolume` (`0x5125bc` and `0x5125c0`), which are 16.16
and 1.0 by default.

There are 32 voice slots at `0x655240`, 96 bytes each. A voice carries its
volume at `+0x14`, a left and a right at `+0x30` and `+0x34` - both started at
half the volume - the loop flag at `+0x1c`, and a position the allocator
(`0x41f0b0`) is handed by pointer, with `-1, -1, -1` meaning "no place". Every
volume is multiplied by `soundVolume` as it is set (`0x41f34f`). `0x452dac`
walks all 32 and halves the volume, both sides and two more fields of each
when one particular voice is sounding, which is a duck of some kind.

What the driver does with a voice's position - attenuation, panning, or both -
is still not read.

### Multiplayer

DirectPlay, over IPX, TCP/IP or serial - `system/DPSERIAL.DLL`,
`DPSOCKET.DLL`, `DPWSOCK.DLL` ship alongside the game. Three dedicated levels,
`NETLVL1` to `NETLVL3`, plus a cloak powerup marked "Multi only" and ten
canned taunt messages in `HELLBEND.INI`.

### Saved games

`Fatal error: corrupt save game file`, `Unable to save enemy state`,
`Unable to save ground type list`. A save is a snapshot of the enemy array and
the terrain's mutable state, not just the player.

## Fixed point

The game is fixed point throughout. 65,536 is 1.0 in the `.DEF` damage
multipliers; the `.ANI` frame delay of 6,553 is 0.1 s; the `.LVL` weather
parameters are 5.0, 10.0, 15.0, 20.0 and 30.0. Angles are a 16-bit circle: the
model format's `angleList` values run 0-65,535 and the `.LVL` headings are
16,384, 24,576, 32,768, 40,960, which are quarter, three-eighths, half and
five-eighths of a turn.

## Unknown

The main loop, the flight model, the rasteriser's inner loops, the collision
system, and how a mission is scored or won. None of those have been read yet;
everything above is the shape of the thing, not its contents.
