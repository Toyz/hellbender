---
number: 109
title: Eve, and the spin she talks over
date: 2026-09-21
area: port, ui, decomp
files: crates/hb-fly/src/main.rs, docs/port/plan.md
---

# 109. Eve, and the spin she talks over

The game runs under Wine now, in a prefix beside the repository, and that
settled an argument I had been losing on the evidence: **there is a camera spin
when a new game starts**, with a voice over it. [[107]] was right that
`0x45a290` is an empty hook and wrong to conclude from that that nothing
happens.

## What Wine settled

`/mnt/data/source/re/hellbender-wine/run.sh` installs nothing: the disc is
copied into a prefix of its own, the 1.02 patch's `HELLBNDX.EXE` and
`STARTUPX.POD` are dropped in beside it, the disc is mapped as `D:` and the
install key is written. Two things had to change before it would start:
`redbookFlag=0`, because there is no CD audio device and it waits forever on
one, and the window has to be a virtual desktop.

Then `run.sh nospin`, which is the disc's executable with the first byte of
`0x45a2a0` patched to `ret`. The spin still happened. So the eight-second
sequence [[105]] read is not what plays at the start, and what does is still
unnamed.

## What is named

The voice is. `0x4201a9` is:

```
if [0x674d6c] == 1 { [0x674d6c] = 0; play_phrase(128) }
```

Phrase 128 is `opendiag.wav` - "Welcome, Councilor. I am Eve, the Hellbender's
Enhanced Virtual Entity. I am programmed to assist you in securing Coalition
victory. I will guide you through every battle -- but winning the war is up to
you." Two seconds of it.

`0x4834e3` is the only thing that sets that flag, and it sits in the new-game
routine, four instructions before the three calls that play the opening movies
([[106]]). Armed once when a game starts, cleared by the first frame that shows
it. Which is exactly "it plays once and never again".

## What the port does now

The sequence is the engine's - its curve from `0x45a2a0`, its eight seconds,
its phrase - and where it is hung is the port's, because which routine the
engine hangs it from is still open. After the movies and after the briefing
screen, the camera swings twice round the ship and back to level while Eve
says her piece, the ship holding exactly where `0x471333` put it. Once a
session, as the game does it. Any key skips.

Getting the "after" right took a correction: the welcome fired the moment the
level loaded, which is over the movies and over the briefing text. A movie and
the briefing screen both hold `dt` at zero, so the sequence now starts on the
first frame that actually runs.

**Still unknown:** which routine spins the camera at the start. Not
`0x45a2a0` - patching it out leaves the spin - and not the three-way view
cycles, which are keys. The ship's own heading has 46 writers and the spin may
be among them rather than in the camera at all.
