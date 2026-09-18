---
number: 23
title: The music plays, and the whole soundtrack uses four effects
date: 2026-09-17
area: audio, port, format
files: crates/hb-audio, crates/hb-fly/src/sound.rs, docs/formats/audio-video.md
---

# 23. The music plays, and the whole soundtrack uses four effects

`hb-audio` decodes the `.WAV` effects and plays the `.MOD` music, with no
dependency. `hb-fly` hands the samples to cpal and the level's music starts
when the level loads. Sixty frames a second and a soundtrack, which is the
first point at which the port is doing more than one thing at once.

## The soundtrack uses four effects

I wrote the mixer with portamento, vibrato, volume slides and sample offset,
and a list of effects it had *not* implemented so a listener could tell what
was missing. Then I asserted that list against the fifteen modules to pin which
effects the music actually needs.

The list came back empty, which I first read as a bug in the accounting. It is
not. Across every pattern of every module, exactly four effect numbers appear:

```
0x0  none              72,217 notes
0xb  position jump          15
0xc  set volume         13,769
0xf  set speed or tempo     15
```

No portamento, no vibrato, no volume slide, no arpeggio, no sample offset. The
composer used note, instrument and volume, and nothing else - the tracker as a
sampler rather than as a synthesiser. So a player that implements those four
plays this soundtrack completely, and the several hundred lines of effect
handling I wrote first are dead code against this data. I have kept them,
because they cost nothing and a replacement module would use them, but the test
now asserts the mixer ignores nothing, which is the honest version of the claim
I set out to make.

That is the second time in this project that writing the general case first was
the wrong order. The measurement takes a minute and it tells you what to build.

## The effects are not all 11 kHz

`HELLBEND.INI` sets `mixSpeed=11025` and the obvious assumption is that the art
matches. 327 of the 332 effects do. Four are 22,050 Hz and one is 8,287, which
is not a rate anybody chooses on purpose. All 332 are mono and 8-bit unsigned.

So a reader takes the rate from the header. The test asserts the exact
distribution rather than the round number, which is the difference between
recording what is there and recording what was expected.

## The module layout

The standard 31-sample ProTracker header, `6CHN` at offset 1080 in all fifteen.
Worth writing down because the sizes do not quite add up: a header plus
patterns plus sample data comes to between 4 and 15 bytes short of every file,
which is normal slack in a tracker's output and would otherwise look like a
misread.

The 20-byte title is where the composer's sample path ended up -
`(C) Terminal Reality\samples\kik...` - which is how you can tell the music was
made in house rather than licensed.

## The shape of it

`hb-audio` has no dependency and produces samples; `hb-fly/src/sound.rs` is the
only file in the workspace that touches an audio API, the same way `hb-fly` is
the only one that touches a window. The mixer renders stereo and the device
callback spreads it to however many channels the output has.

**Still unknown:** whether the engine plays these modules at all, or only when
`redbookFlag` is off - each level also names a CD audio track on line 35 of its
`.LVL`, and the two are alternatives. The sound effects are decoded and not yet
played by anything, because nothing in the port makes a noise yet.
