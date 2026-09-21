---
number: 80
title: Eight briefings, one a chapter
date: 2026-09-21
area: decomp, format, content, port
files: crates/hb-formats/src/brief.rs, crates/hb-formats/tests/against_the_game.rs, crates/hb/src/main.rs, docs/formats/level-text.md
---

# 80. Eight briefings, one a chapter

Line 0 of every `.LVL` names `DATA\<stem>.TXT` and the port has been
resolving it and ignoring it since [[3]]. [[29]] wrote the format down. It is
a model, a picture and some prose:

```
Globe.Bin
Morbos00.Raw
PLANET: Morbos
MISSION: Counterstrike

We have tracked the Bion Commandos who destroyed the Fighter Academy on
Sebek to the remote world of Morbos.
...
.
```

## The second name is not the backdrop

The doc said "a backdrop", and rendering it said otherwise: `MORBOS00.RAW` is
4,096 bytes. 64 by 64. It is a **texture**, and `ART\BRIEF.RAW` - 64,000
bytes, 320 by 200, with `BRIEF.ACT` beside it - is the screen, which the
engine opens by name (`0x50c594`, `Unable to open brief.raw`) and which no
briefing mentions because they all use it.

So the globe wears the planet you are about to fly to, and turns on the
briefing screen while you read about it. `globe.bin` is one of the seven
models [[36]] listed as untextured throughout, 576 polygons of it - because
its texture is not in the model, it is in the briefing.

## Eight, not twenty three

The test I wrote asserted a briefing per level and failed with a list. The
list was exactly the fifteen levels that are not the first of their chapter.

One briefing a chapter: *Counterstrike*, *Savior*, *Protector*, *Pin Point*,
*Steel Forge*, *Heavy Metal*, *Freedom*, *Dagger's Heart*. Which is
[[79]]'s chapter structure arrived at a second time, from the other end - the
campaign table in the executable said eight chapters, and the data says eight
briefings.

`hb brief <level> <out.png>` draws one: the screen, the prose inside its
frame. In the small font, because `FONT.BIN` is 23 pixels a line and a
briefing runs to twenty-one of them, which does not fit a 200-line screen.

**Still unknown:** which font the engine actually uses there, and where on
the screen it puts the text - the port centres it in the frame, which reads
but is a choice. The globe is drawn by nobody yet: it wants a viewport and a
turn, which is the same machinery the reticle, the objective arrow and the
weapon picture all want ([[72]]). And the `.SMK` movies each level names -
the briefing movie, the story movies, the death movie - are still just names.
