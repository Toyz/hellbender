#!/usr/bin/env python3
"""The .POD archive: list, extract, inventory.

Terminal Reality's POD1 container, stored uncompressed. See docs/formats/pod.md.

    tools/pod.py list    <pod> [--long]
    tools/pod.py extract <pod> <dest> [--only GLOB]
    tools/pod.py stats   <pod>...
"""

import argparse
import fnmatch
import pathlib
import struct
import sys
from collections import Counter

DIR_OFFSET = 0x54
ENTRY_SIZE = 40


class Entry:
    __slots__ = ("name", "tail", "size", "offset", "index")

    def __init__(self, index, raw_name, size, offset):
        # The 32-byte field holds the path, then a NUL, then whatever was in
        # the packer's buffer - which for art entries is the palette name.
        parts = raw_name.split(b"\0")
        self.index = index
        self.name = parts[0].decode("cp437")
        self.tail = next(
            (p.decode("cp437") for p in parts[1:] if p), ""
        )
        self.size = size
        self.offset = offset

    @property
    def path(self) -> str:
        return self.name.replace("\\", "/")

    @property
    def ext(self) -> str:
        return pathlib.PurePosixPath(self.path).suffix.upper().lstrip(".")


class Pod:
    def __init__(self, path: pathlib.Path):
        self.path = pathlib.Path(path)
        self.data = self.path.read_bytes()
        (count,) = struct.unpack_from("<I", self.data, 0)
        self.comment = self.data[4:DIR_OFFSET].split(b"\0")[0].decode("cp437")
        self.entries = []
        for i in range(count):
            base = DIR_OFFSET + i * ENTRY_SIZE
            raw = self.data[base : base + 32]
            size, offset = struct.unpack_from("<II", self.data, base + 32)
            self.entries.append(Entry(i, raw, size, offset))

    def read(self, entry: Entry) -> bytes:
        return self.data[entry.offset : entry.offset + entry.size]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)

    show = sub.add_parser("list")
    show.add_argument("pod")
    show.add_argument("--long", action="store_true")

    out = sub.add_parser("extract")
    out.add_argument("pod")
    out.add_argument("dest")
    out.add_argument("--only", default="*")

    counts = sub.add_parser("stats")
    counts.add_argument("pod", nargs="+")

    args = parser.parse_args()

    if args.command == "list":
        pod = Pod(args.pod)
        print(f"# {pod.path.name}: {pod.comment!r}, {len(pod.entries)} entries")
        for e in pod.entries:
            if args.long:
                print(f"{e.offset:10d} {e.size:9d}  {e.name:<24} {e.tail}")
            else:
                print(e.name)
        return 0

    if args.command == "extract":
        pod = Pod(args.pod)
        dest = pathlib.Path(args.dest)
        written = 0
        for e in pod.entries:
            if not fnmatch.fnmatch(e.path.upper(), args.only.upper()):
                continue
            target = dest / e.path
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(pod.read(e))
            written += 1
        print(f"{written} files -> {dest}")
        return 0

    for name in args.pod:
        pod = Pod(name)
        by_ext = Counter()
        bytes_ext = Counter()
        for e in pod.entries:
            by_ext[e.ext] += 1
            bytes_ext[e.ext] += e.size
        print(f"== {pod.path.name}  {pod.comment!r}")
        print(f"   {len(pod.entries)} entries, {len(pod.data)} bytes")
        for ext, n in by_ext.most_common():
            print(f"   {ext or '(none)':<8} {n:5d}  {bytes_ext[ext]:12,d} bytes")
    return 0


if __name__ == "__main__":
    sys.exit(main())
