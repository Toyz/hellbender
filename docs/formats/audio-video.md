---
title: Sound, music and cutscenes
status: solid
covers: SOUND\*.WAV, MUSIC\*.MOD, system/Story/*.SMK
worklog: 1, 23
---

# Sound, music and cutscenes

All three are standard formats that need a decoder, not reverse engineering.

## .WAV - sound effects

Standard RIFF/WAVE, PCM, uncompressed. Every one of the 332 is **mono and
8-bit unsigned**, and nearly all are 11,025 Hz, which matches `mixSpeed=11025`
in `HELLBEND.INI`:

```
11,025 Hz   327
22,050 Hz     4
 8,287 Hz     1
```

So a reader must take the rate from the header rather than assume the one the
settings file names.

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

ProTracker modules, six channel. The tag at offset 1080 is `6CHN` in all
fifteen, and the layout is the standard 31-sample one:

```
0x0000  char[20]   title
0x0014  31 * 30    sample headers
           char[22] name
           u16 be   length in words
           u8       finetune, a signed nibble
           u8       volume, 0 to 64
           u16 be   repeat start in words
           u16 be   repeat length in words
0x03b6  u8         song length, positions
0x03b7  u8         restart position
0x03b8  u8[128]    order: the pattern at each position
0x0438  char[4]    channel tag
0x043c  patterns, 64 rows of `channels` notes of 4 bytes
        sample data, signed 8-bit, in header order
```

The 20-byte title carries the sample path from the composer's machine -
`(C) Terminal Reality\samples\kik...` - which is how the modules are
identified as Terminal Reality's own rather than licensed.

15 modules: 14 in GAME.POD plus one in STARTUP.POD. Each level names its module
on line 15 of its [.LVL](lvl.md).

### The soundtrack uses four effects

Across every pattern of every module, exactly four effect numbers appear:

```
0x0  none
0xb  position jump
0xc  set volume
0xf  set speed or tempo
```

No portamento, no vibrato, no volume slide, no arpeggio, no sample offset. A
player that implements those four plays this soundtrack completely, which is a
useful thing to know before writing one.

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
