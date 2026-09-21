# Hellbender

Reverse engineering Hellbender (Microsoft / Terminal Reality, 1996) and porting
it to Rust.

```
original/            symlink to the game disc - read only, never modified
docs/                the reference: formats, engine, content, port plan
worklog/             how each of those was worked out, newest last
crates/              the Rust port - hb-pod, hb-formats, hb-world,
                     hb-render, hb (tools), hb-fly (the window)
tools/               Python: pod.py, pe.py, mrgl.py, docs.py
work/                scratch - extracted archives, dumps. Not checked in.
```

Start with [docs/README.md](docs/README.md) for what is known, and
[WORKLOG.md](WORKLOG.md) for how it came to be known. The two are deliberately
separate: docs state what is true, the worklog records the evidence and the
dead ends.

## The state of it

Every file in both archives parses, or is one of two documented anomalies.

```
$ cargo run -p hb -- check
startup: 104 models, 187 palettes, 574 images, 0 levels, ...
game:    238 models, 101 palettes, 3317 images, 26 levels, ...
```

All 342 models walk their node stream to the byte, all 26 levels resolve every
file they name, and all 26 load their thirteen terrain grids. Models decode to
vertices, unit face normals and texel coordinates, and `hb view` draws them.

```
$ cargo run -p hb -- view startup:ship.bin /tmp/ship.png
ship.bin: 360 vertices, 490 polygons - 228 drawn, 262 back-facing
```

Terrain decodes to a wrapping 128 x 128 grid of 8.0-unit cells, each split into
two triangles whose diagonal alternates with the cell's parity, and `hb ground`
draws a whole level textured, palettised and shaded from above.

```
$ cargo run -p hb -- ground hoth /tmp/hoth.png
hoth: 205/205 textures resolved, 0 cells without one, palette hoth.act, ramp yes
```

`hb-fly` opens a window and flies through a level at 60 frames a second, in any
of the game's three screen sizes. `hb-fly --demo 1` replays the original game's
own recorded attract-mode flight through the port instead.

```
$ cargo run --release -p hb-fly -- hoth --mode 480 --scale 1
hoth: 205/205 textures, 640x480 screen
```

Arrows steer, `w`/`s` is the throttle, `x` stops, `a`/`d` strafes, `r`/`f`
climbs and dives, **space fires**, `c` toggles collision, `k` the cockpit, `m`
the music, `h` the readout, tab cycles the level, escape quits. Shoot a bunker
and it becomes its ruin, with its own destroy sound. It keeps you above the ground and above anything standing
on it, and it plays the level's ProTracker soundtrack.

The 7,606 objects placed across the 26 levels are drawn too, from the instance
list in each level's `.DEF`, and so are the chambers - the voids carved under
the terrain that the levels are actually flown through - the sky, and the
cockpit over the top of it, and the terrain's textures animate. Objects that
follow a course fly it - 1,301 of them across the levels - and anything can be
shot and destroyed. Nothing shoots back yet, and there are no missions; see
[docs/port/plan.md](docs/port/plan.md).

## Getting the data

The port reads the disc directly. Point `HB_GAME` at the directory holding
`system/GAME.POD`, or put a symlink at `original/`.

```
cargo test                           skips if the disc is not there
cargo run -p hb -- level float
cargo run -p hb -- terrain hoth
cargo run -p hb -- model cube.bin
cargo run -p hb -- png startup art/ckpt200.raw /tmp/cockpit.png
cargo run -p hb -- view startup:ship.bin /tmp/ship.png
cargo run -p hb -- heightmap float /tmp/float.png
cargo run -p hb -- ground jurasic /tmp/jurasic.png
cargo run -p hb -- fly hoth /tmp/frame.png 40 100 12288 60 4096
cargo run --release -p hb-fly -- hoth
cargo run --release -p hb-fly -- --demo 1
cargo run -p hb -- demo 1 38 /tmp/demo.png
```

## Tools

The Python tools under `tools/` are for exploration; the Rust crates are the
port. `tools/pe.py` is the one that matters most - `HELLBEND.EXE` kept its
diagnostic strings, so a string is usually one cross-reference away from the
routine that emits it, and almost every format here was read out of its loader
rather than guessed from the bytes.

```
tools/pe.py strings --grep 'ground'
tools/pe.py xref 0x005013d0        who mentions this address
tools/pe.py calls 0x00474360       who calls this function
tools/pe.py dis 0x00412d00 --len 400
tools/mrgl.py check work/game/MODELS
cairns new "What I found" --area format       the worklog
tools/docs.py check
```

## No game data here

This repository contains no game content: no art, no sound, no music, no
levels, no executable, and no extracted copies of any of them. Everything here
is either written from scratch or a description of how the shipped files are
laid out, worked out from the retail binary with the tools under `tools/`.

To run anything you need your own copy of Hellbender. Point `original` at it -
a symlink to the installed game or the mounted disc, so that
`original/system/GAME.POD` exists - or set `HB_GAME` to that directory. The
tests skip themselves when neither is there.

Hellbender is Microsoft's; the trademarks and the game's content belong to
their owners. This project is unaffiliated with Microsoft and with Terminal
Reality, and is not endorsed by either.

## Licence

The code and documentation in this repository are MIT licensed - see
[LICENSE](LICENSE). That covers this work only, not anything the game ships.
