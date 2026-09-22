---
number: 106
title: Four films before the ground
date: 2026-09-21
area: decomp, port, video
resolves: 101
files: crates/hb-fly/src/movie.rs, crates/hb-fly/src/main.rs, docs/formats/lvl.md
---

# 106. Four films before the ground

"We're missing like 4 or 5 video segments between the Hellbender title and
planet fall." Counted, and yes: the port was playing two of them and had the
first four in the wrong order.

## The order the game opens in

Three calls in a row at `0x4835a3`, `0x4835a8` and `0x4835ad`, each into a
routine that checks `0x512650` and then plays its names:

```
0x4835a3 -> 0x45bbf0   mslogo.smk
0x4835a8 -> 0x45bbc0   tri.smk, then hell.smk
0x4835ad -> 0x45bc10   intro.smk
```

Microsoft, Terminal Reality, the title, the story. [[101]] had them as Terminal
Reality, the title, Microsoft, the story, which was a guess at the order of
three routines rather than a reading of their callers.

## The two a level opens with

`0x45b9f0` is called as a level starts (`0x481627`), and it plays two of the
five story slots. The buffers are 40 bytes apart from `0x666f58`, in slot
order, so which two is readable: it pushes `0x666fa8` first and `0x666f58`
second - **slot 3, then slot 1**. The chapter's own film and then the arrival.

The sibling at `0x45ba50` plays `0x666f80`, slot 2, and is called from the
level-end sequence at `0x4824dc`. Which is the departure, as [[101]] had it.

So `MORBOS`, the first mission, opens like this:

```
mslogo.smk    Microsoft
tri.smk       Terminal Reality
hell.smk      the title
intro.smk     the story
morbbrf.smk   the briefing
morbos1.smk   the chapter's film
morbin.smk    the arrival
```

Four films between the title and the ground, which is the count that was
reported.

## In the port

`hb-fly` plays all seven now, in that order, each skippable with a key. The
four openers play by default rather than behind a switch, since that is what
the game does; `--no-intro` skips them and `--no-movies` skips everything.

**Still unknown:** what `0x512650` is called in the `.INI` - every one of these
routines is gated on it, and `cinemaFlag` is the obvious candidate and has not
been traced. Slots 4 and 5 are `null` in all 26 levels and nothing has been
found that plays them.
