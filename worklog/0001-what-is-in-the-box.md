---
number: 1
title: What is in the box
date: 2026-09-17
area: content, tooling
files: tools/pod.py, tools/pe.py, docs/content/inventory.md
---

# 1. What is in the box

The disc image under `original/` is the retail Hellbender CD (Microsoft, 1996;
developed by Terminal Reality as the sequel to Fury3, itself the Windows
descendant of Terminal Velocity). Everything that matters is five items:

```
HELLBEND.EXE          1,685,504   the game, a 32-bit PE
system/STARTUP.POD   14,778,978   menus, cockpit art, sounds, demo recordings
system/GAME.POD      42,714,189   the 26 levels and everything in them
system/Story/*.SMK   ~280 MB      31 Smacker cutscenes
system/HELLBEND.INI       2,760   the settings the game writes back
```

The rest of the disc is not the game. `Setup/directx/` is the DirectX 2
redistributable - 200-odd driver files for 1996 sound and video cards, none of
which a port needs. `Goodies/` is Internet Explorer 2.0 and joystick profiles
for CH, Gravis and Thrustmaster sticks. `Setup/Fury32.dll` is left over from
Fury3 and is not referenced by Hellbender. `Patch v1.02/upgrade.exe` is a 7 MB
self-extractor holding a later build, which has not been opened yet.

`SMACKW32.DLL` is RAD Game Tools' Smacker decoder, so the cutscenes are
standard Smacker 2 and `ffmpeg` already plays them. They need no reverse
engineering, only a decoder binding.

`HELLBEND.HLP` is 9,451,149 bytes and is a real WinHelp 3.1 file - the first
bytes are the `3f 5f 03 00` WinHelp magic followed by `Hellbender Help`. It is
the manual, not disguised game data.

## The two archives

Both PODs are Terminal Reality's POD1 container and both are stored
uncompressed - see [2](0002-the-pod-archive-and-a-name-field-with-two-strings-in-it.md).
Extracted with `tools/pod.py extract`, they hold 5,547 files:

```
STARTUP.POD  1,206 entries   ART 761, SOUND 315, MODELS 104, UI 15, MUSIC 1,
                             DEMO 3, FOG 3, STARTUP 4
GAME.POD     4,341 entries   ART 3,420, DATA 536, MODELS 256, LEVELS 26,
                             FOG 72, MUSIC 14, SOUND 17
```

The split is by lifetime, not by kind: STARTUP.POD is what the front end and
the cockpit need and is held for the whole session; GAME.POD is streamed per
level.

## The binary is a debug build in all but name

`HELLBEND.EXE` is an MSVC PE, image base `0x00400000`, entry `0x004add30`, six
sections, timestamped 1996-09-04 22:02:18 UTC - two days before the file dates
on the disc. It kept every diagnostic string: 1,957 printable strings live in
`.data` between `0x00500000` and `0x0050e000`, and they carry function names.

```
0050168c  Unable to save ground type list
00500d2c  enemyPtrToIndex: Bad dynamic enemy pointer used
00501b3c  drawWeaponIcon: bad selected weapon
00501f48  logicTransportTakeoffLand: unknown logic phase
0050e618  Bad MRGL type
```

That changes the shape of the whole project. A string is one cross-reference
away from the routine that emits it, so most formats can be read out of the
loader rather than guessed from the bytes. `tools/pe.py` exists for exactly
that: `strings` with virtual addresses, `xref` for the 4-byte little-endian
occurrences of an address, and `dis` to disassemble from a VA through
`objdump -b binary -m i386`. Every format entry that follows was found that way.

The one thing the binary does not have is symbols: no debug directory, no
export table worth the name. Function names only exist where a diagnostic
string happens to contain one.

**Still unknown:** what is inside `Patch v1.02/upgrade.exe`, and whether its
`HELLBEND.EXE` differs enough to matter. The v1.02 build is the one most people
played.
