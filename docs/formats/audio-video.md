---
title: Sound, music and cutscenes
status: solid
covers: SOUND\*.WAV, MUSIC\*.MOD, system/Story/*.SMK
worklog: 1
---

# Sound, music and cutscenes

All three are standard formats that need a decoder, not reverse engineering.

## .WAV - sound effects

Standard RIFF/WAVE, PCM, uncompressed. The shipped effects are 11,025 Hz, mono,
8-bit unsigned - which matches `mixSpeed=11025` in `HELLBEND.INI`.

```
RIFF ....  WAVE
fmt  16    format 1 (PCM), 1 channel, 11025 Hz, 11025 bytes/sec, align 1, 8 bits
data ....
```

332 files: 315 in STARTUP.POD, 17 in GAME.POD. Six more sit loose on the disc
under `SOUND\TAUNT1-6.WAV`, outside either archive - they are the multiplayer
taunts, and `HELLBEND.INI` has a `keyMultiTaunt1` through `keyMultiTaunt7`
binding for them.

## .MOD - music

ProTracker modules, six channel. The tag at offset 1080 is `6CHN`. The 20-byte
title field carries the sample path from the composer's machine:

```
(C) Terminal Reality\samples\kik...
```

15 modules: 14 in GAME.POD plus one in STARTUP.POD. Each level names its module
on line 15 of its [.LVL](lvl.md).

The game can also play CD audio instead - `redbookFlag` in `HELLBEND.INI`, and
line 35 of each `.LVL` is the track number.

## .SMK - cutscenes

Smacker, RAD Game Tools. `SMACKW32.DLL` on the disc is RAD's own 32-bit
decoder, so these are ordinary Smacker 2 files and `ffmpeg` decodes them
without help.

31 files under `system/Story/`, about 280 MB, from `MSLOGO.SMK` (644 KB) to
`IOWAH1.SMK` (26 MB). They are referenced by name from three places in each
[.LVL](lvl.md): five in-level story triggers on lines 24-28, a briefing movie
on line 37 and a failure movie on line 38.

## .VOX - the sky that is not there

`ART\SPACE.VOX` and `ART\STARS.VOX` are both zero bytes. Line 11 of a `.LVL`
names either a `.RAW` sky texture or `space.vox`, and the two space levels
`ROID` and `SHIP` name the latter. An empty file in that slot means "no sky
texture" - the engine draws stars instead. Nothing has to be decoded.

## Unknown

Nothing in the formats. Which of the 332 effects the engine binds to which
event is a separate question, answerable from the `.data` string tables - the
weapon and destruction sounds are named there in blocks.
