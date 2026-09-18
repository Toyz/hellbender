---
number: 34
title: The mission runs from the .NAV file
date: 2026-09-18
area: decomp,format,engine,port
files: crates/hb-formats/src/nav.rs,crates/hb-formats/tests/against_the_game.rs,crates/hb-sim/src/mission.rs,crates/hb-sim/tests/mission.rs,crates/hb-render/src/level.rs,crates/hb-fly/src/main.rs,crates/hb-fly/src/battle.rs,crates/hb/src/main.rs,docs/formats/level-text.md,docs/engine/simulation.md
---

# 34. The mission runs from the .NAV file

Until now a level had no goal: `hb-fly` dropped the player over the middle of
the map and left them there. Every level's `.NAV` is its mission, and the
engine's handling of it is compact enough to read whole.

## The file

The loader is `0x470bd0`. A record is a kind, a position, three optional
blocks recognised by a peeked first character - `!priority,time`,
`@completion sound` (and a completion text line the loader reads and drops),
`;proximity sound` - the objective text, and kind-specific data. The earlier
page had the right lines but not what they meant; it now has all fifteen
kinds, and `hb_formats::nav` parses every shipped file to its separator lines.
The census the parser gives matches one taken earlier with a throwaway
script: 179 destroy points, 61 checkpoints, 69 sync points, 47 starts (the
network levels have eight each), and one each of the rarer kinds.

Two things the files show. Only the "End of Navs" records are optional, so the
loader's coin toss - it drops two in three optional points of kind 9 or below
(`0x4712f6`, `rand() * 100 / 32767 > 33`) - never removes an objective. And the
loader's two passes that insert sync points around tunnels and jump zones find
nothing to do in any shipped file: the editor already wrote them, under the
same name, "Sync point: auto added".

## The frame

`0x471df0` runs once a frame from the game loop. Where the current point is
depends on its kind: a destroy point follows the first of its placements still
standing, a guardian follows its shields while they stand and the guardian
after, an escort or kill point follows its placement. From there, one shared
tail: the HUD arrow is `atan2(dx, dz)` less the player's heading, where
`d` is the player's position less the point's - it points from the objective to
the player, so the HUD must turn it half round - and the distance on the HUD
is measured across, without height.

The thresholds are all 16.16 floats compared as integers: 15 units for the
jump zone (`0x49700000`), 30 for a warp, 40 for checkpoints, tunnels and
beacons (`0x4a200000`), 60 for the HUD's near flag, 80 for the proximity sound
(`0x4aa00000`). Tunnels also test the sign of the player's height: under zero
to enter, over zero to leave.

A guardian holds its shields' hit points against it: every frame any shield
stands, the guardian's are reset to their starting value. Its point comes with
its own music module - `Fog-Boss.Mod` in `JURASIC3` - which the advance routine
swaps in, saving the level's name to restore when the guardian falls.

## Choosing the next point

`0x471770` is the subtle one. It looks for the next point after the current
that is not done; the end marker is skipped; and reaching a sync point that is
not done sends the choice back to the first required point still open. The
advance routine `0x4719d0` marks sync points done - the first one with nothing
required before it still open - so in a straight run through the list sync
points are passed without a stop, and they only bite when the player has
chosen to go ahead out of order.

A jump zone becoming current plays one of two lines by the parity of its
index: "Mission accomplished... Proceed to jump zone." or "Mission
complete...". The voice lines come from a phrase table at `0x505c20`, 36
bytes an entry, whose sound-and-subtitle items give the names; the sounds are
in `STARTUP.POD`.

## Winning and losing

The level ends through the jump zone (`0x5125c8`), or when every required point
before the end marker is done (`0x5125cc`) - which a level with a jump zone
never reaches, since the jump zone is never marked. It fails (`0x512720`) when a
timed point's clock runs out, with "20 seconds" and a spoken countdown from
ten on the way; when a third placement of a friendly type is destroyed
(`0x40d412`); and when the escorted shuttle is (`0x40d7ca`) - `0x424fc0` finds
it as the first friendly actor of class 50, and in `IOWAH2` and `IOWAH3` that
is `irshut.bin`. At first this last one looked like the player's death; it is
not, and what the player's death does is still not read.

## In the port

`hb_sim::mission` is the loader's additions and the frame, point for point,
behind a two-method `World` trait (where is placement `i`, and restore its hit
points). `hb-fly` starts the player at the mission's start, 16 units over the
surface and facing its heading, rather than over the middle of the map; puts the objective, its distance and
the clock on the HUD with an arrow, flashes the messages, plays the sounds and
the guardian music, drops beacons on B (`keyBeacon`, scancode 48), and after a
won mission moves to the next level, after a lost one starts it again.
`hb nav <level>` prints a mission.

Eleven tests cover the loader's additions, a play-through of destroy,
checkpoint, tunnel exit and jump zone, the arrow's convention, the countdown,
a guardian with shields, rescue beacons and the ten-beacon limit, and that
every shipped mission loads unchanged within 50 records.

Not done: message pods (no pods yet), the escort shuttle's own route - its
point completes when its hit points reach zero, which is presumably how it
leaves at the jump zone, but the class-50 routine is not read - the map
screen's markers, and choosing a point by hand (`keyNavChoose` is Tab, which
`hb-fly` uses for changing level).
