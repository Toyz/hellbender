---
title: The .ACT palette
status: solid
covers: ART\*.ACT
worklog: 6
---

# The .ACT palette

768 bytes: 256 entries of three bytes, red then green then blue, one byte each.

```
  0x000  u8 r, u8 g, u8 b     entry 0
  0x003  u8 r, u8 g, u8 b     entry 1
   ...
  0x2fd  u8 r, u8 g, u8 b     entry 255
```

## Notes

- Despite the extension it is not an Adobe Color Table. Adobe's form is 768
  bytes plus an optional 4-byte trailer; no entry in either archive is anything
  other than exactly 768 bytes, or zero.
- Components are full 0-255 range, not the 0-63 that VGA DAC hardware took, so
  no shifting is needed to reach a modern framebuffer.
- 286 of the 288 `.ACT` entries are 768 bytes. The other two are zero-length
  placeholders.

## Which palette applies

Three different mechanisms, depending on what is being drawn:

- A level's ground and its models use the palette named on line 5 of the
  [.LVL](lvl.md) manifest.
- The sky uses the palette on line 12 of the same file.
- An individual texture records the palette it was authored against in the
  second string of its [POD](pod.md) directory entry. The engine ignores this;
  a viewer should not.

In `STARTUP.POD` every one of the 574 `.RAW` entries names `VGA.ACT`. In
`GAME.POD` the five most common are `KREASH.ACT` 470, `MORBOS.ACT` 463,
`FLOAT.ACT` 444, `SHIP.ACT` 441 and `JURASIC.ACT` 388.

## Reserved indices

Index 0 is transparent when an indexed image is blitted as an overlay - see
[.RAW](raw.md) for the measurement. It is `(0, 0, 0)` in `VGA.ACT`.

Index 255 is the fog colour: the last row of a `.FOG`
[colour table](colour-tables.md) sends every index to 255, so that is what a
fully fogged pixel becomes.

Indices 240 to 255 are reserved from shading and blending entirely. A `.LTE`
passes them through unchanged at all 16 levels and a `.MIX` maps them to 0, so
the renderer's colour range is 0 to 239 and the top 16 belong to whatever must
stay exact - the HUD and the front end. See
[colour tables](colour-tables.md).

## Unknown

What exactly claims indices 240 to 255, entry by entry.
