---
number: 101
title: Arriving and leaving
date: 2026-09-21
area: port, video, format
files: crates/hb-fly/src/main.rs, crates/hb-fly/src/movie.rs, docs/formats/lvl.md
---

# 101. Arriving and leaving

"The entry to the first level is still wrong, I swear it was animated in the
original." It was, and the `.LVL` has been naming the film since [[48]] read
the manifest - five story slots on lines 24 to 28, all of them called "in-level
story movie N" because nothing had looked at what the names were.

Dumping those five lines out of all 26 levels answers it in one go:

```
HOTH.LVL:     snowin.smk  null         null           null null
HOTH3.LVL:    null        snowout.smk  null           null null
MORBOS.LVL:   morbin.smk  null         morbos1.smk    null null
ROID.LVL:     astrin.smk  null         astroid1.smk   null null
ROID4.LVL:    null        astrout.smk  null           null null
SHIP.LVL:     shiv1in.smk shiv1out.smk null           null null
SHIP2.LVL:    null        shiv2out.smk null           null null
FLOAT.LVL:    null        null         eyrie1.smk     null null
IOWAH.LVL:    null        null         iowah1.smk     null null
JURASIC.LVL:  null        null         chimera1.smk   null null
KREASH.LVL:   null        null         kresh1.smk     null null
```

Every level naming a slot 1 is the first of its chapter and every one naming a
slot 2 is the last, and the names are `in` and `out`. So slot 1 is arriving and
slot 2 is leaving. Slot 3 is the chapter's own film, one a planet. Slots 4 and
5 are `null` in all 26 levels.

The engine's side is five short routines around `0x45b9f0`. Each compares a
name buffer against `"null"` - the buffers are 40 bytes apart at `0x666f58`,
`0x666f80`, `0x666fa8`, `0x666fd0` - and hands anything else to the player at
`0x49f920`, all gated on `0x512650`, the same word the four opening movies are
gated on. Which routine the level sequence calls when is a state machine at
`0x482440` that has not been read, so what the port does is the reading of the
names rather than of the code: slot 1 as a level opens, after its briefing, and
slot 2 as it is left.

So `morbos` now opens with `morbbrf.smk` and then `morbin.smk`, which is the
thirteen seconds of arrival that was missing.

**Still unknown:** when slot 3 plays. `morbos1.smk` is 1,333 frames, 89
seconds, far too long for a level opening, and `MORBOS` names it beside an
arrival movie - so it is triggered by something in the mission rather than by
the level starting. Also what `0x512650` is called in the `.INI`.
