---
number: 63
title: The floating mine goes off on you
date: 2026-09-20
area: decomp, engine
files: docs/engine/simulation.md
resolves: 45
---

# 63. The floating mine goes off on you

Weapon 28 has been on the not-ported list since worklog 38, deferred each
time because it needed "the 100-slot pool at `0x61bde0` and its own update".
Both are now read, and the weapon turns out to be stranger than the list
implied: **a mine is triggered by the local ship and by nothing else.**

Dropping one (`0x47d82a`) tests the ship's speed at `0x50cc50` against
`0x61a80`, which is 6.1 units a second. Below it the drop is refused and the
HUD gets `Go Faster to Deploy Mine` - a string that has been sitting in the
image at `0x50f410` all along. Above it the mine goes down at the ship's
position less the third row of its rotation matrix, so a unit behind it, into
the first free slot of a hundred at `0x61bde0`.

`0x479670` runs the pool once a frame. A live mine is drawn with a model
loaded at startup, and then tested against `0x5b3830` - the ship - per axis
against a trigger radius of 16 units, wrapped for the world's edge. Inside
it, the mine arms; armed, it spins its two angles at a quarter and an eighth
of a circle a second, runs the same test again, and on passing calls the
splash at `0x47d3b0` with 32 units and its own damage, as weapon kind 26.

Nothing else is tested. Not the actors, not other shots - the ship.

## Which makes it a multiplayer weapon

In a network game that is exactly right: a mine another player dropped is
placed in the same pool by a packet handler (`0x4795f0`, from `0x431e41` and
`0x434a59`), and `0x5b3830` is *your* ship, so their mine goes off when you
fly into it. The pool is shared and the trigger is always local.

In a single-player level the only thing that can set your mine off is you.
The damage then falls on whatever is inside the 32-unit blast, which may well
include the enemy you led over it, so it is usable - but as a weapon by
proxy rather than a trap that catches anything on its own.

## The copy that nothing calls

There is a second drop at `0x479500`: the same speed test, the same message,
the same walk of the same pool, written out again. Nothing in the image
references it - `calls` reports no callers and a scan for the address as a
datum finds nothing, so it is not in a table either. The live one is inlined
into the weapon dispatch at `0x47d82a`; this is the copy that was not.

One process note that cost me twenty minutes: I nearly recorded the whole
pool as dead code on the strength of an xref listing I had truncated to eight
lines with `sed`. The update at `0x479681` was the ninth. An absence is only
evidence when the whole list has been looked at.

**Still unknown:** what `+0x1c` and `+0x28` of a mine slot carry - the drop
writes one from an argument and leaves the other zero, and the update reads
neither. Whether the engine draws an armed mine differently from a dormant
one beyond the spin.
