---
number: 102
title: The port without the executable
date: 2026-09-21
area: port, format, tooling
files: crates/hb-formats/src/hud_font.rs, crates/hb-formats/tests/against_the_game.rs, crates/hb/src/main.rs, crates/hb-fly/src/main.rs, docs/formats/font.md
---

# 102. The port without the executable

"Can we extract the game fonts so we don't need the exe at all."

The answer turned out to be smaller than the question. Grepping every crate for
`HELLBEND.EXE` finds it in fourteen places, and thirteen of them are comments -
`0x47442c` for the MRGL size table, `0x463aa0` for the flight model, and so on,
all of them read once and transcribed. Exactly one is a file read at run time:
the HUD font at `0x50f530`, which [[67]] found and which is the only thing in
the image the port still needs the image for. The front end's font is already a
pair of files in `STARTUP.POD`.

So the fix is one table.

`HudFont::read` now slices the 12,288 bytes out of the PE and hands them to a
new `HudFont::parse`, which is the same reader over a bare table, and
`HudFont::table` writes one back. `hb hudfont hudfont.bin --extract` is the
command. Both binaries look for `hudfont.bin` beside the archives *before* they
look for the executable, so the file is enough on its own.

Proved by doing it: a directory holding nothing but `system/GAME.POD`,
`system/STARTUP.POD` and `hudfont.bin` renders a specimen sheet byte-identical
to the one made from the executable, and `hb-fly` runs against it without the
line it prints when it has no font. A round-trip test in
`against_the_game.rs` checks every one of the 256 characters, width and
pixels, through the table and back.

None of the table is in this repository and none of it needs to be. It comes
off the player's disc, the same as everything else.

**Still unknown:** nothing here. The question was whether anything else in the
image is read at run time, and nothing is.
