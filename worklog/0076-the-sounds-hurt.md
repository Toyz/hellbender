---
number: 76
title: The sounds hurt
date: 2026-09-20
area: port, audio, decomp
files: crates/hb-audio/src/voices.rs, crates/hb-audio/src/mixer.rs, crates/hb-audio/tests/loudness.rs, crates/hb-sim/src/combat.rs, crates/hb-sim/tests/combat.rs, crates/hb-fly/src/sound.rs, crates/hb-fly/src/battle.rs, crates/hb-fly/src/main.rs, docs/engine/overview.md
---

# 76. The sounds hurt

Toyz: "the sounds hurt my ears". They did, and for three reasons, only one of
which needed the engine read to fix.

## The clamp was per voice

`mix_into` added each voice into the output buffer and clamped as it went.
Three explosions at once were therefore three separate squared-off waves, not
one loud one - and a squared-off wave is a buzz. It now sums into floats and
bends the whole thing once at the end: below 60% of full scale nothing
happens, and above it what is left of the range is approached rather than
reached. Four, eight and twelve voices are each louder than the last and none
of them touches the ceiling, which is a test.

Every voice also plays at 40% now. The music is already mixed down by twelve
and every effect was going on top of it at up to full scale, so one explosion
was the whole range on its own. The engine plays its own effects at about
half - the missile warning goes out at `0x8000` - and hands them to a driver
with headroom, which is what the 40% stands in for.

## An 11 kHz sample at 48 kHz was a staircase

Every effect in the game is 11,025 Hz and the device is usually 44,100 or
48,000, so the port was holding each sample for four output samples. That
staircase is grit, and on a sample that is already noisy it is the part that
hurts. It slides between samples now.

## Everything was at full volume wherever it was

The engine hands the sound driver a position - `0x41f0b0` takes one by
pointer, `-1, -1, -1` meaning "no place" - and what the driver does with it
has still not been read. But "no attenuation at all" is not neutral: it is
its own invention, and a louder one. An explosion 79 units away, which is the
last unit at which an actor even thinks (`0x42f7b9`), was arriving as loudly
as one on the wing.

`combat::falloff` is the port's own, and says so: flat on top of you, squared
down to nothing at the 80 units of `in_range`. The explosions, the near
misses and the doors go through it. The player's own guns and the warnings do
not - they are on the ship.

## And the two volumes it always had

`[Sound]` of `HELLBEND.INI` carries `musicVolume` and `soundVolume`, both
16.16 and 1.0 by default (`0x5125bc`, `0x5125c0`), and every voice's volume
is multiplied by `soundVolume` the moment it is set (`0x41f34f`). The port
reads both now, and `HB_VOLUME` scales them again for a machine where the
game's own 1.0 is too much.

While looking: the voice slots are 32 of 96 bytes at `0x655240`, with the
volume at `+0x14`, a left and a right at `+0x30` and `+0x34` - both started at
half - and the loop flag at `+0x1c`. `0x452dac` walks all 32 and halves the
volume and both sides of every one while some particular voice sounds, which
is a duck of some kind and is not ported.

**Still unknown:** what the driver does with a voice's position, which is the
same unknown [[66]] ended on; what `0x452dac`'s duck is for; and whether the
engine's effects are resampled or played at the device's rate, since the
whole mixer runs at `mixSpeed` (11,025 by default) and may simply never have
had to.
