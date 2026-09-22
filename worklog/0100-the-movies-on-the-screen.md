---
number: 100
title: The movies, on the screen
date: 2026-09-21
area: port, video, ui
files: crates/hb-fly/src/movie.rs, crates/hb-fly/src/main.rs, crates/hb-fly/src/sound.rs, crates/hb-audio/src/voices.rs
---

# 100. The movies, on the screen

[[98]] and [[99]] made the cutscenes readable; this plays them.

`hb-fly/src/movie.rs` is the clock and the handoff. A `Show` opens a file out
of `system/Story` - matching the name case-insensitively, because the `.LVL`
asks in lower case and the disc is mixed - decodes its whole soundtrack up
front into a `Wav` and hands it to the mixer as one voice, then walks the
picture by the wall clock at the movie's own 15.0015 frames a second.

Three ways in:

- `--movie NAME` plays one and nothing else.
- `--intro` plays the four the engine's own routines name, in their order:
  `tri.smk`, `hell.smk`, `mslogo.smk`, `intro.smk` (`0x45bbc9`, `0x45bbf9`,
  `0x45bc19`, each gated on `0x512650`).
- A chapter's briefing movie, from line 36 of its `.LVL`, plays before the
  briefing screen the port already drew. `--no-movies` turns that off.

Any key skips, which is what the engine's player does.

## Two things that were wrong the first time

**The world was still being drawn behind it.** The movie was a late branch in
the frame, after the terrain, the actors and the HUD had all been drawn into a
buffer nobody was going to look at. At 640x480 that is enough to make the
playback uneven. The movie now takes the frame and leaves.

**It was letterboxed.** A movie is 320x240 and the game's screen is 320x200,
and both were shown on the same 4:3 monitor - which is what this port presents
its window as, since [[30]]. Fitting a 4:3 picture inside a window that is
already 4:3 squashes it twice. It fills the frame.

## The logo is not broken

Reported as glitching, and worth checking rather than assuming: frames 0, 8,
16, 24, 32, 48, 64, 80 and 89 of `MSLOGO.SMK` side by side are a spark that
writes "Microsoft" in outline, the filled logo behind light rays, and then a
fade to black. The last two frames are black because the movie ends black. The
decode is clean; what was wrong was the playback around it.

**Still unknown:** what `0x512650` is called in the `.INI` - the three opening
routines are all gated on it and `cinemaFlag` is the obvious candidate but has
not been traced. And what the second argument to the engine's player at
`0x49f920` is: every call passes an empty string.
