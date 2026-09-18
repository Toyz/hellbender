---
title: The level text files - .DEF, .NAV, .TXT, .TEX, .ANI, .LVL family
status: partial
covers: DATA\*.DEF, DATA\*.NAV, DATA\*.TXT, DATA\*.TEX, DATA\*.ANI, DEMO\*.DMO
worklog: 3, 15, 16, 22, 28, 34
---

# The level text files

Most of a Hellbender level is CRLF text, and all of it shares one design: a
count on the first line, fixed-length positional records after it, and blocks
appended over the project's life marked by a line starting with a punctuation
character. The same five-mark vocabulary appears in [.LVL](lvl.md) and in
`.DEF`, which means it is a house style rather than a per-file accident.

A parser should check the sentinel lines and refuse a file whose sentinels are
not where the version says they should be.

## .DEF - enemy and object definitions

The biggest of them. First line is a record count; each record is exactly 25
lines. `FLOAT.DEF` declares 83 and has 83. Across all 26 levels there are 1,848
records. The loader at `0x00404fc5` refuses more than 100 per level.

**These records are types, not placements.** Field 2 of a record's first line
is a function of its model: 242 of the 250 models used take exactly one value,
and `wbunker.bin` takes two across 439 records - because it is the model's
radius (see below). The display names say the same
thing - "Extra" appears 395 times and "none" 156 - which is a fixed-size table
with unused slots.

The placements are in the **same file**, in a second section after the type
records. See below.

```
 0  0,0,782409,0,0,0,fmbbld.bin,cube.bin    six ints, then two model names
 1  0,0,0,0,0
 2  0,0,0,0
 3  0,0,0,0,0,0,0,0,0
 4  ;NewHit
 5  0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0
 6  !NewAtakRet
 7  0,0,0,0
 8  Bion Control Center                     display name
 9  #New2ndweapon
10  0,0,0,0
11  %SFX
12  null                                    sound file, or null
13  null                                    sound file, or null
14  : Path to follow
15  -1                                      course id, -1 for none
16  = cannonDamage, laserDamage, missileDamage
17  65536                                   1.0 in 16.16
18  65536
19  65536
20  @ Friendly flag
21  0
22  { Escape and destroy sound files
23  null                                    escape sound
24  null                                    destroy sound
```

What the first line's fields are, measured across all 1,848 records:

| field | distinct values | offset | reading |
| ---: | ---: | ---: | --- |
| 0 | 27 | 0x0c | behaviour class; 0 in 874 records, 9 in 454, 10 (turret) in 118 |
| 1 | 1 | 0x1c | always 0 |
| 2 | 235 | 0x08 | radius, 16.16 world units: the size the model is drawn at |
| 3 | 1 | 0x10 | always 0 |
| 4 | 4 | 0x14 | 0 in 1,830; otherwise 300000, 350000 or 500000 |
| 5 | 1 | 0x18 | always 0 |
| 6 | 250 | | model filename |
| 7 | 7 | | second model filename |

The offsets are into the 664-byte type struct the loader fills, one per record,
in the array at `[0x500748]`.

**Field 0 is the behaviour class.** The actor update switches on it at
`0x40bb93` through a 65-entry jump table at `0x40c6cc`. Class 47 runs
`0x421240`, the course follower (see [the simulation](../engine/simulation.md));
class 10 runs `0x408c30`, a turret. Classes 0 and 9, most of the records, run
nothing but the visibility test.

**Field 2 is the radius.** Models are normalised to +/-1.0 (see
[MRGL](mrgl.md)), and the actor draw at `0x40da00` passes this value as the
draw record's size (`[type+0x08]` into record `+0x1c`, at `0x40dabe`); the
visibility test at `0x40c21f` weighs `radius << 8 / distance` against 16.
Across the 7,606 placements the median radius is 5.8 units - under a cell -
and the largest 44.8.

### The rest of the type record

The loader (`0x404f80` to `0x4056ef`) reads the record's numeric lines with
`fscanf` into these offsets. Lines 6-7 and 9-10 are optional to the engine: it
peeks for the `!` or `#` and, if absent, fills defaults - but every shipped
record has both.

| line | format | offsets | reading |
| ---: | --- | --- | --- |
| 1 | `%d,%d,%d,%d,%d` | 0x64 0x68 0x6c 0x70 0xdc | move rate, turn rate, fire interval, shot damage, weapon kind |
| 2 | `%d,%d,%d,%d` | 0xe0 0xe4 0xe8 0xec | not read yet |
| 3 | nine `%d` | 0x190, 0x194.. | muzzles: a count, then up to eight model vertex indices |
| 5 | seventeen `%d` | 0x1b4, 0x1b8.., 0x1d8.. | hit spheres: a count, eight vertex indices, eight 16.16 half-sizes |
| 7 | `%d,%d,%d,%d` | 0x1f8 0x1fc 0x200 - | attack and retreat range in whole units (defaults 32 and 16), which the flyers' AI compares with; the third, when set, lets the actor play its line-13 sound at random |
| 10 | `%d,%d,%d,%d` | 0x20c 0x210 0x214 0x218 | barrel mode, two unread, shot speed in 16.16 units a second |
| 12 | `%s` | 0x21c | the sound played with each shot; `null` in every record |
| 13 | `%s` | 0x230 | a sound the actor plays now and then |
| 15 | `%d` | 0x244 | course id |
| 17-19 | `%d` | 0x248 0x24c 0x250 | cannon, laser and missile damage multipliers |
| 21 | `%d` | 0x254 | friendly: three friendly kills set `0x512720` |

Line 1 in detail, as the turret uses it:

- **Move rate** and **turn rate** are the rates of the exponential approach at
  `0x4068f0`: each frame an actor's position closes on where it wants to be by
  `error * move_rate * dt` and its angles by `error * turn_rate * dt`.
- **Fire interval** is seconds in 16.16. Frame time accumulates in the actor
  and a shot goes when it passes the interval.
- **Shot damage** comes off the player's health, whose full value is 1.0.
- **Weapon kind** picks the target's damage multiplier and the shot's sound;
  19 is a guided missile instead of a straight shot.

Without line 10, the shot speed defaults to twice the move rate (`0x405338`).

Barrel mode 1 cycles three barrels and 2 cycles five, each offset by 2,048 -
11.25 degrees - in pitch or heading, from the tables at `0x500760` and
`0x500778`:

```
barrel     0      1      2      3      4
pitch      0  -2048      0   2048      0
heading    0      0  -2048      0   2048
```

Field 7 is the wrecked form of field 6: `wbunker.bin` is paired with
`wbnkruin.bin` 439 times, and everything else pairs with `cube.bin`. Fields 1,
3 and 5 are dead in the shipped data.

The loader is at `HELLBEND.EXE:0x00404fc5`, which emits
`"Unable to open enemy description file"` and `"Too many enemy defs"`.

### The instance list

After the last type record the file continues with a count and that many
eight-integer lines. `0x405938` reads them with
`fscanf(file, "%d,%d,%d,%d,%d,%d,%d,%d")` and refuses more than 500.

```
293
0,81920,-21493030,-3768320,19365510,0,0,0
0,81920,-22517262,-5439488,20000101,0,0,32703
1,65536,-21759556,-3768320,18127669,0,0,32632
```

The loader's `fscanf` at `0x4059ab` writes straight into the actor struct, so
each field has an actor offset:

```
0  kind        0x18   index into the type records above
1  hit points  0x1c   16.16; a hit subtracts damage, zero or below is dead
2  x           0x00   16.16 world position, signed, the world is centred
3  y           0x04
4  z           0x08
5  pitch       0x0c   zero in all 7,606 shipped instances
6  roll        0x10   zero in all 7,606
7  heading     0x14   the engine's 16-bit circle
```

Field 1 was first read here as a scale. It is not: the draw at `0x40da00` never
reads it, and the damage routine at `0x40d3fc` subtracts from it. The values
count hits of the player's laser, whose damage is 4,096: 7,453 of the 7,606 are
a whole number of them, and none is zero or below
(`a_placements_second_field_counts_laser_hits`). Difficulty 2 multiplies every
actor's hit points by 1.5 and 3 doubles them as the level loads (`0x405b0b`);
the default is 1.

Measured across all 26 levels: 7,606 instances, **no** kind index out of range,
x and z between -512.0 and +511.9 units, y between -125.5 and +127.5 - which is
exactly the world's horizontal bounds and exactly the terrain's own vertical
span of 127.5 units. `HOTH` places 476, `SHIP` 290, `FLOAT` 293.

The commonest hit points are 0.625, 0.125 and 0.0625 - ten, two and one laser
hits.

### The type record

The loader reads a record's six integers with
`fscanf(file, "%d,%d,%d,%d,%d,%d,%s", ...)` into struct offsets 0x0c, 0x1c,
0x08, 0x10, 0x14 and 0x18. There is only one `%s` for two filenames, because
`%s` stops at whitespace and the two names are separated by a comma - the
loader takes the whole `fmbbld.bin,cube.bin` token and splits it afterwards. It
then checks `fscanf` returned 7.

## .NAV - the mission

A count, then that many records, each a list of navigation points the player
is sent to in turn. The reader is `0x470bd0`; it is `fscanf` driven, and each
block that opens with `!`, `@` or `;` is optional - the loader peeks at the
next line's first character and fills in a default when the block is absent.
A record is 256 bytes in memory at `0x625140`, 50 of them.

```
19                                           record count
6                                            kind (+0xc)
14942208,491520,9699328                      position, 16.16 (+0x0)
!priority,time                               optional block
0,0                                          priority (+0x60), time (+0x64)
@Completion sound & completion text (39 chars max)   optional block
null                                         completion sound (+0x6c)
null: line ignored                           completion text: read, discarded
; Proximity Sound file                       optional block
null                                         proximity sound (+0xa8)
Start for Eyrie Level 1                      objective text (+0x10)
0,0,0                                        kind-specific data (+0xbc...)
-------------------------------------------------
```

- **Priority** 0 is a required point, 1 an optional one; with no `!` block it
  is 1. In the shipped files only the "End of Navs" records are optional.
- **Time** is a limit in 16.16 seconds, 0 for none. Two points in the game
  have one: `IOWAH3`'s damaged jump zone (32.3 s) and `SHIP2`'s escape
  checkpoint (38.9 s).
- **Sounds** are `%s` tokens, so a name stops at the first blank; `null` is
  none. The completion sound defaults to `null` without the `@` block.
- **Text** is the line as `fgets` reads it, `strncpy`'d to 79 bytes with the
  last one dropped: the newline, or the 79th character of a longer line.
- The **position**'s height is raised to the surface under it when below it
  (`0x41c300`), before the kind's data is read.

The kinds, with what follows the text and what completes them (see
[the simulation](../engine/simulation.md#the-mission)):

| Kind | Name (HUD) | Data | In the game |
|---|---|---|---|
| 0 | Destroy Target | count, then that many placement indices, one per line | 179 |
| 1 | Enter Tunnel | - | 17 |
| 2 | Fly to Checkpoint | - | 61 |
| 3 | Fly to Jump Zone | - | 16 |
| 4 | Exit Tunnel | - | 14 |
| 5 | guardian | placement; a music module; a skipped line (`;place4`); then optionally `!NewH`, a count and that many placements | 5 |
| 6 | start | pitch, roll, heading on one line | 47 |
| 7 | sync point | - | 69 |
| 8 | drop a rescue beacon | - | 2 |
| 9 | end of the list | - | 26 |
| 10 | warp | `x,y,z` destination, then a name | 0 |
| 11 | beacon | never in a file; dropped by the player | 0 |
| 12 | Escort Ship | placement | 3 |
| 13 | Pick up Message Pod | pod index | 1 |
| 14 | Destroy Target | placement | 1 |

Anything above 14 stops the loader with "Bad nav type!". A destroy point's
position is replaced by its first target's placement position. The network
levels list eight starts and nothing else.

After reading, the loader:

1. appends a kind-9 end marker if the file has none (priority 1);
2. inserts a sync point after every tunnel exit (4) not followed by one;
3. inserts a sync point before every tunnel entry, jump zone and tunnel exit
   (1, 3, 4) not preceded by one. An inserted record is zeroed - so required -
   and named "Sync point: auto added". Every shipped file already carries
   these, under the same name: the editor ran the same pass;
4. marks each optional point of kind 9 or below done unless `rand() * 100 /
   32767` is 33 or less - two in three optional points are dropped. Only the
   end markers are optional, so this never touches an objective;
5. if the first record is a start, puts the player on it at the surface plus 16
   units, turned by its angles, marks it done and makes record 1 current.

## .TXT under DATA\ - the mission briefing

Not a model and not a placement list. The first two lines name a model and a
backdrop image, then free prose terminated by a line holding a single `.`:

```
Globe.Bin
Morbos00.Raw
PLANET: Eyrie
MISSION: Savior

The Bion shock troops are leaving
planet Eyrie by the thousands aboard
...
.
```

`MODELS\*.TXT` is a completely different thing - an animated model - and shares
only the extension.

## .TEX - the texture list

Count, then that many `.RAW` filenames, one per line. `FLOAT.TEX` declares 274.
This is the table the terrain's per-cell texture indices point into.

## .ANI - animated textures

Count, then per entry: the base texture name, a `frames,delay` pair, then that
many frame texture names. Eleven levels ship one, 146 cycles in all, of two to
eight frames each.

```
24
FDG00.RAW
4,6553
FDG00.RAW
FDWATA2.RAW
FDWATA3.RAW
FDWATA4.RAW
```

`delay` is 16.16 seconds: 6,553 is a tenth of a second, 16,384 a quarter,
32,768 a half. The first frame is the base texture itself.

**The frames are not in the level's `.TEX` list.** 140 of the 146 cycles name
at least one texture the list does not contain, and every one of those is in
`ART\` all the same. The `.TEX` list is the terrain's set; an animation frame
is simply another texture, and the engine keeps one 1,024-entry table that both
register into - `"Too many flippin textures 1"` at `0x00412874` is its overflow
check, and "flippin" there means flip-book.

## .DMO - recorded demo

Has [its own page](demo.md): a tagged stream of poses and key presses, and the
source of the port's heading and pitch conventions.

## .PUP and .TDF

Count-prefixed text in the same family. In most levels both are the three bytes
`0\r\n` - a count of zero and nothing else - so their record shape cannot be
read from the shipped data.

`.CRS` has [its own page](courses.md), and `.GLT`, `.QKE` and `.TTY`
[another](scenery.md).

## Unknown

`.DEF` type field 4 (0x14), line 2 (0xe0-0xec), line 7's fourth value
and line 10's middle two. What the 63 behaviour classes other than 10 and 47
do. How the engine animates a group model such as the SAM site's. The record
shape of `.PUP` and `.TDF`. What a guardian's `;place4` line was for, and the
name a warp point carries.
