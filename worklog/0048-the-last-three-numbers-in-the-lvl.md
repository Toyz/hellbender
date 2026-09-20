---
number: 48
title: The last three numbers in the .LVL
date: 2026-09-20
area: decomp,format
files: crates/hb-formats/src/glt.rs,crates/hb-formats/src/lvl.rs,crates/hb-formats/tests/against_the_game.rs,docs/formats/lvl.md,docs/formats/scenery.md
---

# 48. The last three numbers in the .LVL

The `.LVL` page had been `partial` since worklog 3 over three numbers -
lines 22, 30 and 33 - and two filename slots nobody had opened, `.tdf` and
`.glt`. All five are now named, and the page is `solid`.

The level struct is at `0x666cb0` and the parser at `0x44c830` fills it line
by line, so every line has an address and every address has an xref list. That
is the whole method: read the offset out of the parser, ask who else touches
it.

**Line 22 is the fog colour.** The only reader is `0x48619f`, which runs when
the `.FOG` named on line 16 cannot be opened: `0x485ed0` takes the line as a
palette index, reads the three bytes at `0x5b3350 + 3*index` as the colour to
fade towards, and fills the last two rows of the ramp with the index itself -
which is why everything past the draw distance comes out one flat colour. The
table is then written back to `FOG\` for next time. It is 255 in 22 levels,
190 in `KREASH`, 130 in `JURASIC`, 0 in `HOTH` and `SHIP`.

**Line 30 is the altitude of the sky layer.** It goes to the sky setup as a
third argument beside the sky texture and its palette, and is stored at
`0x5055d4` shifted up fifteen - the same scale a terrain height byte is read
at, so 255 is 127.5 units, just under the sky plane at 128. Three places use
it: the object draw test at `0x42f4f0`, which draws nothing on the other side
of the layer from the eye; the powerup drop, which clamps a drop's altitude to
twice it; and the death code at `0x465541`, which only plays the wreck
animation when the ship is below half of it, and then only half the time.
`JURASIC` puts the layer at 65 units and `KREASH` at 95.

**Line 33 is read and never used.** `0x667074` has no xrefs at all.

**Line 10 is a tunnel definition file, and the game does not read it.**
`JURASIC.TDF` and `JURASIC3.TDF` are the only two with anything in them: a
count of one, then the level `artic-t1.lvl`, two world positions, and the
textures for the hole at each end - `icehole.raw` over `DBROWN.RAW`. No such
level ships. What the level sequence calls for this slot (`0x4624b0`) is ten
bytes long: it sets the tunnel count to zero and returns without looking at
the name. The writer that would produce the file (`0x4624c0`, "Unable to open
tunnel list") has no callers left. The count's only other reader is the
end-of-level tally, which skips its `Tunnels found: %d%%` line when it is
zero. A feature cut late, with its data left in the archive.

**Line 32 is the ground light table**, and chasing it filled out the `.GLT`
half of the scenery page. The loader `0x48c4c0` takes the name, cuts it at the
dot and appends `.glt`, and where line 32 is empty - as it is in most levels -
it uses line 9's name instead, so ten files serve all 26 levels. A record is
three texture names for a light lit, unlit and broken, then eight numbers; the
loader resolves the three names against the level's texture list and keeps
them as indices. At level load `0x48bd60` then walks the level's faces and
compares each one's texel against every record's lit and unlit index. That is
how the lights are found: nothing marks them but the texture they wear.

`hb_formats::glt` parses all ten tables - 90 records, every one in the long
form, every texture a `.raw`, every first number 2, 4, 6 or 10 units - and
`hb_formats::lvl` now calls its fields `fog_colour`, `sky_height`,
`ground_lights` and `unused_33`.

One more fell out along the way. Line 42 is `15.0, 30.0` in all 26 levels and
had been sitting under "weather parameters" with no use found; it goes to the
lightning spawner as the level loads (`0x49d220`), which sets the countdown to
the next strike to the first value plus a random amount up to the second. A
level with lightning flashes every 15 to 45 seconds, and never has more than
five strikes alive at once.

Still unread: what the eight numbers of a `.GLT` record set, and what actually
puts a light out - the textures for an unlit and a broken light are loaded and
indexed, but the code that swaps them has not been found.
