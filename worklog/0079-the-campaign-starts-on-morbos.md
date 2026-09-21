---
number: 79
title: The campaign starts on Morbos
date: 2026-09-21
area: decomp, port, content
files: crates/hb-formats/src/campaign.rs, crates/hb-formats/tests/against_the_game.rs, crates/hb-render/src/hud.rs, crates/hb-fly/src/main.rs, docs/formats/lvl.md
---

# 79. The campaign starts on Morbos

Toyz flew it and sent a screenshot with two things wrong in it.

## The reticle was behind the hill

[[70]] drew the reticle with the same triangle routine the world uses, which
means it went through the depth buffer. The reticle sits fifty units ahead of
an eye that is not the player's, so the first hillside inside fifty units
wins and the crosshair vanishes into the ground - which is exactly what it
looked like.

It is an overlay. It now rasterises straight into the frame with no depth
test and no shading, which is forty lines and no `Shade` at all.

## And we were starting on the wrong level

`hb-fly` defaulted to `hoth` and cycled alphabetically, because that is the
order `GAME.POD` holds them in and nothing in a `.LVL` says what comes next.

The order is the engine's, and it is right there: `0x482720` is what a new
game runs, and it copies twenty three level names into forty-byte slots at
`0x6106c0` while filling two parallel arrays - `0x60ff60` and `0x60fc30`.
The first five entries set those to (1,1), (1,2), (1,3), (2,1), (2,2), which
names them: a chapter and a mission within it.

```
1 Morbos    morbos morbos2 morbos3      5 Jurasic  jurasic jurasic2 jurasic3
2 Float     float float2                6 Roid     roid roid2 roid3 roid4
3 Iowah     iowah iowah2 iowah3         7 Hoth     hoth hoth2 hoth3
4 Kreash    kreash kreash2 kreash3      8 Ship     ship ship2
```

Twenty three, which is the twenty six in the archive less the three `NETLVL`
levels - those are network play and `netlvl.ini` lists them on their own.

So the game opens on Morbos, and `hoth`, which the port has been flying since
[[13]] because it was a convenient stem, is mission 7-1. `hb-fly` now starts
where the campaign does, cycles in its order, and prints which mission it is.

**Still unknown:** the rest of what `0x482720` sets up, which is a dozen more
globals beside the three arrays; whether a chapter is presented as a chapter
anywhere - the briefing screens are not read; and what picks the next level
after a win in the engine rather than in the port, which steps through the
table and is not the same thing as a `.LVL` naming its successor.
