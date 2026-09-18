---
title: The .GLT lights, .QKE moving geometry and .TTY ground types
status: partial
covers: DATA\*.GLT, DATA\*.QKE, DATA\*.TTY
worklog: 15
---

# .GLT, .QKE and .TTY

Three small per-level text files, all count-prefixed in the house style.

## .GLT - destructible lights

A count, then per entry three texture names and two parameter lines.

```
11
ZLTE1ON.RAW
ZLTE1OFF.RAW
ZLTE1BRK.RAW
262144,90000,131072,6,1
32768,4,1
```

The three states are readable from the names: lit, unlit, broken. `262144` is
4.0 in 16.16 and `131072` is 2.0. Ten levels ship one, between 231 and 983
bytes.

## .QKE - moving geometry

The biggest of the small files, up to 72 KB. "Quake" is the engine's own word:
the diagnostics are `processBoxQuake: no match for watchBox found` and the
counters `-------- Ground Quake Count -------` and
`-------- Box Quake Count --------`, and the file writes those same banners.

```
-------- Ground Quake Count -------
1
------- Ground Quake Entry 1-------
0
0,0
0,0,0,0,1
...
!--Additional quake info--
0
-------- Box Quake Count --------
5
-------- Box Quake Entry 1 --------
1
7040,128
48,65,1
-1,0,0,0
0,65536,262144,65536,0
0,1,0,1,1
1-0UDOOR.WAV
1-0DDOOR.WAV
NULL
NULL
NULL
!--Additional quake info--
```

A box quake entry names two sounds, and in this one they are `1-0UDOOR.WAV` and
`1-0DDOOR.WAV` - a door going up and a door going down. `48,65,1` reads as a
cell reference plus a flag, and `65536` and `262144` are 1.0 and 4.0 in 16.16.
So a box quake is a [ground box](terrain.md) that moves: a door, a lift or a
shaking structure. That a box is "watched" implies the system tracks which
cells are in motion so that collision stays right while they move.

## .TTY - the ground type list

Every shipped `.TTY` is the three bytes `0\r\n` - a count of zero and nothing
else. Its name comes from the save side: `0x41e0c0` replaces the level's
extension with `.tty`, opens it in `data` with mode `wt`, and fails with
`"Unable to save ground type list"`.

So the format exists, the editor writes it, and no shipped level uses it. Its
record shape cannot be read from the data.

## Unknown

Every numeric field of `.GLT` and `.QKE` beyond the ones named above. The
record shape of `.TTY`. Whether a ground quake and a box quake share any
structure - they have separate counts and separate banners, so probably not.
