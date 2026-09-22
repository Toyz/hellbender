---
number: 5
title: MRGL, the node stream inside a model, and its size table as a jump table
date: 2026-09-17
area: format, decomp
files: tools/mrgl.py, docs/formats/mrgl.md
---

# 5. MRGL, the node stream inside a model, and its size table as a jump table

A `.BIN` model is not a struct with a header. It is a flat stream of
variable-length typed records, terminated by a record of type 0, and the engine
calls the record type an MRGL type - the only place the name appears is the
diagnostic `"Bad MRGL type"` at `0x0050e618`.

The loader at `0x00473d20` does almost nothing:

```
podFileLength("models", name)      0x00463170
open("models", name, "rb")         0x00473a80
malloc(length)
fread(buffer, 1, length, file)
fclose
if (*(u8 *)buffer != 0x14 && *(u8 *)buffer != 0x20) fatal("Bad model!")
return buffer
```

The file is the memory image. Nothing is parsed, nothing is byte-swapped, and
the only validation is that the first record's type is 0x14 or 0x20. It also
dispatches on the extension before any of this: at `0x00473d5a` it upper-cases
the second character of the extension and compares it with `'T'`, so a `.TXT`
model goes to a completely different parser at `0x0046cd60`.

## The size table

Because records are variable length, something has to be able to step over one
without understanding it. That something is the function at `0x00474360`:

```asm
mov  eax, [esp+4]
mov  ecx, [eax]              ; the record type
cmp  ecx, 0x26
ja   bad                     ; "Bad MRGL type"
jmp  DWORD PTR [ecx*4 + 0x47442c]
```

A 39-entry jump table where every arm computes a byte count and returns. That
table is the format specification, and transcribing it is the whole job. `n` is
the record, `n[k]` the `u32` at byte offset `k`:

```
0x00  4                    end of stream
0x01  16
0x02  12 + 12 * n[8]       vertex list, 3 x i32 per vertex
0x03  12 +  4 * n[8]
0x04  12 +  8 * n[8]
0x05  24 +  4 * n[4]       and 0x06 0x07 0x08 0x0f 0x15 0x19 0x1a 0x1b 0x21
0x09  32
0x0a  8
0x0b  8
0x0c  28
0x0d  24                   material: u32, then char[16] texture name
0x0e  24 + 12 * n[4]       polygon: n[4] corners, 12 bytes each
0x10  20
0x11  24 + 12 * n[4]       and 0x18 0x1e 0x22
0x12  8
0x14  8                    mesh start
0x16   8 +  4 * n[4]
0x17  12
0x1d  28 + 32 * n[8]
0x1f  12 +  4 * n[8]
0x20  344                  group node
0x26  15712                animated model, built by the .TXT parser
```

Types 0x13, 0x1c, 0x23, 0x24 and 0x25 fall through to the `"Bad MRGL type"`
arm, so they are holes in the numbering rather than records.

## It walks all 342 models to the byte

`tools/mrgl.py check` steps every `.BIN` in both archives using nothing but
that table and asserts the last record ends exactly at end of file:

```
342/342 models walk to the byte
```

That is the proof. A wrong size for any type that appears would desynchronise
the stream and land on a byte that is not a valid type, and 33,484 records of
type 0x18 alone pass through. The type census:

```
0x18  33484      0x0d  3265      0x02   336      0x14   336
0x0e    244      0x17   270      0x0f    52       0x04    45
0x05     41      0x1d    40      0x0a    37       0x06    24
0x19    215      0x20     6      0x0c     4       0x12     4
0x1f      4      0x00   342
```

## CUBE.BIN, end to end

The smallest model reads cleanly and says what the common types are:

```
0x000000  type 0x14 meshStart   size 8
0x000008  type 0x02 vertexList  size 108   count=8
0x000074  type 0x0d material    size 24    name='rustplat.raw'
0x00008c  type 0x0e polygon     size 72    count=4
   ... six of them ...
0x00023c  type 0x00 end         size 4
                                         576 bytes, the file's exact length
```

Eight vertices, one texture, six quads: a cube. The vertices are `i32` triples
at +12, values `+/-2595` and `+/-2662`. A polygon record is 24 bytes of header and
then 12 bytes per corner, and those 12 bytes are three `i32`: a vertex index
and a `u`, `v` pair in 16.16 fixed point, so corner 0 of the first face is
index 2 at `(1.0, 1.0)`.

A group node (0x20) is 344 bytes and holds child filenames rather than
geometry. `ALIENSH.BIN` is one: type 0x20, child count 8 at +8, then eight
32-byte names `aliensh1.bin` to `aliensh8.bin` from +0x18, then eight pointer
slots at +0x118 that the file leaves zero and the runtime fills in. The free
routine at `0x00473e70` confirms both offsets by walking them. The file is 348
bytes, which is 344 plus the 4-byte end record.

**Still unknown:** the semantics of the 14 record types that appear in the data
but have no name yet - above all 0x18, which is 98% of all records and has the
same size formula as the polygon type 0x0e, so is probably the polygon variant
the exporter actually emitted. The 24-byte polygon header is also unread: only
its corner count at +4 is accounted for.
