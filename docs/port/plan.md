---
title: The Rust port
status: partial
covers: crates/
worklog: 7 to 20
---

# The Rust port

The goal is Hellbender playable on a modern machine from the original disc's
data, with no reimplementation of DirectDraw, DirectPlay or the 1996 sound
card matrix.

This page is the only place in `docs/` that describes intent rather than
observed fact. Everything else describes what the data and the binary do.

## Principles

- **The disc is read-only input.** Nothing is converted ahead of time and
  checked in. The port reads `GAME.POD` and `STARTUP.POD` directly, the way the
  engine does.
- **Bug-for-bug where it is visible.** The colour tables are authored, not
  computed, so the port uses them rather than recomputing shading in true
  colour. Where the original is wrong in a way a player can see, the port is
  wrong the same way.
- **No unverified constants in code.** If a field's meaning is a guess it keeps
  a numbered name. `Shape::guess` refuses `INFOBAR.RAW` rather than encode an
  inference.
- **Every documented claim gets a test.** `crates/hb-formats/tests/` checks the
  documentation against the shipped archives, and skips when the archives are
  not present.
- **No dependencies until one is unavoidable.** Every crate that reads, draws
  or decodes the game's data has none, so that half of the workspace builds
  offline and will keep building. `hb-fly` is the exception and the only one: a
  window, a keyboard and a sound device are not worth writing by hand, and it
  is the only file in the workspace that touches either API. The joystick is
  not among them: Linux's `js` device is eight bytes an event, so
  `hb-fly/src/stick.rs` reads it straight and every other platform gets the
  keyboard.

## Crates

```
hb-pod       the POD container                      done
hb-formats   act raw lvl terrain mrgl colour text   data layer done
hb-world     cell geometry and height queries       done
hb-render    the software rasteriser, plus the       draws a level
             level loader both binaries share
hb           the `hb` inspection tool, no deps       growing
hb-audio     .WAV decode, .MOD playback, no deps     music plays
hb-fly       a window, a keyboard, a joystick and    flies
             a speaker
```

```
hb-sim       the ship, the fight, the mission,       the game's rules
             the weapons, the doors, no deps
```

## Stages

**1. Data layer.** Done. Everything in both archives parses, or is a documented
anomaly. `hb check` is the gate.

**2. Look at it.** Mostly done. `hb png` renders a `.RAW` through its palette,
`hb view` renders a model flat shaded, and `hb heightmap` renders a level's
ground lit by the engine's own per-triangle normal. Between them they proved
the polygon node and the terrain right - the ship looks like a ship and FLOAT
looks like floating platforms. Still to do: the colour ramps as strips, models
to OBJ.

**3. The world, headless.** Done. `hb-world` has the cell geometry,
`heightAtGrid`, the box span query, the height at an arbitrary position
through the containing triangle's plane, the top of whatever is solid under a
position, and the boxes near one. The collision that uses them is
`hb_sim::collide` (worklogs 41, 43, 46).

**4. A picture.** Done. `hb ground` draws a level from above, `hb fly` draws one
frame from inside it, and `hb-fly` opens a window and flies through it at 60
frames a second in any of the game's three screen sizes, with the sky at the
level's own altitude, stars in the space levels, the placed objects, the
chambers, explosions, the shot and missile models, the HUD and the cockpit,
and it will not let you fly through the ground or a wall. Still to do: the
engine's own visibility scheme, which this port replaces with a back-to-front
walk and a depth buffer.

**5. Models.** Done for static geometry, and they are lit the engine's way
(worklog 36). The MRGL meshes draw at the position,
heading and scale each level's instance list gives them, each polygon with the
material that precedes it in the node stream or its flat colour. The `.TXT`
animated models parse and draw in their rest pose, so every placement in every
level is now drawable. Still to do: how a part is placed relative to its
parent, and playing the animation.

**6. Flight.** Done. `hb-sim`: the ship at the engine's own speeds, the
controls from `HELLBEND.INI`'s bindings, the swept step against `hb-world`
that holds it out of walls and inside tunnels (worklogs 41, 43, 46), and the
death sequence when the hull runs out (worklog 42).

**7. The game.** Most of it. `hb-sim` moves every placed object whose type
names a course, joining it at the nearest point as the engine's first logic
phase does, and anything can be shot: it takes hits against its hit points and
becomes its wreck, with its destroy sound. Turrets, SAM sites and the
dogfighting flyers shoot back (worklogs 28, 33). The mission runs from the
level's `.NAV` (worklog 34): the player starts where it says, the HUD names the
objective with its distance and an arrow, the voice lines play, and a level is
won through its jump zone or by finishing every objective, and lost against the
clock or by losing friendlies. Powerups lie about and drop from destroyed
things (worklog 35). The player's whole weapon system is there - the eight
weapons and their energy, the barrels the weapon energy buys, the dispersion
pattern, missiles and the lock, the MIRV's ten children and the Bion super
weapon (worklogs 37, 38, 44, 45). Doors and lifts open when shot and the
moving ground runs on its own (worklogs 51 to 53).

Still to do: the rest of the logic routines, the escort shuttle's route, what
comes between missions, the flyer classes 56, 59 and 60, the cluster missile,
the floating mine, and the cruise missile's own steering.

**8. The trimmings.** Music is done - `hb-audio` plays the `.MOD` files and the
level's track starts with the level - and the sound effects play: the guns,
the near misses, the explosions, the voice lines, the doors. Still to do:
where a sound is, which lives in the mixer and has not been read; Smacker
cutscenes through a decoder binding; the front end, demos and saves.

Multiplayer is out of scope until everything above works.

## Where the port departs on purpose

Two places.

**A level opens on a camera the engine does not have.** `0x481631` calls
`0x45a290` as a level starts, right after the opening movies, and `0x45a290` is
a single `ret` - in the disc's executable and in the December one both, byte
for byte. The hook is there and the animation was cut. The port fills it with
four seconds borrowed from the jump-out the engine does have: the eye two units
off the ship, `0x3f00` above it looking down and half a turn round, coming
level and back into the cockpit. The ship does not move while it runs, so
control begins exactly where `0x471333` puts it, and any key skips it.
`--no-entry` turns it off.

**Scenery is solid.** The engine lets the ship fly through a radar dish, a
reactor or a bunker - classes 0 and 9 - without a scratch, because its actor
test skips those two classes and its own collision is cells only. The port
gives each of them a box and pushes the ship out, **above ground only** - a
chamber is tunnels, and a box around the reactor standing in one fills the
passage. Everything else keeps the engine's answer, ramming included: a tank
still grinds you down rather than stopping you.

## Known problems

Reported from playing `hb-fly` and deliberately deferred:

- **Everything feels too big.** Two causes found. Worklog 30: the engine's
  view is 90 degrees down as well as across, where the port's was 64 down at
  320x200 - everything was drawn 1.6 times too tall - and the original showed
  its frame on a 4:3 monitor, which `hb-fly` now does too. Worklog 28: objects
  were drawn at a median one twentieth of their real size, because the port
  read a placement's hit points as its scale. With them drawn at their type's
  radius the terrain has things of the right size on it. And worklog 31: the
  port flew at up to 90 units a second on its own controls; the engine's flight
  model settles at 16, or 48 on the afterburner, which is what the recorded
  demo flies at.
- **Half the world was empty.** Also worklog 28, and not reported but surely
  seen: at a negative coordinate the terrain was drawn 1,024 units away.
- **The frame rate is poor.** Likely the debug build: `hb bench hoth` draws
  148 frames a second at 320x200 in release and 24 unoptimised. The workspace
  now builds the rasteriser, the mixer, the world and the formats optimised
  even in a dev build (114 a second), so a plain `cargo run -p hb-fly` is
  fast. At 640x480 release draws 50 to 80 a second.
- **The world did not look right.** Worklog 32: the engine draws ten cells
  each way and fogs everything out between 48 and 64 units, with textures
  dropping to half and quarter resolution with distance; the port drew five
  times as far with a thin fog and full-resolution textures to the horizon,
  which is where most of the shimmering came from. Worklog 29: every ground
  texture was mirrored and none were turned, the ground was lit flat per cell
  where the engine shades each grid point, box sides took the wrong one of
  each pair, and textures were mapped affinely. All four are now the engine's.
- **It looks crunchy.** Partly authentic: the default is the game's own 320x200
  upscaled by whole pixels, the palette is 8-bit with only 16 light levels, and
  the textures are 64x64 and point-sampled - `--mode 480` is far less blocky.
  Partly the port being cruder than the original: it interpolates textures
  affinely where the original has a `perspectiveFlag`, it does not dither where
  the original has `ditherFlag=1` to hide the banding between shade levels, and
  it never filters where the Direct3D path had `filterFlag`.

## What is still missing

The cell triangulation is read out of `groundTriangleMidpoint` and reproduced
in `hb-world`. The box faces, the orientation code and bit 8 of the ground
shading word were read out of the ground and box drawers in worklog 29.

What remains:

- The sky above the clouds and in space, and how a group model animates.

None of these block a first terrain render; all of them would make it wrong in
some detail.

## Running it

```
cargo test                      needs the disc; skips without it
cargo run -p hb -- check
cargo run -p hb -- level float
HB_GAME=/path/to/disc cargo run -p hb -- terrain hoth

cargo run --release -p hb-fly -- hoth --mode 480 --scale 1
```

`HB_GAME` points at the directory holding `system/GAME.POD`. It defaults to
`original/`, which in this repository is a symlink to the mounted disc.

A joystick is picked up from `/dev/input/js0` if one is there - but nothing
is read from it until an axis moves, because `js0` is whatever the kernel
numbered first and on a machine with a touchscreen and no stick that is the
touchscreen, whose axes sit wherever they were last touched. A stick also
only ever adds to the keys, never replaces them. `HB_JOYSTICK`
names another, `HB_JOY_X`, `HB_JOY_Y`, `HB_JOY_RUDDER` and `HB_JOY_THROTTLE`
say which axis is which - the throttle is ignored unless it is named, since a
pad's third axis is not a lever - and `HB_JOY_INVERT_Y=0` stops the y axis
being inverted. None of the engine's own calibration (`xStickMin`,
`joystickDeadZonePercent`, and the rest of `HELLBEND.INI`'s joystick block) is
transcribed; the device reports its own range and the dead zone is a flat 8%.
