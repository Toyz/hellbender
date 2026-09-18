---
number: 4
title: The terrain is seven 128x128 grids, and the loader names them
date: 2026-09-17
area: format, world, decomp
files: docs/formats/terrain.md
---

# 4. The terrain is seven 128x128 grids, and the loader names them

Every level ships thirteen files under `DATA\` whose sizes are suspiciously
round - `.RAW` and `.RA0` through `.RA5` at 16,384 bytes, `.CLR` at 32,768,
`.CL1` at 65,536, `.CL0` and `.CL2` at 196,608. 16,384 is 128 x 128, so the
terrain grid is 128 x 128 and the rest are multiples of one value per cell.

Which file is which comes straight out of the loader at `0x00412c00`. It builds
each filename with `sprintf` from a format string, and the format strings sit
next to the diagnostic that fires when the open fails:

```
0x005013bc  "%s.ra0"   0x005013d0  "Unable to read ground box layer 0"
0x005013f4  "%s.ra1"   0x00501408  "Unable to read ground box layer 1"
0x00501434  "%s.cl0"   0x00501448  "Unable to read ground box texture"
0x0050146c  "%s.ra2"   0x00501480  "Unable to read chamber ground layer"
0x005014a4  "%s.ra3"   0x005014b8  "Unable to read chamber ceiling layer"
0x005014e0  "%s.cl1"   0x005014fc  "Unable to read chamber texture"
0x0050151c  "%s.ra4"   0x00501530  "Unable to read ground box layer 0"
0x00501554  "%s.ra5"   0x00501568  "Unable to read ground box layer 1"
0x0050158c  "%s.cl2"   0x005015a8  "Unable to read ground box texture"
```

So the six `.RAn` files are three pairs, and each pair has a texture file:

```
.raw + .clr        the ground itself: altitude and colour
.ra0 .ra1 + .cl0   ground box set A: bottom altitude, top altitude, textures
.ra2 .ra3 + .cl1   chambers:         floor altitude, ceiling altitude, textures
.ra4 .ra5 + .cl2   ground box set B: bottom altitude, top altitude, textures
```

A "ground box" is an extruded cell - a block standing between two altitudes -
and a "chamber" is its inverse, a roofed void between a floor and a ceiling.
Two box sets and one chamber set per cell is what lets the levels have
buildings stacked on buildings and tunnels underneath them.

## The cell structs, read off the loop strides

The loops are the evidence. The ground loop is at `0x00412d5e`:

```asm
mov  ebx, 0x73bcc0        ; the ground array
mov  ebp, 0x80            ; 128 rows
add  ebx, 6               ; 6 bytes per cell
call fgetc
shl  ax, 7                ; altitude byte scaled by 128
mov  WORD PTR [ebx-6], ax ; stored at cell + 0
```

128 iterations inside 128, stride 6, `u16` written at +0. The colour pass at
`0x00412dd0` starts from `0x73bcc2` - the same array, offset +2 - and branches
on the file's length: at 0x4000 it reads one byte per cell, otherwise two. Every
shipped `.CLR` is 32,768 bytes, so the two-byte path is the live one.

The box loop at `0x00412f2f` has stride 0x12:

```asm
mov  ebx, 0x675fa0        ; the box array
add  ebx, 0x12            ; 18 bytes per cell
```

and 18 is exactly 2 + 2 + 12 - two `u16` altitudes and six `u16` textures. The
size check at `0x0041302a` confirms the six: it compares the `.CL0` length
against 0x18000, which is 98,304, which is six bytes per cell; the shipped
files are 196,608, twice that, so the engine takes the `u16` branch. Six
textures per box is four sides plus top plus bottom.

```
ground cell  6 bytes at 0x0073bcc0   u16 altitude, u16 colour, u16 unknown
box cell    18 bytes at 0x00675fa0   u16 alt0, u16 alt1, u16 texture[6]
```

## Altitudes are bytes scaled by 128

Every altitude pass does `shl ax, 7` on the byte it read. The stored value is a
`u16` in the same 16.16-derived world units the rest of the game uses, so an
altitude file has 256 usable heights and a step of 128 units. That is the whole
vertical resolution of a Hellbender level - it is a heightmap game, and the
heightmap is 8-bit.

The `.CLR` colour value is *not* scaled. It is a raw `u16` per cell, and the
values seen in `FLOAT.CLR` - 0x40df, 0x004d, 0x004e, 0x004f, 0x0050, 0xc0cf -
have a byte-sized low part and a small high part, which reads as an index plus
flags rather than a colour. Confirming that needs the renderer, not the loader.

**Still unknown:** the third `u16` in the ground cell, the meaning of the
`.CLR` high byte, and which of the six box textures is which face. The
one-byte-per-cell variants of `.CLR` and `.CL0` are supported by the loader but
no shipped level uses them, so they cannot be checked against data.
