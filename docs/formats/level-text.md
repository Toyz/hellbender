---
title: The level text files - .DEF, .NAV, .TXT, .TEX, .ANI, .LVL family
status: partial
covers: DATA\*.DEF, DATA\*.NAV, DATA\*.TXT, DATA\*.TEX, DATA\*.ANI, DEMO\*.DMO
worklog: 3, 15, 16, 22
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
and `wbunker.bin` takes two across 439 records. The display names say the same
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

| field | distinct values | reading |
| ---: | ---: | --- |
| 0 | 27 | a type or class id; 0 in 874 records, 9 in 454 |
| 1 | 1 | always 0 |
| 2 | 235 | large, 36269 and 304869 dominate; hit points or an id |
| 3 | 1 | always 0 |
| 4 | 4 | 0 in 1,830; otherwise 300000, 350000 or 500000 |
| 5 | 1 | always 0 |
| 6 | 250 | model filename |
| 7 | 7 | second model filename |

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

```
0  kind        index into the type records above
1  scale       16.16; 1.0 is the model's own size
2  x           16.16 world position, signed, the world is centred
3  y
4  z
5  unknown     zero in all 7,606 shipped instances
6  unknown     zero in all 7,606
7  heading     the engine's 16-bit circle
```

Measured across all 26 levels: 7,606 instances, **no** kind index out of range,
x and z between -512.0 and +511.9 units, y between -125.5 and +127.5 - which is
exactly the world's horizontal bounds and exactly the terrain's own vertical
span of 127.5 units. `HOTH` places 476, `SHIP` 290, `FLOAT` 293.

The commonest scales are 0.625, 0.125 and 0.0625, so most objects are drawn
much smaller than their model's own size. A model is normalised - see
[MRGL](mrgl.md) - so the scale is the object's half-extent in world units.

### The type record

The loader reads a record's six integers with
`fscanf(file, "%d,%d,%d,%d,%d,%d,%s", ...)` into struct offsets 0x0c, 0x1c,
0x08, 0x10, 0x14 and 0x18. There is only one `%s` for two filenames, because
`%s` stops at whitespace and the two names are separated by a comma - the
loader takes the whole `fmbbld.bin,cube.bin` token and splits it afterwards. It
then checks `fscanf` returned 7.

## .NAV - navigation courses

Count, then records. `FLOAT.NAV` declares 19. A record carries a position
triple, a priority and time pair, a completion sound and text, a proximity
sound, a display name, and a separator line of dashes.

```
19                                           record count
6
14942208,491520,9699328                      position, 16.16 fixed point
!priority,time
0,0
@Completion sound & completion text (39 chars max)
null
null: line ignored
; Proximity Sound file
null
Start for Eyrie Level 1                      display name
0,0,0
-------------------------------------------------
```

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

The meaning of `.DEF` type fields 0, 2 and 4, and of its lines 1 to 7 and 10.
What instance fields 5 and 6 are, given they are zero everywhere. The record
shape of `.PUP` and `.TDF`. Whether `.NAV`'s leading `6` is part of the header
or the first record.
