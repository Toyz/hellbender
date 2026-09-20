---
title: The simulation
status: partial
covers: HELLBEND.EXE logic phases, crates/hb-sim
worklog: 26, 27, 28, 31, 32, 33, 34, 35, 37, 38, 40, 41
---

# The simulation

What moves in a Hellbender level, and how much of it has been read. Most of it
has not; this page is the map of what is known so far.

## Actors follow courses

A type in a level's [`.DEF`](../formats/level-text.md) names a course on its
line 15, and every placement of that type follows it. Across the 26 levels,
1,560 placements belong to a type that names a course and 1,301 of those
courses exist - the other 259 are the dangling references described under
[courses](../formats/courses.md).

The engine's logic routines are named by their own diagnostics:

```
logicFollowPath                 logicFollowPathAttitude
logicFollowGroundPath           logicPathDogfight
logicTransportDisappear         logicTransportTakeoffLand
logicTransportTakeoffLandLeave
```

Each is a phase machine over the actor's `+0x64` field, and each checks its
course the same way before doing anything:

```
course = courses[actor+0x244]      "Bad course ID passed to ..." if negative
                                   "Bad course ID for enemy"   if missing
                                   "No course points for course" if empty
```

## Joining a course

Phase 0 of the follow logic at `0x00421240` sets a best distance of
`0x40000000`, walks every point of the course, and keeps the nearest. So an
actor **joins its course at whichever point is closest to where it stands**,
and does not have to be placed on it.

That explains a measurement that otherwise makes no sense: only 202 of the
1,301 course-following placements sit within one unit of their course, and the
median is 460 units away. They are meant to fly to it. The transport logic's
names - take off, land, leave - say the same thing: a transport starts on a pad
and goes to its route.

## What this port does

`hb-sim` reproduces phase 0 exactly and then follows the course point to point.
Everything past phase 0 is its own choice:

| choice | why |
| --- | --- |
| 20 units a second | The engine's speed source has not been read. A type's `.DEF` line 1 has a field that reads as 15 to 30 units a second in half the records and is zero in the other half. |
| Straight lines between points | The engine has a `"Curve parameter calculation failed"` diagnostic, so it probably fits curves. |
| A periodic course loops; a plain one stops at its last point | What the engine does at the end of a non-periodic course is not known. |
| All seven logic routines treated as "follow" | Only the follow logic's first phase has been read. Transports, dogfighting and attitude-following all behave alike here. |

The heading an actor faces is `atan2(dx, dz)` of its direction of travel, the
same convention the recorded demo flight measured.

## Behaviour classes

Every actor runs one routine a frame, chosen by its type's class - field 0 of
the `.DEF` record's first line, type offset 0x0c. The switch at `0x40bb93`
indexes a 65-entry jump table at `0x40c6cc` (type records are 664 bytes at
`[0x500748]`, the class at `+0x0c`). Most cases call a routine and then run the
same visibility test, `radius << 8 / distance >= 16`, into the actor's `+0x20`.

```
class   types   routine    what
    0     874   -          scenery: visibility only
    9     454   -          bunkers and domes: visibility only
   10     118   0x408c30   turret
   47      97   0x421240   course follower (phase 0 read, see above)
   25       0   0x40aa90   rises from the ground playing missile.wav; unused
```

The other classes' routines are listed by the table and not yet read.

An actor thinks only while it is **within 80 units of the eye on both x and
z**. The routine that runs a class (`0x40bb00`) has no range test itself, but
its caller, the actor loop at `0x406650`, first calls the draw-and-cull routine
`0x40da00`, and skips the update when that reports the actor out of range -
which `0x42f710` does past `0x500000` on either axis (`0x42f7b9`). Worklog 28
read only the inner routine and said every actor thinks every frame; worklog
32 corrected it. Enemy shots already in flight carry on regardless.

## The flyers

Classes 7 and 53 - 1,108 placements, the hornets, fighters and gunships -
run `0x4967b0`; 56, 59 and 60 run routines of their own that start the same
way (`0x497050`, `0x497be0`, `0x4999a0`) and are not read. The decisions:

```
phase 0      initialise, go to 200
phase 200    fly at the player (steering mode 0); within the turning reach
             sqrt((R + r + 2)^2 - r^2) - R the radius, r speed / turn rate -
             go to 2000 (0x49685b); within the retreat range go to 201
phase 2000   fly AWAY (mode 1 negates the target direction, 0x494853) at no
             less than twice the player's speed; past the attack range, 200
phase 201    the same with an eighth of the turn rate
phase 900    stop
2008 / 2009  break-off points 16 units left / right of the player, 2011 16
             above; phase 2012 flies to it until it stops closing, then 201
```

The situation that drives the break-offs comes from two cone tests -
`0x492750`, is the flyer within 30 degrees of the player's nose across and up
(1), ahead but outside (2), or behind (-2); `0x492940` the same the other way
round - and whether the two fly the same way, headings and pitches within 30
degrees:

```
both in each other's sights        head-on: under 32 units, go to 201
same way, player in my sights       chasing
same way, me in the player's        on my tail: break - up if level within 30
                                     degrees, else toward the lower wing
player behind me / ahead unaimed    ahead: match the player's speed in range
```

A flyer fires through the turrets' `0x407770` whenever the player is within
its attack range and it is aimed (`+0x190`), at **twice its own speed**
(`0x496f86` sets the type's shot speed to double the flyer's for the call).
The attack and retreat ranges are the `.DEF`'s `!NewAtakRet` line, whole
units - 30 to 50 and 8 to 32 across the shipped types.

What it adds up to is a strafing run: in at the player, two or three shots,
past, out to the attack range at twice the player's speed, round, and in again.

The steering itself, `0x4944c0`, is not read beyond its outline: it predicts
the next position from the velocity and lifts the target above the ground by a
clearance (`type+0x98`) when that would be underground, takes the target's
direction as a heading (`atan2(dx, dz)`) and a pitch, negates it for mode 1,
and integrates a rigid body in x87. `hb_sim::flyer` turns heading and pitch
toward the target at the type's turn rate (65,536 is a turn a second) and flies
along its nose.

## Time

Frame time is `0x59d14c`, 16.16 seconds. `0x46f160` reads `timeGetTime` and
scales milliseconds by 1179.648, and `0x481cd7` divides the difference by 18 -
65.536 a millisecond, exactly 16.16 - then caps it at `0x4000`, a quarter of a
second, and scales it by a game-speed factor at `0x50f580`.

## The turret

Class 10 (`0x408c30`), each frame:

1. **Aim with lead.** The time a shot takes to reach the player is the 3D
   distance over the type's shot speed (`+0x218`); the aim point is the
   player's position plus the player's velocity (`0x5b3a00`, `0x5b3a08`)
   times that time, horizontally only. The wanted heading is
   `atan2(lead_x, lead_z)`; the wanted pitch is 0.
2. **Turn.** `0x4068f0` eases the actor's angles toward the wanted ones by
   `error * turn_rate * dt` (`+0x68`) - exponential, not a fixed rate. The same
   routine eases position by `move_rate * dt` (`+0x64`), but a turret asks to
   stay where it is.
3. **Fire.** Frame time accumulates at actor `+0x68`; past the fire interval
   (`+0x6c`) the interval is subtracted and the turret fires - through
   `0x4074e0` for weapon 19, `0x406dc0` otherwise.

`0x406dc0`, the straight shot:

- Steps the barrel counter at actor `+0xac` for barrel modes 1 and 2, then
  offsets pitch and heading by that barrel's entry in the tables at
  `0x500760` and `0x500778`.
- Builds the shot direction from the actor's angles plus the offset, and
  **returns without firing if the player is behind it** (`0x406fc6`).
- Picks a muzzle at random from the type's line-3 list and places it with the
  actor's rotation; a muzzle vertex at the model's origin fires nothing. With
  no list, the shot leaves the actor's origin.
- Spawns into the shot pool with the type's damage (`+0x70`), shot speed
  (`+0x218`) and weapon kind (`+0xdc`), and plays line 12's sound - `null`
  everywhere.

A shot's direction does not converge on the player from the muzzle: it is the
barrel's direction, from wherever the muzzle is. A gun whose muzzles are high
on its model fires over a player at its own height. On `HOTH`'s spike gun, 17
units off, a player at the spike tips' height takes 13 shots in a minute and
one at the gun's own height takes 3 (`a_hoth_spike_gun_hits_at_its_muzzles_height_and_not_below`).

## Straight shots

A pool of 256 at `0x614570`, 104 bytes a slot; the player's shots take slots
0-127 and everyone else's 128-255. The spawner (`0x476780`) records damage,
speed, side and kind, and gives every shot **two seconds** (`0x20000` at
`+0x04`). A slot with speed zero is free.

The update (`0x476ac0`) moves each shot along its direction in **eight equal
sub-steps a frame** and tests after each: a point test, not a swept one. It
stops at solid ground below or a ceiling above (`0x41c300`, `0x41c4d0`).

- A **player's** shot tests every actor with hit points above zero
  (`0x40d750`). An actor is hit if the point is inside one of its type's
  line-5 hit spheres - each really a cube, tested axis by axis - or, when it
  lists none, inside the model's bounding box turned to the actor
  (`0x40ce70`). The box is filled from the model at `0x404d40`.
- An **enemy's** shot also tests actors, and then the player: inside 2 units
  on every axis of the ship (`0x465650`).

A hit takes the shot's damage times the target type's multiplier for the
weapon kind (`0x40d2b0`): kinds 1-3 use line 18 (laser), 18, 19 and 24-28 line
19 (missile), 23 line 17 (cannon), and anything else none. At zero the actor
is destroyed (`0x40cb00`); a friendly one counts toward the three friendly
kills that set `0x512720`.

An enemy shot that comes within 16 units of the player while closing plays a
sound once (`0x4768a0`): `missile.wav` for kind 17, else the weapon table's
sound for its kind, else `whiz3.wav`.

## The weapon table

68-byte rows at `0x50e7a0`, speed first then damage, both 16.16:

```
kind   speed   damage   sound
   0    32.0   0.0625
   1    32.0   0.0625   laser4.wav
   2    24.0   0.125    laser3.wav
   3    64.0   0.0625   laser5.wav
 4-8    32.0   0.25
18,19   64.0   1.0      missile.wav, missl-2.wav
  23   128.0   0.125    m-gun-r.wav
24,25   64.0   1.0      missl-1.wav, missl-3.wav
```

Rows 9-17 and 20-22 have speed zero. A row is 68 bytes that begin 32 bytes
before the speed: a word saying how its shots are drawn - 0 a laser, 1 a
missile, 2 not at all - a 16-byte model name, an 8-byte HUD code
(`VAL`, `SKL`, `DIS`, `RFL`, `HAM`, `VIP`, `SCR`, `LGN`...), then speed,
damage, a flag the next-weapon key stops on (`+8`), volleys a second
(`+0xc`), the fire sound (`+0x10`), and two words. The names are a table of
their own at `0x50e700`. The player fires with the row's speed **plus the
ship's speed** - `0x47d57a` takes the magnitude of the velocity at `0x5b3a00`
- and the row's damage.

## The player's weapons

The player starts on weapon 23, the Valkyrie Cannon (`0x426ebf`), with the
servo-kinetic laser (1) and the cannon unlimited, 20 Dead-On missiles (18),
5 Vipers (19) and 2 cruise missiles (24); stocks are the 32 eight-byte slots
at `0x61bce0`, -1 for unlimited.

**The trigger** (`0x47db11`) is a rate accumulator at `0x613c84`: while fire
is held it gains the row's volleys a second times the frame time, and each
time it passes 1.0 the fire routine `0x47d520` fires one volley; let go, it
sits at 1.0, so a press fires on its first frame. The lasers and the cannon
fire six volleys a second, the dispersion cannon two, missiles one. After a
volley the stock drops by one; at zero the next weapon with a stock is taken.

**Weapon energy** (`0x62d678`, half at the start) sets how many barrels the
guns use and pays for them: at zero one barrel, alternating three quarters
of a unit either side and half a unit down, free; up to half two barrels,
half a unit either side, a 256th a volley; above half four, at the corners
of a unit square, two 256ths (`0x479ce0` for the cannon, `0x47a5d0` for the
lasers, which also sit half a unit further forward). Each shot starts one
frame's flight behind its barrel. Below 0.1 a line warns "Weapon energy low".

**The dispersion cannon** (`0x47ab60`) fires from the ship's centre in a plus
of five directions 1,024 of the circle apart (`0x50f040`, `0x50f058`), and
fires straight back as well: one pair with one barrel, four ahead and two
behind with two, nine scattered at random within 2,048 and two behind with
four.

**Main energy** (`0x62d63c`) goes to weapons (`0x465ba0`) or the shield
(`0x465a70`) an eighth at a keypress - `keyTransferWeapon` and
`keyTransferShields`, `,` and `.` - with what does not fit handed back.
Every frame (`0x464d0f`) main energy and the hull each gain 0x48 a second.

**The afterburner** is weapon 22 run through the same accumulator while its
key is held (`0x47db3f`): each "volley" burns `0x1000 / 6` of the tank
(`0x61bd90`), a sixteenth a second, sixteen seconds from full. The tank refills
at a thirty-second a second while there is weapon energy to pay 0xda a
second for it; an empty tank waits five seconds for a thirty-second.

The weapon keys are `keyVulcanCannon` (the backquote) for 23 and 1 to 9 for
2, 1, 3, 18, 24, 19, 25, 26 and 27 (`0x5127b0` to `0x5127d4`); `keyMine`, 0,
is the floating mine (28); a key for an empty
dispersion cannon or rapid-fire laser says it is "not in arsenal".
`keySelectNextWeapon` (`=`, `0x479ca0`) steps round the weapons with a stock
whose row flag allows it.

**A shot is drawn as a model** (`0x4769cf`), at size 1.0, from the table the
weapon rows' names are loaded into (`0x61bc60`, `0x476473`): turned by its own
angles, or held facing the eye for kinds 2, 4 and 9-16 (the fireballs, the
balls and the bosses' weapons). The Valkyrie Cannon has no shot model; it
cycles `muzzle.bin`, `muzzle2.bin` and `muzzle3.bin`, one a draw
(`0x476a44`). The laser models are quads with a texture that has holes, like
the powerups'; the missiles are solid models.

**Missiles** (`0x47cd70`) leave from under one wing or the other in turn
(`0x50f088`) - half a unit ahead, a unit to the side and a unit down - along
the nose at the ship's speed, into the guided missiles' pool through
`0x477890`: the same 16 slots, acceleration, steering and six-second life as
the SAM sites' (ten seconds for the cruise missile), with the row's damage,
1.0. The record's `+0xc` is its owner, 0 for the player, and `+0x10` its
target: `0x477f20` steers a player's missile at that placement, an enemy's at
the player, and one with target -1 along its own heading. A player's
missile loses its target when the placement dies or is cloaked (`+0x14c`).
The Dead-On (18) and the MIRV (26) launch with no target; the Viper (19),
the cruise missile (24) and the super weapon (30) launch at the lock.

**The lock** (`0x50e6fc`, `0x47b700`): `keyMissileLock`, V, steps it to the
next placement the selected weapon can lock; with nothing locked the first
that can be is taken each frame; a lock that no longer passes is dropped.
`0x47bbd0` decides: the placement ran this frame (`+0x84`: alive and in the
80-unit box), is not cloaked, friendly, dying or class 33, is on screen
(`0x42f710` with no radius: in front and inside 90 degrees each way), and is
of the weapon's kind - the Viper takes the classes `0x40dca0` lists as
flying (2, 4, 7, 8, 16-18, 25, 26, 28, 29, 32, 35, 38-40, 43, 44, 46, 48-60,
63, 64), the cruise missile the rest (`0x40dc00`), the cluster missile and
super weapon anything.

**The two that break up.** A MIRV (26) or a guided MIRV (27) checks its own
clock every frame - the same one the steering's gain runs on - and a second in
it bursts (`0x477b90`, `0x477d10`): ten missiles from where it is, at half its
speed, pitched and headed at random, and a blast of 32 units and half a hit
through `0x47d3b0`. A MIRV's ten are Dead-Ons with nothing to home on; a
guided MIRV's are Vipers, and the routine walks the actors to give each one a
target of its own.

`0x47d3b0` is the blast the game uses for splash: every actor the loop ran
this frame whose position is within the reach on **all three axes** - a cube,
not a ball - takes the damage through the same entry a shot does.

**The super weapon** (30), which the eight Bion pieces make and select, is a
missile that scorches as it flies: every sub-step `0x477a19` puts a quarter of
its damage into everything within 16 units of where it is. It does not need to
hit anything.

The cluster missile (`0x47cec0`) and the floating mine (`0x47d82a`, into a
hundred-slot pool at `0x61bde0`) are not ported. Nor is the cruise missile's
own steering (`0x4780b0`): the port steers it as the others.

## The guided missile

Weapon 19, the SAM sites' (`0x4074e0`). No in-front test: it launches from a
random muzzle along the site's current heading into a second pool, 16 slots of
88 bytes at `0x613700` (`0x477890`).

- It starts at the type's move rate plus one - effectively at rest - and gains
  16 units a second every second (`0x478673`) up to 64 (`0x400000`).
- It lives **six seconds** (`0x60000`; ten for kind 24) and does **0.25**
  damage (`0x4000`).
- It steers in **four sub-steps a frame** (`0x478670`) toward the player's
  heading and pitch from it (`0x477f20`). The gain on the error rises from 0
  to 4.0 over its first second and holds 4.0 to a second and a half; **after
  that it points straight at the player every sub-step**.
- A smoke puff every sixteenth of a second (`0x478e35`).

## The player's flight

`0x463aa0`, once a frame, with the pose at `0x5b3830` - position, then
pitch, roll and heading at `+0x0c`, `+0x10`, `+0x14`, then a 3x3 matrix at
`+0x1c`:

```
keys      each of up, down, left, right, roll left, roll right ramps an input:
          held, +2 x frame time up to 1.0; released, -4 x frame time to 0
          (0x464a8e). upKey=72, downKey=80, leftKey=75, rightKey=77,
          rollLeftKey=71, rollRightKey=73 in HELLBEND.INI.
throttle  0 to 1.0 at 0x512588; throttleUpKey (X) adds the frame time,
          throttleDownKey (Z) takes it away (0x464367).
damping   the pitch, roll and yaw rates (0x50cc54, 0x50cc58, 0x50cc5c) and
          the local velocity (0x50cc48, 0x50cc4c, 0x50cc50) are halved.
input     pitch rate += (up - down) / 7        (0x2492)
          yaw rate   += (right - left) / 7
          roll rate  -= (roll right - roll left) / 7 + 0.73 x yaw input
                                              (0xbb80: turning banks)
level     with autoLevel=1: roll rate -= clamp(roll / 7 / frame time,
          +-0xc30) x (1 - sin^2 pitch). Past a quarter turn the roll has half
          a turn taken off it unwrapped, so rolled left it levels upside down
          and rolled right it rolls back upright.
thrust    forward velocity += 8.0 x throttle, or 24.0 on the afterburner
          (weapon 22 selected), or 72.0 with a further flag.
turn      the rates times the frame time, in the 16-bit circle, turn the ship
          about its own axes (0x4631f0 builds the rotation, 0x4632f0 composes
          it, 0x463580 takes the angles back out).
move      velocity = the pose's matrix times the local velocity; position +=
          velocity x frame time.
```

Halving and adding every frame settles at twice what is added, at any frame
rate: **16 units a second at full throttle, 48 on the afterburner**, and **2/7
of a turn a second on a held key** - 18,724 in the 16-bit circle, 103 degrees.
The recorded demo agrees: it flies at a median 16.5 units a second with a 90th
percentile of 49, turns at a 90th percentile of 18,002 a second and pitches
at a 99th percentile of 18,734; and in its right turns the roll is negative 77
per cent of the time, in its left turns positive 87 per cent - the bank the
0.73 couples in.

There is no strafe and no vertical thrust: nothing but a reset writes the side
and vertical velocities. Up dives: the up key raises the pitch angle, and a
positive pitch is nose down. That sign is argued rather than read - it is the
one that makes the yaw's "right turns right" hold under the same composition -
and it is the flight-sim convention.

`hb_sim::flight::Ship` transcribes it; its tests hold the settled speeds, the
turn rate, frame-rate independence, the signs, and auto-level.

## The player

Health at `0x5b39ec`, full at `0xffff`; shield at `0x62d6e0`, starting at 0.5
(`0x426f47`). A hit (`0x4653a0`):

- At difficulty 0 the damage is halved, plus one. The default is 1
  (`0x512628`).
- Scaled by `1 - shield / 2`, so the starting shield takes a quarter off.
- The shield loses 1/32, and a voice warns as health passes 0.1 and as the
  shield passes 0.1.
- One of `exp1.wav`-`exp5.wav` plays (`0x476ef0`), and a one-second timer at
  `0x5b36a4` shakes the view.
- At zero health the ship is destroyed.

## Shooting in this port

`hb-sim` transcribes all of the above for straight shots, turrets, guided
missiles and the player's health and shield. What is its own:

| choice | why |
| --- | --- |
| Enemy shots do not hit other actors | The engine's do. A muzzle can sit inside its own turret's box, and the engine's exclusion, if any, is not read. |
| The level starts again five seconds after the ship blows up | The engine ends the level there; what it shows between missions is not read. |
| No view shake | The shake's amounts are not read. |
| Shots and missiles are points | The engine draws models by kind (`0x4769cf`). |
| A group model is its first child | Only the SAM site uses one. The engine's frame advance is not read. |
| Turrets and flyers only among the 65 classes shoot | The rest are not read. Classes 56, 59 and 60 borrow the class-53 routine. |
| A flyer steers by turning heading and pitch at its turn rate | The engine's `0x4944c0` is an x87 rigid-body integrator not yet read. |

The fire button is the space bar, because `HELLBEND.INI` binds `fireKey=57`,
which is the space bar's scan code - and 636 of the 638 key presses in the
recorded demos are it.

## The mission

A level's `.NAV` list ([format](../formats/level-text.md#nav---the-mission))
is worked through one point at a time; the current one's index is
`0x6283bc`. Each frame the game loop calls `0x471df0` (from `0x4823d4`),
which finds where the current point is, points the HUD at it, and checks
whether it is done.

**Where it is.** By kind (`0x472a34`):

- destroy (0): the first listed placement with hit points above zero and not
  in the flyers' dying phase `0x12d`. With none left the point is done.
- guardian (5): `0x4720d9`. While any listed shield placement stands, the
  guardian's hit points are put back to their starting value (actor `+0x24`)
  every frame and the arrow points at the last shield standing; otherwise at
  the guardian. The HUD line is `Guardian: %d%%` of whichever it watches. When
  the guardian falls: "Guardian Destroyed", the level's music back, an
  explosion at it, and the point is done.
- sync (7): done at once.
- escort (12), kill (14): the placement's position.
- drop beacon (8): done when a player beacon lies within 8 units on x and z
  (wrapped) and on the same side of y = 0.
- message pod (13): the pod's (`0x66fd60`, 24-byte records).
- the rest: the point's own position.

**The HUD.** From the player's position less the point's, wrapped: the arrow
`0x59d118` is `atan2(dx, dz)` in the 16-bit circle less the player's heading -
it runs from the point to the player - and the readout `0x6283c0` is the
distance across, `sqrt(dx^2 + dz^2)`, height left out. `0x59d100` is 0x1f
inside 60 units, 0x10 outside. The label `0x625110` is "Destroy Target",
"Enter Tunnel", "Fly to Checkpoint", "Fly to Jump Zone", "Exit Tunnel",
"Escort Ship" or "Pick up Message Pod" by kind; the others leave it alone.

**Done**, all against that distance across (`0x4724a0`):

| Kind | When | Then |
|---|---|---|
| jump zone (3) | under 15 units, and the player between the point's height and 20 above it | the level ends (`0x5125c8`) |
| enter tunnel (1) | under 40, player below y = 0 | "Enter Tunnel" |
| checkpoint (2) | under 40 | "Checkpoint" |
| exit tunnel (4) | under 40, player above y = 0 | "Exit Tunnel" |
| warp (10) | under 30 | the player is moved to its destination at the surface plus 16; the point is not advanced |
| beacon (11) | under 40, and under 20 in height | "Objective complete" |
| escort (12) | the placement's hit points at or below zero | |
| message pod (13) | the pod collected (`+0x68`) | |
| kill (14) | the placement's hit points at or below zero | not advanced: the level ends (`0x5125c8`) if the player's hull (`0x5b39ec`) is above zero |

Done plays the point's completion sound and calls `0x4719d0`. Under 80 units
the proximity sound plays once.

**Advancing** (`0x4719d0`): the point is marked done (a beacon is not), its
clock stopped, and the first sync point that no required point before it
holds up is marked done too. Then `0x471770` chooses the next point: the one
after the current that is not done, skipping the end marker, where reaching an
unfinished sync point sends the choice back to the first required point still
open. It will not leave a point whose clock is running. A guardian coming up
switches to its music, plays `warning.wav` at the player and flashes "Mission
Goal Ahead!"; a jump zone plays "Mission accomplished... Proceed to jump zone."
or "Mission complete..." by whether its index is odd.

**The level is won** when every required point before the end marker is done
(`0x5125cc`); a level with a jump zone never gets there, and is left through
it instead. **It is lost** (`0x512720`) when a point's clock runs out - it
says "20 seconds" at 20 and counts down aloud from 10 - when the third
placement of a friendly type (`+0x254`) is destroyed (`0x40d412`), or when a
shot destroys the friendly class-50 actor - the escorted shuttle, which
`0x424fc0` finds as the first actor of class 50 with the friendly flag
(`0x40d7ca`). The flag also chooses the level's death movie afterwards
(`0x45baf6`).

**Beacons** (key `keyBeacon`, B): a point of kind 11 at the player, appended
to the list; with ten out, the oldest is replaced. "Beacon launched".

The voice lines are entries in a phrase table at `0x505c20`, 36 bytes each:
a count, then items whose high word is a kind - 2 is a sound and a subtitle,
the sound's name at `+0x14` and the text at `+0x1c`. `0x3c` is `objcomp.wav`,
"Objective complete"; `0x57`/`0x58` the jump-zone lines; `0x5e` "Beacon
launched."; `0xa1`-`0xaa` the countdown from `10-sec.wav` to `1.wav`. The
mission's sounds are in `STARTUP.POD`.

`hb_sim::mission` is this, point for point. What it leaves out: the map
screen's markers (`0x410c60` marks them), the flash `0x4062f0` starts, the
guardian's death explosion, the network kind 15,
and the manual choice of point (`keyNavChoose`), which hb-fly's Tab already
uses for changing level. hb-fly's floor for the loader's height check is the
top of the solid, so a point below y = 0 is left where it is rather than put
on the tunnel floor.

## Being shot down

With the hull at zero the player's update stops flying and runs `0x464870`.
The velocity is dropped; the nose pitches down and the ship rolls, both a
quarter of a turn a second, the pitch stopping at `0x1fff` - forty-five
degrees; and the ship drifts forward at `0x7a120`, seven and a half units a
second. Within two units of the ground it explodes - the eleven puffs at two
units - and the loadout goes back to what it started with. From then
`0x512618` gathers the frame time, and the game loop ends the level once it
passes five seconds (`0x4822a4`). `hb_sim::death` is that; what the port does
at the end of it - start the level again - is its own choice, since what the
engine does with a failed mission is not read.

## The ship against the world

`0x427280` is the ship's collision, and it runs on the position the flight
model just produced, with the one before it. It grows a box a unit each way
(`0x10000`) around the pair and asks every cell that box spans for the
surfaces in it: `0x4279b0` walks the cells between the two corners and
`0x428790` collects them, the ground's two triangles and the boxes' faces
above the ground, the chamber's below it, chosen by a mask.

For each surface the position is behind, `0x4277c0` puts it back: the signed
distance from the plane, `dot(normal, position - a point on it)`, wrapped on x
and z like every other distance in the world, times `0x103e8` - a hundredth
over - pushed back along the normal. Position only: the flight model's
velocity is not touched, so the ship slides along what it hits rather than
stopping dead.

Below zero the surfaces are the chamber's. A chamber is two heightfields of
its own - a floor and a ceiling, both signed and both under the ground - and
where they meet there is rock: `0x4290f0` reads each at the cell's four
corners and counts the corners where they are equal. `HOTH` has 5,414 chamber
cells, about a hundred units down.

The port keeps the ground with its height query and does the boxes as solids:
`hb_sim::collide` pushes the ship, a sphere of the engine's one unit, out of
the face it is least far through, with the same fraction over. Underground it
holds the ship between the chamber's floor and ceiling, sampled across the
ship's own unit, and backs it out the way it came when those two meet - the
crude form of the engine's wall. What it does not have is the engine's swept
test between the old and new positions, so a fast enough ship can still pass
through a thin box in one frame.

## Explosions

`0x47f3f0` makes one out of eleven puffs into the effect pool at `0x612230`,
sixteen slots of 32 bytes: ten of twice its size scattered within it, and one
of four times at the centre (`0x476f90` takes the first free slot, or writes
over the first). A puff keeps a position, a size, a clock and a rate - a
quarter to three quarters of real time, drawn at random - and a two-second
life it never reaches, because `0x4771e0` drops it once its clock passes
sixteen frames of a sixteenth of a second each. It is drawn as a square facing
the eye, textured `blast1.raw` to `blast16.raw`, its corners a size out.

The player's death is one of these two units across, and then the loadout goes
back to what it started with (`0x465560`, `0x464850`, `0x464998`, which are
being shot down, flying into the ground and flying into a ceiling). What the
engine does for a destroyed actor is a different path - `0x40cb3c` spawns an
actor of its own (`0x40c7d0`) - and is not read; the port uses the puffs
there too, at the type's radius.

## Powerups

They live in an array at `0x66fd60`, 24 bytes each, at most 299: the
position, the kind (bit 31 set once taken), the pickup size, and a timer the
network game uses to put a taken one back after 120 seconds. They come from
two places: the level's `.PUP` (`0x426540`) and destroyed actors. The destroy
routine (`0x40cc3c`) rolls `rand() * 100 / 32767` and drops a powerup when the
type's chance (`.DEF` line 2, `+0xe8`) is at least the roll - a chance of 0
never drops - of the type's kind (`+0xec`), or of any of the 31 when that is
-1 (`rand() * 31 / 32767`). The weapon bunkers (`wbunker.bin`) drop theirs
every time.

A powerup is put down (`0x426f80`) at least twice its size above the floor
under it, below the ceiling over it less the same, and under twice
`0x5055d4`. Its size is its model's radius at the model's own unit (see
[the model format](../formats/mrgl.md#0x14-mesh-start)): 2.6 units for the
`f6*` family. Each frame (`0x426cf0`) the player inside that size on every
axis is offered it (`0x426760`), which applies it or refuses it; the rest are
drawn turned by two of the view's angles (`0x4266c0` passes `0x5b37c8` and
`0x5b37c4`), so their one quad faces the eye.

The kinds, by the engine's own names (`0x502308`), and what picking one up
does. Stocks are slots in an array of 32 at `0x61bce0`, eight bytes each.

| Kind | Name | Effect | Line |
|---|---|---|---|
| 0 | Rapid-Fire Lasers | slot 3 +100 | "RFL secured" |
| 2 | Dispersion Cannon 14 | slot 2 +100 | "Dispersion Cannon secured" |
| 3 | Dead-On Missiles | slot 18 +20 | "Sledgehammer Rockets secured" |
| 4, 5 | Vipers, Bion Fury | slot 19 +20 | "Viper Missiles secured" |
| 7 | Cloak (Multi only) | none alone | message "Cloaking Device" |
| 9 | Guided MIRV | slot 27 +1 | "Hellion Missiles secured" |
| 10 | Turbo Thrust | afterburner fuel full (slot 22) | message "Afterburner Fuel" |
| 11 | Energy Can | hull +25%, refused at 0xfffa | "Hull Repaired" / "fully repaired" |
| 12 | Cruise | slot 24 +5 | "Scorcher Missiles secured" |
| 13 | Cluster | slot 25 +10 | "Legion Missiles secured" |
| 14 | MIRV | slot 26 +1 | "Independence Missile secured" |
| 15 | MINE | slot 28 +5 | "Doomsday Mines secured" |
| 16, 17 | Damage 25%, 50% | hull +25% / +50%, refused over 0xea60 | "Hull partially repaired." / "fully restored." |
| 18 | Damage 100% | hull full, refused over 0xea60 | "Hull fully restored." |
| 19, 20 | Energy 25%, 50% | main energy +25% / +50%, refused when full | "Energy boost secured." |
| 21 | Energy 100% | main energy full | "Main energy at 100%" |
| 22 | Message Pod | the mission's pod point is done (`0x426deb`) | "Message pod retrieved." |
| 23-30 | Super Weapon Piece 1-8 | a bit in `0x62d634`; all eight make weapon 30 and select it | "Bion technology captured.", then "Weapon complete..." |
| 1, 6, 8 | not used | taken, nothing | |

The names lag the effects in places - kind 11 is labelled an energy can and
repairs the hull. A refusal says "Hull undamaged. Repairs not required." or
"Energy not required." and leaves the powerup where it is; the engine says it
every frame the player stays inside, the port once. The hull is `0x5b39ec`
(full `0xffff`) and main energy `0x62d63c`, which starts at half, as the
shield `0x62d6e0` does (`0x426e90`); energy past full is capped with "Main
energy at maximum capacity." (`0x4659f0`). The single-player loadout is 20 in
slot 18, 5 in 19, 2 in 24, slots 1 and 23 unlimited, weapon 23 selected.

`hb_sim::powerup` is this. The port keeps the stocks and the energy but has no
weapons to spend them on yet.

## Unknown

Everything past phase 0 of the course follower: speeds, curve fitting, what
happens at the end of a course, how the seven logic routines differ. The other
63 behaviour classes, including the flyers (53, 56, 60) that make up most of
what moves. The player's rate of fire. What happens at death, and what follows
a won or lost mission. How the ship collides with the ground and boxes.
How the engine picks which shot model to draw. Line 2 of the type record and
line 7's first two values.
