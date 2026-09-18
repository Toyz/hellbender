#!/usr/bin/env python3
"""HELLBEND.EXE as a flat address space: strings, xrefs, disassembly.

The retail binary is a 1996 MSVC PE with its debug strings intact, so a string
is usually one xref away from the routine that emits it.

    tools/pe.py info
    tools/pe.py strings [--grep RE] [--min N]
    tools/pe.py xref <hex-va>...        who mentions this address
    tools/pe.py dis <hex-va> [--len N]  disassemble from a VA
    tools/pe.py read <hex-va> --len N   hexdump from a VA
"""

import argparse
import pathlib
import re
import struct
import subprocess
import sys

DEFAULT = pathlib.Path("original/HELLBEND.EXE")


class Image:
    def __init__(self, path=DEFAULT):
        self.path = pathlib.Path(path)
        self.data = self.path.read_bytes()
        d = self.data
        pe = struct.unpack_from("<I", d, 0x3C)[0]
        assert d[pe : pe + 4] == b"PE\0\0", "not a PE"
        nsec = struct.unpack_from("<H", d, pe + 6)[0]
        self.timestamp = struct.unpack_from("<I", d, pe + 8)[0]
        optsize = struct.unpack_from("<H", d, pe + 20)[0]
        opt = pe + 24
        self.entry = struct.unpack_from("<I", d, opt + 16)[0]
        self.base = struct.unpack_from("<I", d, opt + 28)[0]
        self.sections = []
        for i in range(nsec):
            b = pe + 24 + optsize + i * 40
            name = d[b : b + 8].rstrip(b"\0").decode()
            vsize, va, rawsize, rawptr = struct.unpack_from("<IIII", d, b + 8)
            self.sections.append(
                dict(name=name, va=self.base + va, vsize=vsize,
                     rawptr=rawptr, rawsize=rawsize)
            )

    def section_of(self, va):
        for s in self.sections:
            if s["va"] <= va < s["va"] + max(s["vsize"], s["rawsize"]):
                return s
        return None

    def off(self, va):
        """File offset for a VA, or None if the VA is in uninitialised data."""
        s = self.section_of(va)
        if s is None:
            return None
        delta = va - s["va"]
        if delta >= s["rawsize"]:
            return None
        return s["rawptr"] + delta

    def read(self, va, n):
        o = self.off(va)
        return b"" if o is None else self.data[o : o + n]

    def cstr(self, va, limit=512):
        raw = self.read(va, limit)
        end = raw.find(b"\0")
        return raw[: end if end >= 0 else limit]

    def va_of(self, off):
        for s in self.sections:
            if s["rawptr"] <= off < s["rawptr"] + s["rawsize"]:
                return s["va"] + (off - s["rawptr"])
        return None

    def strings(self, minimum=4):
        for m in re.finditer(rb"[\x20-\x7e]{%d,}" % minimum, self.data):
            va = self.va_of(m.start())
            if va is not None:
                yield va, self.section_of(va)["name"], m.group().decode()

    def xrefs(self, va):
        """Every 4-byte little-endian occurrence of `va` in the image."""
        needle = struct.pack("<I", va)
        out = []
        start = 0
        while (i := self.data.find(needle, start)) >= 0:
            start = i + 1
            at = self.va_of(i)
            if at is not None:
                out.append((at, self.section_of(at)["name"]))
        return out

    def calls(self, target):
        """Every `call rel32` and `jmp rel32` in .text that reaches `target`.

        Direct calls are relative, so they do not show up in a search for the
        target's address - which is why `xref` misses them.
        """
        text = next(s for s in self.sections if s["name"] == ".text")
        blob = self.data[text["rawptr"] : text["rawptr"] + text["rawsize"]]
        out = []
        for i in range(len(blob) - 5):
            op = blob[i]
            if op not in (0xE8, 0xE9):
                continue
            rel = int.from_bytes(blob[i + 1 : i + 5], "little", signed=True)
            here = text["va"] + i
            if here + 5 + rel == target:
                out.append((here, "call" if op == 0xE8 else "jmp"))
        return out

    def disassemble(self, va, length=128):
        o = self.off(va)
        if o is None:
            return "(no raw data at that VA)"
        blob = self.data[o : o + length]
        tmp = pathlib.Path("/tmp/_hb_dis.bin")
        tmp.write_bytes(blob)
        out = subprocess.run(
            ["objdump", "-D", "-b", "binary", "-m", "i386",
             "-M", "intel", f"--adjust-vma={va:#x}", str(tmp)],
            capture_output=True, text=True, check=True,
        ).stdout
        return "\n".join(out.splitlines()[7:])


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--exe", default=DEFAULT)
    sub = parser.add_subparsers(dest="command", required=True)
    sub.add_parser("info")
    st = sub.add_parser("strings")
    st.add_argument("--grep", default=None)
    st.add_argument("--min", type=int, default=4)
    xr = sub.add_parser("xref")
    xr.add_argument("va", nargs="+")
    cl = sub.add_parser("calls")
    cl.add_argument("va", nargs="+")
    di = sub.add_parser("dis")
    di.add_argument("va")
    di.add_argument("--len", type=int, default=128)
    rd = sub.add_parser("read")
    rd.add_argument("va")
    rd.add_argument("--len", type=int, default=64)
    args = parser.parse_args()
    img = Image(args.exe)

    if args.command == "info":
        print(f"{img.path}  base {img.base:#010x}  entry {img.base+img.entry:#010x}")
        print(f"{'name':<10}{'VA':>12}{'vsize':>10}{'rawsize':>10}{'rawptr':>10}")
        for s in img.sections:
            print(f"{s['name']:<10}{s['va']:>#12x}{s['vsize']:>10}"
                  f"{s['rawsize']:>10}{s['rawptr']:>10}")
        return 0

    if args.command == "strings":
        pattern = re.compile(args.grep, re.I) if args.grep else None
        for va, sec, text in img.strings(args.min):
            if pattern is None or pattern.search(text):
                print(f"{va:08x} {sec:<7} {text}")
        return 0

    if args.command == "xref":
        for spec in args.va:
            va = int(spec, 16)
            print(f"== {va:#010x} {img.cstr(va)[:60]!r}")
            for at, sec in img.xrefs(va):
                print(f"   {at:08x} {sec}")
        return 0

    if args.command == "calls":
        for spec in args.va:
            target = int(spec, 16)
            found = img.calls(target)
            print(f"== {target:#010x}: {len(found)} direct references")
            for at, kind in found:
                print(f"   {at:08x} {kind}")
        return 0

    if args.command == "read":
        va = int(args.va, 16)
        blob = img.read(va, args.len)
        for i in range(0, len(blob), 16):
            row = blob[i : i + 16]
            hexed = " ".join(f"{b:02x}" for b in row)
            text = "".join(chr(b) if 32 <= b < 127 else "." for b in row)
            print(f"{va+i:08x}  {hexed:<47}  {text}")
        return 0

    print(img.disassemble(int(args.va, 16), args.len))
    return 0


if __name__ == "__main__":
    sys.exit(main())
