---
number: 66
title: Three sounds the port was missing
date: 2026-09-20
area: decomp, audio, port
files: crates/hb-audio/src/voices.rs, crates/hb-audio/tests/held.rs, crates/hb-fly/src/sound.rs, crates/hb-fly/src/main.rs, crates/hb-fly/src/battle.rs, docs/engine/simulation.md
---

# 66. Three sounds the port was missing

`STARTUP.POD` carries 315 `.WAV` files and the port triggers a few dozen of
them. Rather than guess at the rest, the way to find what is missing is to
take a filename, find the string in the image, and read what plays it. Three
came out of one sitting.

## The warning is about a missile chasing you

`m-lock7.wav` is referenced from `0x44ea56`, and the branch it sits in first
calls `0x47f6d0`, which walks the sixteen slots of the guided missile pool at
`0x613700` looking for one whose fields match `[0x503c68]` - the local
player. While one does, the engine plays `m-lock7.wav` and keeps the voice at
`0x505394`; when none does, it stops it.

So it is not the tone for a lock you are holding, which is what the name
suggests. It is the missile warning, and the port had nothing for it.

## The afterburner has a note

`engine4.wav` is referenced once, from `0x47d6df`, right after `blast7.wav`.
The kick and then the note, and the note's voice slot gets its loop flag set
at `+0x1c`, so it holds until the burn ends. The port already had the fuel,
the speed and the drain, and no sound at all.

## A powerup says so

`power-1.wav` appears seven times in the data, each beside a different case's
HUD line - `0x40e422` prints `Invincible!` and then plays it. The port had
the message and not the sound.

## What it needed

Two of the three hold, and `hb-audio`'s voices were one-shot only: a voice
ended when it ran out of samples and there was no way to stop one early.
`hold` now returns a handle and loops the sample, `stop` takes the handle,
and a one-shot is a voice with no handle - so the old path is unchanged.
Two tests: a held sound is still as loud in the second half of half a second
as in the first and goes silent the moment it is let go, and a one-shot still
ends on its own.

The engine keeps its held voices in the same 32-slot table as everything
else, with a flag per slot, which is the same shape.

**Still unknown:** where a sound is. The voice slots carry a position at
`+0x40` and `0x41f0b0` takes one, with `-1, -1, -1` meaning "no place", but
what the mixer does with it - attenuation, panning, or both - is in the
sound driver and has not been read. Everything this port plays is played flat.
