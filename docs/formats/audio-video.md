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
decoder, so there is nothing in `HELLBEND.EXE` to read and everything below was
worked out from the files themselves. `hb-formats`'s `smk` decodes them.

32 files under `system/Story/`, about 280 MB, from `MSLOGO.SMK` (644 KB) to
`IOWAH1.SMK` (26 MB). Every one is `SMK2`, 320 by 240, 15 frames a second, one
audio track, no flags. They are referenced by name from three places in each
[.LVL](lvl.md): five in-level story triggers on lines 24-28, a briefing movie
on line 37 and a failure movie on line 38.

### The header

104 bytes, then a `u32` a frame, then a byte a frame, then the packed trees,
then the frames one after another. That accounts for every byte of all 32
files exactly.

```
+0x00  char[4]  "SMK2"
+0x04  u32      width, 320
+0x08  u32      height, 240
+0x0c  u32      frames
+0x10  i32      rate: > 0 milliseconds a frame, < 0 hundredths of one,
                0 means ten a second. -6666 in every file, so 15.0015 a second
+0x14  u32      flags: 1 a ring frame, 2 y-interlaced, 4 y-doubled. 0 here
+0x18  u32[7]   each audio track's largest unpacked size
+0x34  u32      packed size of the four trees
+0x38  u32[4]   the four trees' sizes: map, colour, full, type
+0x48  u32[7]   each track's rate word: bit 31 compressed, bit 30 sixteen-bit,
                bit 29 stereo, sample rate in the low 24. 0xc0002b11 is
                11,025 Hz mono 16-bit compressed, which is 31 of the 32
+0x64  u32      zero
+0x68  u32[frames]  each frame's length, bit 0 a keyframe and bit 1 unused -
                mask both off. No shipped frame has either set
       u8[frames]   bit 0 the frame carries a palette, bits 1-7 which audio
                tracks it carries. Only track 0 is used, and only one frame
                in a movie usually carries a palette
       u8[trees]    the four trees, packed
```

### The four trees

One bit stream, least significant bit of each byte first, holding four
sixteen-bit Huffman trees in the order the sizes list them: the block map, the
two-colour pairs, the full-colour pairs and the type codes.

Each is a bit that says whether it is there at all, then two byte trees for the
low and high halves of a value, then three sixteen-bit escape values, then the
tree itself, then one more bit.

A byte tree is the same shape: a presence bit, then a bit per node - 0 a leaf
and the eight bits of its value, 1 a branch and then its two children - then
one more bit. The sixteen-bit tree's leaves each hold one code from the low
tree and one from the high tree.

The escapes are the format's cache. A leaf whose value is one of the three does
not mean that value: it means the first, second or third most recently decoded
value, and every decode that is not already at the front moves the list along.

**The cache is emptied at the start of every frame.** All four of them, back to
zero. This is the one thing about the format that cannot be worked out by
reading a file, because a decoder that carries the caches across frames still
decodes most frames perfectly - every one of `MORBBRF.SMK`'s 600 - and quietly
ruins the ones where a frame's first cached code lands on a slot the previous
frame left different. Half of `MORBIN.SMK` came out wrong that way while the
briefing next to it was exact.

**The header's size for a tree is `(leaves + 1) * 8`.** That holds for all four
trees of all 32 movies, which is what says this reading is right. The packed
blob has a little slack after the fourth tree - 63 bytes in `Intro.smk`, 225 in
`KRESHBRF.SMK`.

### A frame

A palette chunk if the frame's byte says so, then a chunk per audio track it
carries, each behind its own four-byte length, then the rest is video.

The palette chunk's first byte is its length in fours. Then, until 256 entries
are filled: a byte with bit 7 set keeps the next `(b & 0x7f) + 1` entries as
they were, a byte with bit 6 set copies `(b & 0x3f) + 1` entries from the old
palette starting at the next byte's index, and anything else is the first of
three six-bit channels. Six bits become eight as `(v << 2) | (v >> 4)`.

The video is 4x4 blocks in rows, left to right and top to bottom. A correct
frame covers every block and stops inside its own bytes; a decoder out of step
with the stream fails one or the other, which is what the port tests. A code from
the type tree carries the kind in its low two bits, a run in the next six, and
for a fill the colour in the rest. The run is the code itself up to 59, then
128, 256, 512, 1024, 2048.

- **0, two-colour.** A code from the colour tree is two palette indices, and a
  code from the map tree is sixteen bits, one a pixel, low bit first, choosing
  between them.
- **1, full colour.** Two codes from the full tree a row, the right pair before
  the left, each code two palette indices.
- **2, skip.** The run's blocks keep what the last frame left there.
- **3, fill.** The whole run is one colour, the type code's top bits.

### A sound chunk

A `u32` of the unpacked size in bytes, then a bit stream of its own. The first
bit says whether what follows is packed at all, then one bit for stereo and one
for sixteen-bit sound - every shipped track is mono and sixteen-bit.

Then a byte tree a channel for eight-bit sound, or two a channel for sixteen -
the low half and the high - written exactly like the video's byte trees,
presence bit and all.

The samples are differences. The first of each channel is stored whole, the
channels in reverse order, sixteen bits big-endian; every one after it is a
code from each of its channel's trees, put together low half first and added to
the last sample, wrapping rather than clipping. Which is the format's own
choice: `MSLOGO.SMK` reaches full scale and wraps.

Each track is exactly as long as its movie - 40.0 seconds of sound for 600
frames at 15, 126.1 for 1892 - and the samples correlate with their neighbours
at 0.96 and up, which is what says this is sound and not noise.

## .VOX - the sky that is not there

`ART\SPACE.VOX` and `ART\STARS.VOX` are both zero bytes. Line 11 of a `.LVL`
names either a `.RAW` sky texture or `space.vox`, and the two space levels
`ROID` and `SHIP` name the latter. An empty file in that slot means "no sky
texture" - the engine draws stars instead. Nothing has to be decoded.

## Unknown

Nothing in the formats. Which of the 332 effects the engine binds to which
event is a separate question, answerable from the `.data` string tables - the
weapon and destruction sounds are named there in blocks.
