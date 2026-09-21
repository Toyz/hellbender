---
number: 82
title: One register holds both halves of a shade lookup
date: 2026-09-21
area: decomp, render, format
files: docs/formats/colour-tables.md
---

# 82. One register holds both halves of a shade lookup

Toyz asked why the colour tables page is still marked partial. It is because
of two named questions, and one of them was only unanswered because nobody
had read the span loop. So I read it.

## Following the table rather than the light

The `.LTE` goes into `0x605370` with one `fread(0x605370, 256, 16)`
(`0x4861d3`). Finding where it is *used* was the hard part: the address has
five references and every one of them is in the loader. The rasteriser never
mentions it.

It mentions `0x605270` instead, which is 256 bytes earlier - and the two
instructions after the read explain why. `0x48623c` copies the file's row 0
into `0x605270`, and fills `0x606270`, which is where the file's last row
landed, with one byte repeated. So the table the spans see is the file with a
duplicate of row 0 bolted on the front and a flat row on the end.

## The lookup is one addressing mode

```
mov bl, ch                  ; the texture's x
mov bh, dh                  ; and its y
mov eax, ebp                ; ebp is the interpolated light
mov al, [ebx]               ; al = the texel
mov bl, [eax + 0x605270]    ; and the shade of it
```

`eax` is loaded with the light and then has its low byte overwritten by the
texel, which leaves `ah` holding the light's high byte. One register, both
halves of a two-dimensional lookup, one `mov`. The loop is unrolled sixteen
times and every copy does it.

The light is stepped by `0x513310`, whose gradient was built with `sar 4`
(`0x4a19c0`), so the accumulator is the light shifted down four - and the
byte that picks the row is the light's top four bits. Which is what the port
has been doing since [[11]] on the reasoning that sixteen rows over a
0-to-0.996 range could not mean anything else. It is measured now.

## And the half I did not settle

Which end is which. Row 0 is the identity and row 15 is black, and the port
inverts - a bright vertex takes a low row - because that is what makes a
level look right. But `0x44fb7d` puts `0xffff` in the vertex light for the
draws that want no shading at all, and read straight that is the *darkest*
row. One of the two is carrying a sign I have not accounted for, and guessing
which would be worth less than saying so.

**Still unknown:** that sign; and how `.MAP` was generated, which is a
question about the tool that built the disc rather than about the game and
may not have an answer in the image at all.
