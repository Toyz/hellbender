---
number: 72
title: The weapon has a picture
date: 2026-09-20
area: decomp, ui, port
files: crates/hb-render/src/hud.rs, crates/hb-fly/src/main.rs, crates/hb/src/main.rs, docs/engine/hud.md
---

# 72. The weapon has a picture

The top-left panel is 123 pixels wide at 320x200 and the port was writing
into the right third of it. The left two thirds are not empty in the game:
they hold a picture of the weapon you are holding.

`ART\VALK.RAW` is 64x64 and it is the Valkyrie cannon, drawn. There are
twelve of them, named at `0x501838` - `valk`, `d4s`, `skl`, `f6rfl1`,
`dom4s`, `c4s`, `vip4s`, `cls4s`, `m4s`, `mg4s`, `f6mine1`, `f6super` - and
the names follow the weapons once you have the table beside them: `dom4s` is
the Dead On Missile, `cls4s` the Cluster, `mg4s` the Guided MIRV, `d4s` the
Dispersion Cannon.

`0x501868` is the map, read with the weapon row as the index: row 23, the
Valkyrie you start with, takes picture 0; row 18 takes `dom4s`; row 28, the
floating mine, takes `f6mine1`. Rows with no picture of their own take 0,
which is the Valkyrie's, and `0x420e8e` treats anything outside 0 to 11 as
an error.

## Where it goes, and what covers it

`0x41f8fa` builds the box out of the view's size: 14, 3, 63 by 56 in the
640x480 everything else on the HUD is authored in. The message panel that
[[68]] found is 16, 3, 236 by 56 - the same strip, four times as wide - so a
message covers the picture and the weapon lines together. That is the whole
reason those lines hide behind `0x674d68`.

## Drawn the long way

`0x420e70` does not blit it. It clears the box, loads the picture as a
texture, sets the viewport to the box, builds four vertices at plus and
minus one, and draws them - the same viewport machinery the reticle ([[70]])
and the objective arrow ([[71]]) go through. That is how a 64x64 picture
fills a 63 by 56 box without anyone scaling anything by hand.

The port scales it in, which comes to the same picture in the same box.

**Still unknown:** nothing new here, but the same viewport question from
[[71]] is underneath all three of these - the reticle, the arrow and now the
picture are drawn through it, and how it turns a model at the camera's own
position into pixels has not been read.
