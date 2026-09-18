#!/usr/bin/env python3
"""MRGL: the node stream inside a .BIN model.

A .BIN model is a flat sequence of variable-length typed nodes, terminated by a
type-0 node. Every node begins with a u32 type; the engine's size switch at
HELLBEND.EXE:0x474360 gives each type's length, and that table is transcribed
here. See docs/formats/mrgl.md.

    tools/mrgl.py walk <file.bin> [--depth N]
    tools/mrgl.py check <dir>...        every node accounted for, to the byte
    tools/mrgl.py types <dir>...        which types appear, and how often
"""

import argparse
import collections
import pathlib
import struct
import sys

# type -> (constant, count_field_offset, per_item)
# size = constant + per_item * u32_at(count_field_offset), count_field None for
# fixed-size nodes. Transcribed from the jump table at 0x0047442c.
SIZES = {
    0x00: (4, None, 0),
    0x01: (16, None, 0),
    0x02: (12, 8, 12),
    0x03: (12, 8, 4),
    0x04: (12, 8, 8),
    0x05: (24, 4, 4),
    0x06: (24, 4, 4),
    0x07: (24, 4, 4),
    0x08: (24, 4, 4),
    0x09: (32, None, 0),
    0x0A: (8, None, 0),
    0x0B: (8, None, 0),
    0x0C: (28, None, 0),
    0x0D: (24, None, 0),
    0x0E: (24, 4, 12),
    0x0F: (24, 4, 4),
    0x10: (20, None, 0),
    0x11: (24, 4, 12),
    0x12: (8, None, 0),
    0x14: (8, None, 0),
    0x15: (24, 4, 4),
    0x16: (8, 4, 4),
    0x17: (12, None, 0),
    0x18: (24, 4, 12),
    0x19: (24, 4, 4),
    0x1A: (24, 4, 4),
    0x1B: (24, 4, 4),
    0x1D: (28, 8, 32),
    0x1E: (24, 4, 12),
    0x1F: (12, 8, 4),
    0x20: (344, None, 0),
    0x21: (24, 4, 4),
    0x22: (24, 4, 12),
    0x26: (15712, None, 0),
}

NAMES = {
    0x00: "end",
    0x02: "vertexList",
    0x0D: "material",
    0x0E: "polygon",
    0x14: "meshStart",
    0x20: "group",
    0x26: "animated",
}


def node_size(blob: bytes, at: int) -> int:
    kind = struct.unpack_from("<I", blob, at)[0]
    if kind not in SIZES:
        raise ValueError(f"bad MRGL type {kind:#x} at {at:#x}")
    const, field, per = SIZES[kind]
    if field is None:
        return const
    count = struct.unpack_from("<i", blob, at + field)[0]
    return const + per * count


def walk(blob: bytes):
    """Yield (offset, type, size) for every node, stopping after type 0."""
    at = 0
    while at < len(blob):
        kind = struct.unpack_from("<I", blob, at)[0]
        size = node_size(blob, at)
        yield at, kind, size
        at += size
        if kind == 0:
            break


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    w = sub.add_parser("walk")
    w.add_argument("file")
    c = sub.add_parser("check")
    c.add_argument("dirs", nargs="+")
    t = sub.add_parser("types")
    t.add_argument("dirs", nargs="+")
    args = parser.parse_args()

    if args.command == "walk":
        blob = pathlib.Path(args.file).read_bytes()
        for at, kind, size in walk(blob):
            name = NAMES.get(kind, "")
            extra = ""
            if kind in (0x02, 0x0E, 0x1D, 0x1F, 0x03, 0x04):
                extra = f" count={struct.unpack_from('<i', blob, at + (8 if kind in (0x02,0x03,0x04,0x1D,0x1F) else 4))[0]}"
            if kind == 0x0D:
                extra = f" name={blob[at+8:at+24].split(b(0)).decode() if False else blob[at+8:at+24].split(bytes(1))[0].decode('cp437')!r}"
            print(f"{at:#08x}  type {kind:#04x} {name:<11} size {size:<6}{extra}")
        print(f"file {len(blob)} bytes")
        return 0

    files = []
    for d in args.dirs:
        p = pathlib.Path(d)
        files += sorted(p.rglob("*.BIN")) if p.is_dir() else [p]

    if args.command == "types":
        seen = collections.Counter()
        for f in files:
            try:
                for _, kind, _ in walk(f.read_bytes()):
                    seen[kind] += 1
            except (ValueError, struct.error):
                pass
        for kind, n in sorted(seen.items()):
            print(f"type {kind:#04x} {NAMES.get(kind,''):<11} {n:8d}")
        return 0

    bad = 0
    for f in files:
        blob = f.read_bytes()
        try:
            end = 0
            for at, kind, size in walk(blob):
                end = at + size
            if end != len(blob):
                print(f"{f}: ends at {end}, file is {len(blob)}")
                bad += 1
        except (ValueError, struct.error) as exc:
            print(f"{f}: {exc}")
            bad += 1
    print(f"{len(files) - bad}/{len(files)} models walk to the byte")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
