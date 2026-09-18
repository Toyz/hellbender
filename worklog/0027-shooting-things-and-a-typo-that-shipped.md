---
number: 27
title: Shooting things, and a typo that shipped
date: 2026-09-17
area: engine, audio, port, content
files: crates/hb-sim/src/combat.rs, crates/hb-audio/src/voices.rs, crates/hb-fly/src/main.rs, docs/engine/simulation.md
---

# 27. Shooting things, and a typo that shipped

Space fires. A laser sound plays, a shot flies, and when it hits a placed
object enough times that object becomes its wreck - or vanishes - with its own
destroy sound. It is the first loop in the port with cause and effect in it.

The fire button is the space bar because `HELLBEND.INI` binds `fireKey=57`,
which is its scan code, and 636 of the 638 key presses in the recorded demos
are it. That moved "stop" to `x`.

## How much of it comes from the data

More than I expected, for something whose engine code has not been read.

**What an object becomes.** Every `.DEF` type names a second model, and it is
one of seven: `wbnkruin.bin` for the 439 bunker types, five dome ruins
`krdom0r.bin` to `krdom4r.bin`, and `cube.bin` for the other 1,394. `cube.bin`
is the un-normalised test model from
[16](0016-the-instance-list-was-in-the-same-file-all-along.md) - the one whose
vertices never went through the exporter's normalisation - and a model like
that in the "wreck" slot of three quarters of the types reads as the exporter's
placeholder for "nothing left". So a bunker leaves ruins and a tower simply
goes. That is an inference, and it makes the game look right where the literal
reading would turn every destroyed building into a small cube.

**What it sounds like.** Line 24, the second name under
`{ Escape and destroy sound files`, is the destroy sound, and 143 of them
resolve to a `.WAV`. One does not: a type names **`CRY-DES.DEF`** where it means
`CRY-DES.WAV`. A typo in the shipped data, the second after the dangling course
references in
[15](0015-the-world-is-centred-and-def-is-a-table-of-types-not-a-list-.md). The
engine presumably fails to open it and plays nothing, and the port does the
same.

**How hard it is to kill.** Field 2 of a type's first line was established as a
function of the model rather than a position. That is the shape hit points
should have, and read as 16.16 it orders sensibly - a cube 0.55, a bunker 4.65,
a control centre 11.9. The port uses it as hit points and says in three places
that this is an inference. The damage multipliers on lines 17 to 19 are
unambiguous by comparison: the file labels them
`= cannonDamage, laserDamage, missileDamage`.

What is not in the data - shot speed, lifetime, damage, rate of fire, the hit
test itself - is chosen, and `docs/engine/simulation.md` lists each.

## Effects over music

`hb-audio` grew a voice pool: up to twelve one-shot `.WAV`s, each stepped at
its own rate against the output rate and added over the music. The rates
matter - 327 effects are 11,025 Hz but five are not - so each voice carries its
own step. Twelve is chosen, and dropping the oldest when a thirteenth arrives
is what fixed channel counts on 1996 hardware did anyway.

## Tests

The synthetic ones fire along a heading and check where the shot goes -
heading zero along +z, a quarter turn along +x, positive pitch downward, all in
the convention the demo measured. A shot hits the nearest thing in its path,
not the first thing in the list. The real-data one stands ten units off
`FLOAT`'s first placement and fires until it is destroyed, and checks every
placed object in `FLOAT` has positive hit points and so can be.

**Still unknown:** whether field 2 really is hit points. The engine's own
weapons - their speeds and damage, and the fourteen weapon slots in the powerup
table from [1](0001-what-is-in-the-box.md). What the escape sound on line 23 is
for. Nothing shoots back yet.
