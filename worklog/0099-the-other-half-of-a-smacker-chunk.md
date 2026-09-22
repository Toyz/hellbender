---
number: 99
title: The other half of a Smacker chunk
date: 2026-09-21
area: format, video, audio, port
resolves: 98
files: crates/hb-formats/src/smk.rs, crates/hb-formats/tests/smacker.rs, crates/hb/src/main.rs, docs/formats/audio-video.md
---

# 99. The other half of a Smacker chunk

[[98]] left the audio as the one thing in the movies still unread. It is the
same machinery as the video, which is why it took one sitting.

A track's chunk is a `u32` of the unpacked size and then a bit stream: one bit
for "packed at all", one for stereo, one for sixteen-bit. Then byte trees -
one a channel at eight bits, two at sixteen, the low half and the high - in
exactly the shape the video's byte trees are written, presence bit and skip bit
and all.

Then differences. The first sample of each channel is stored whole, the
channels in reverse, sixteen bits big-endian. After that each sample is a code
from each of its channel's two trees, put together low half first, added to the
one before. Added, not clamped: `MSLOGO.SMK` runs into full scale and wraps,
and the format means it to.

## Two things that say it is right

Length. A test now decodes every audio chunk of every movie and compares the
run time of the sound with the run time of the picture. `MORBBRF.SMK` is 600
frames at 15.0015 a second and exactly 40.0 seconds of sound; `Intro.smk` is
1892 frames and 126.1 seconds. Within half a second on all 32.

Shape. Length alone would pass on noise, so `hb movie <name> <out.wav>` writes
the whole soundtrack and the correlation between neighbouring samples comes out
at 0.834 for the briefing, 0.956 for the intro, 0.991 for `YOUDIE.SMK`. Noise
is 0. A wrong tree or a wrong difference gives noise; nothing gives 0.99 by
accident.

So the movies are whole: 32 of them, picture and sound, with nothing but the
files to read them from.

**Still unknown:** whether any of the three header flags ever matters, since no
shipped file sets one, and what the two bits masked off each frame's size mean,
since no shipped frame sets either.
