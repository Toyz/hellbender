#!/usr/bin/env python3
"""HELLBEND.EXE's code as functions: found by hand, not by a framework.

`tools/pe.py` treats the image as bytes and disassembles linearly from any
address, which is why so much of this project's disassembly started
mid-instruction. This finds the program's functions the way a disassembler
does - recursive descent from the entry point, from every call, from every
function pointer sitting in the data, and through every jump table - using the
hand-written decoder in `tools/x86.py`. On top of that it knows the import
table, so a call through `[__imp_DirectDrawCreate]` says what it is.

    tools/funcs.py fn <va>              the function holding VA: extent, callers,
                                       callees, imports, strings, globals
    tools/funcs.py dis <va>             disassemble that function from its entry,
                                       in step - objdump renders, we decide where
    tools/funcs.py callers <va>
    tools/funcs.py tree <va> [--depth N]
    tools/funcs.py xref <va>            every instruction that names VA, and how
    tools/funcs.py imports [dll]        the import table
    tools/funcs.py uses <name>          functions that call an import
    tools/funcs.py string <regex>       functions that reference a matching string
    tools/funcs.py coverage             how much of the game the repo has cited
    tools/funcs.py uncovered [--top N]  the biggest functions nobody has cited
    tools/funcs.py areas                uncited code, grouped by the imports and
                                       strings it touches
    tools/funcs.py dead [--top N]       functions nothing calls or points at
    tools/funcs.py check                account for every byte and relocation in
                                       .text; nonzero exit if code was missed

The relocation table does most of the work. The linker listed every dword in
the image that holds an absolute address, so a pointer in data is a pointer
exactly when it is listed, a jump table ends where its listed entries do, and
a listed address in .text that no decoded instruction holds means code was
missed. `check` holds the analysis to that.
"""

import argparse
import bisect
import pathlib
import pickle
import re
import struct
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import pe  # noqa: E402
import x86  # noqa: E402

ROOT = pathlib.Path(__file__).resolve().parent.parent
CACHE = pathlib.Path("/tmp/_hb_funcs.pickle")
CITED_IN = ["docs", "worklog", "crates", "tools"]
# Past this the image is MFC and the C runtime, linked in from libraries.
LIBRARY = 0x4AA000
# Bumped whenever the analysis changes, so a stale cache is not trusted.
VERSION = 15


def imports(img: pe.Image):
    """The import table: IAT slot VA -> (dll, name)."""
    d = img.data
    pe_at = struct.unpack_from("<I", d, 0x3C)[0]
    opt = pe_at + 24
    rva = struct.unpack_from("<I", d, opt + 96 + 8)[0]
    out = {}
    if rva == 0:
        return out
    at = img.base + rva
    while True:
        desc = img.read(at, 20)
        if len(desc) < 20:
            break
        lookup, _, _, name_rva, iat = struct.unpack("<IIIII", desc)
        if name_rva == 0:
            break
        dll = img.cstr(img.base + name_rva).decode(errors="replace").lower()
        names = lookup or iat
        i = 0
        while True:
            entry = struct.unpack("<I", img.read(img.base + names + i * 4, 4))[0]
            if entry == 0:
                break
            if entry & 0x80000000:
                name = f"#{entry & 0xFFFF}"
            else:
                name = img.cstr(img.base + entry + 2).decode(errors="replace")
            out[img.base + iat + i * 4] = (dll, name)
            i += 1
        at += 20
    return out


def relocations(img: pe.Image):
    """Every place the loader would patch if the image moved: the VA of each
    dword in the image that holds an absolute address. The linker wrote one
    for every pointer it emitted, so this is exact - a dword listed here is a
    pointer, and one not listed is not, however much it looks like one."""
    r = next((s for s in img.sections if s["name"] == ".reloc"), None)
    if r is None:
        return set()
    raw = img.data[r["rawptr"]:r["rawptr"] + r["rawsize"]]
    out = set()
    at = 0
    while at + 8 <= len(raw):
        page, size = struct.unpack_from("<II", raw, at)
        if size < 8:
            break
        for i in range(at + 8, at + size, 2):
            e = struct.unpack_from("<H", raw, i)[0]
            if e >> 12 == 3:          # IMAGE_REL_BASED_HIGHLOW
                out.add(img.base + page + (e & 0xFFF))
        at += size
    return out


class Code:
    def __init__(self, img: pe.Image):
        self.img = img
        text = next(s for s in img.sections if s["name"] == ".text")
        self.lo = text["va"]
        self.hi = text["va"] + text["rawsize"]
        self.blob = img.data[text["rawptr"]:text["rawptr"] + text["rawsize"]]
        self.imports = imports(img)
        self.relocs = relocations(img)
        last = max(img.sections, key=lambda s: s["va"])
        self.image_end = last["va"] + max(last["vsize"], last["rawsize"])
        self.insns = {}          # va -> Insn, every instruction reached
        self.functions = {}      # entry -> set of instruction VAs
        self.callees = {}        # entry -> set of entries
        self.tables = {}         # jmp va -> [targets]
        self._analyse()

    def in_text(self, va):
        return self.lo <= va < self.hi

    def insn(self, va):
        got = self.insns.get(va)
        if got is None:
            at = va - self.lo
            got = x86.decode(self.blob, at, va)
            self.insns[va] = got
        return got

    def _table(self, base):
        """Entries of a jump table: relocated dwords pointing into .text, until
        one isn't. The relocations say exactly where a table ends, where
        reading until a dword stops looking like an address can run on into
        the next table."""
        out = []
        for i in range(1024):
            at = base + i * 4
            v = self.dword(at)
            if at not in self.relocs or v is None or not self.in_text(v):
                break
            out.append(v)
        return out

    def dword(self, va):
        raw = self.img.read(va, 4)
        return struct.unpack("<I", raw)[0] if len(raw) == 4 else None

    def _pointers_in(self, ins):
        """Addresses in .text that an instruction names as constants - `push
        offset callback`, `mov [slot], offset handler` - not counting a jump
        table's base. Only relocated operands count."""
        out = []
        for at in range(ins.va + 1, ins.va + ins.length - 3):
            if at in self.relocs:
                v = self.dword(at)
                if self.in_text(v) and v != ins.table:
                    out.append(v)
        return out

    def _flow(self, seeds):
        """Follow control flow from `seeds`, decoding every instruction it
        reaches. Call targets become function entries. A path stops at a
        return, at something that does not decode, or where it would land in
        the middle of an instruction already decoded - two readings of the same
        bytes cannot both be code."""
        work = list(seeds)
        while work:
            va = work.pop()
            while self.in_text(va) and va not in self.insns:
                off = va - self.lo
                if self.start[off]:
                    break
                ins = x86.decode(self.blob, off, va)
                if ins.kind == x86.BAD or any(self.start[off + 1:off + ins.length]):
                    break
                self.insns[va] = ins
                self.start[off] = 1
                for i in range(off + 1, off + ins.length):
                    self.start[i] = 2
                for p in self._pointers_in(ins):
                    self.entries.add(p)
                    work.append(p)
                nxt = va + ins.length
                if ins.kind == x86.CALL:
                    if self.in_text(ins.target):
                        self.entries.add(ins.target)
                        work.append(ins.target)
                    va = nxt
                elif ins.kind == x86.JCC:
                    work.append(ins.target)
                    va = nxt
                elif ins.kind == x86.JMP:
                    va = ins.target
                elif ins.kind == x86.JMP_IND:
                    if ins.table is not None:
                        targets = self._table(ins.table)
                        self.tables[va] = targets
                        self.table_sites.update(ins.table + 4 * i for i in range(len(targets)))
                        self.case_labels.update(targets)
                        work.extend(targets)
                    break
                elif ins.kind in (x86.RET, x86.STOP):
                    break
                else:
                    va = nxt

    def _body(self, entry):
        """The instructions a function owns: what its entry reaches without
        passing another function's entry. A jump to another entry is a tail
        call, and is recorded as a callee."""
        body = set()
        callees = set()
        work = [entry]
        while work:
            va = work.pop()
            while va in self.insns and va not in body:
                if va != entry and va in self.entries:
                    callees.add(va)
                    break
                ins = self.insns[va]
                body.add(va)
                nxt = va + ins.length
                if ins.kind == x86.CALL:
                    if ins.target in self.entries:
                        callees.add(ins.target)
                    va = nxt
                elif ins.kind == x86.JCC:
                    work.append(ins.target)
                    va = nxt
                elif ins.kind == x86.JMP:
                    va = ins.target
                elif ins.kind == x86.JMP_IND:
                    work.extend(self.tables.get(va, ()))
                    break
                elif ins.kind in (x86.RET, x86.STOP):
                    break
                else:
                    va = nxt
        return body, callees

    def _fits_relocs(self, ins):
        """What the relocations say about this being an instruction.

        Returns None if they rule it out - a relocated dword that overlaps it
        without lying wholly inside it, which no instruction allows - or else
        how many of the addresses it names in the image are relocated (they
        vouch for it) and how many are not (a constant like 0x7fffff that
        happens to fall in the image, or a sign it is not code)."""
        for at in range(ins.va - 3, ins.va + ins.length):
            if at in self.relocs and not (ins.va < at <= ins.va + ins.length - 4):
                return None
        vouched = doubted = 0
        for r in ins.refs:
            if self.img.base <= r < self.image_end:
                if any(at in self.relocs and self.dword(at) == r
                       for at in range(ins.va + 1, ins.va + ins.length - 3)):
                    vouched += 1
                else:
                    doubted += 1
        return vouched, doubted

    # Opcodes a 1996 compiler does not emit and data decodes as all the time:
    # segment pushes, BCD, bound, arpl, port I/O, far returns, hlt, cli/sti.
    _RARE = {0x06, 0x07, 0x0E, 0x16, 0x17, 0x1E, 0x1F, 0x27, 0x2F, 0x37, 0x3F,
             0x62, 0x63, 0x6C, 0x6D, 0x6E, 0x6F, 0x8E, 0x9A, 0xC4, 0xC5, 0xCA,
             0xCB, 0xCE, 0xCF, 0xD4, 0xD5, 0xD6, 0xE4, 0xE5, 0xE6, 0xE7, 0xEA,
             0xEC, 0xED, 0xEE, 0xEF, 0xF1, 0xF4, 0xFA, 0xFB}

    def _plausible(self, va, strict=False):
        """Does code start here? Decode forward to a return or a jump, or for
        a good way, and see that everything on the way is code: it decodes,
        it is not the `00 00` zero padding decodes as, it does not run into
        code already found, and the relocations agree with it.

        `strict` is for places nothing points at: there, opcodes a compiler
        never emits count against it too, and a long run with no way out has
        to have enough relocated addresses in it to be believed."""
        at = va
        vouched = doubted = 0
        for _ in range(256):
            if not self.in_text(at):
                return False
            off = at - self.lo
            if self.start[off] == 1 and at != va and not strict:
                # Running on into code already found, on one of its
                # instruction boundaries: a shared tail.
                return True
            if self.blob[off:off + 2] == b"\0\0" or self.start[off]:
                return False
            ins = x86.decode(self.blob, off, at)
            if ins.kind == x86.BAD:
                return False
            fits = self._fits_relocs(ins)
            if fits is None:
                return False
            vouched += fits[0]
            doubted += fits[1]
            if doubted > vouched + 1:
                return False
            if strict and ins.opcode in self._RARE:
                return False
            if ins.kind in (x86.RET, x86.JMP, x86.JMP_IND):
                return True
            at += ins.length
        return not strict or vouched >= 4

    # What MSVC pads between functions with: int3, nop, and the do-nothing
    # instructions it aligns with - `lea esp,[esp]`, `lea ebx,[ebx]`,
    # `lea ecx,[ecx]`, `mov edi,edi`, `add eax,0`. Learned from the gaps
    # between functions, not guessed.
    _PADDING = [bytes.fromhex(h) for h in (
        "8da42400000000", "8d9b00000000", "0500000000", "8d642400", "8d4900",
        "8d4000", "8bff", "8bc0", "90", "cc")]

    def _skip_padding(self, va):
        while self.in_text(va):
            at = va - self.lo
            for pad in self._PADDING:
                if self.blob[at:at + len(pad)] == pad:
                    va += len(pad)
                    break
            else:
                return va
        return va

    def _analyse(self):
        img = self.img
        self.case_labels = set()
        self.table_sites = set()
        self.entries = set()
        # Per byte of .text: 0 not code, 1 an instruction starts here, 2 inside one.
        self.start = bytearray(len(self.blob))
        # 1. Everything the entry point reaches.
        first = img.base + img.entry
        self.entries.add(first)
        self._flow([first])
        # 2. Function pointers in the data: the actor classes' routines, MRGL's
        #    draw handlers, window procedures, callbacks. A dword in a data
        #    section that points into .text at a byte no code has reached yet,
        #    and that decodes as code.
        self.pointed = set()
        for site in self.relocs:
            if self.in_text(site):
                continue
            v = self.dword(site)
            if v is not None and self.in_text(v):
                self.pointed.add(v)
        for v in sorted(self.pointed):
            if v not in self.case_labels:
                self.entries.add(v)
                self._flow([v])
        # 3. The gaps: code nothing reaches - called through a register, or
        #    dead. MSVC starts a function on a 16-byte boundary or right after
        #    padding, so those are the places tried; the rest of a gap is data
        #    (switch byte tables, strings) or padding.
        va = self.lo
        while va < self.hi:
            if self.start[va - self.lo]:
                va += 1
                continue
            end = self.start.find(1, va - self.lo)
            end = self.hi if end < 0 else self.lo + end
            # Right after the padding first, then each boundary. Whatever is
            # found, the gap that remains after it starts the search again, so
            # functions laid end to end with no padding - import thunks - are
            # found one after another.
            tries = [self._skip_padding(va)]
            tries += [t for t in range(va + 15 & ~15, end, 16) if t > tries[0]]
            for t in tries:
                t = self._skip_padding(t)
                if t < end and not self.start[t - self.lo] and self._plausible(t, strict=True):
                    self._flow([t])
                    if self.start[t - self.lo] == 1:
                        self.entries.add(t)
                        va = t
                        break
            else:
                va = end
        # 4. A relocated address in .text that no decoded instruction holds
        #    and no jump table lists is an operand of code still not found -
        #    hand-written assembly, mostly, with opcodes the gap search will
        #    not believe. Search back from it for where that code starts.
        for site in sorted(self.relocs):
            if not self.in_text(site) or self.start[site - self.lo]:
                continue
            if site in self.table_sites:
                continue
            # The gap starts after the last byte of code before the site.
            begin = site - self.lo
            while begin > 0 and not self.start[begin - 1]:
                begin -= 1
            begin += self.lo
            tries = [self._skip_padding(begin)]
            tries += [t for t in range(begin + 15 & ~15, site, 16) if t > tries[0]]
            for t in tries:
                t = self._skip_padding(t)
                if t <= site and not self.start[t - self.lo] and self._plausible(t):
                    self._flow([t])
                    if self.start[site - self.lo]:
                        self.entries.add(t)
                        break
        # 5. Carve the code into functions.
        self.functions = {}
        self.callees = {}
        for e in sorted(self.entries):
            if e in self.insns:
                body, callees = self._body(e)
                self.functions[e] = body
                self.callees[e] = callees
        self.owners = {}
        for e, body in self.functions.items():
            for v in body:
                self.owners.setdefault(v, []).append(e)
        self.callers = {}
        for e, cs in self.callees.items():
            for c in cs:
                self.callers.setdefault(c, set()).add(e)
        # What reaches each function, and whether anything that runs does.
        # The roots are the entry point and every function whose address the
        # program holds somewhere - in data or as an operand - since a pointer
        # the linker relocated is a pointer something can call through.
        self.immediate = set()
        for ins in self.insns.values():
            self.immediate.update(p for p in self._pointers_in(ins) if p in self.functions)
        roots = {first} | (self.pointed & set(self.functions)) | self.immediate
        self.live = set()
        work = list(roots)
        while work:
            e = work.pop()
            if e in self.live:
                continue
            self.live.add(e)
            work.extend(self.callees.get(e, ()))

    def _called(self):
        if not hasattr(self, "_called_cache"):
            s = set()
            for cs in self.callees.values():
                s |= cs
            self._called_cache = s
        return self._called_cache

    def containing(self, va):
        """The function whose body holds VA - the smallest, where a shared
        tail belongs to more than one."""
        if va in self.functions:
            return va
        if not self.in_text(va):
            return None
        at = va - self.lo
        while at > 0 and self.start[at] == 2:
            at -= 1
        owners = self.owners.get(self.lo + at) if self.start[at] == 1 else None
        if not owners:
            return None
        return min(owners, key=lambda e: len(self.functions[e]))

    def size(self, e):
        return sum(self.insns[v].length for v in self.functions[e])

    def how_reached(self, e):
        """Why this is a function: called, a pointer in data, an operand, or
        only found in a gap."""
        ways = []
        if e in self.callers:
            ways.append("call")
        if e in self.pointed:
            ways.append("data")
        if e in self.immediate:
            ways.append("operand")
        return "+".join(ways) or "gap"

    def bytes_of(self, entries):
        """Bytes of code in these functions, counting a shared tail once."""
        seen = set()
        for e in entries:
            seen |= self.functions[e]
        return sum(self.insns[v].length for v in seen)

    def refs_of(self, e):
        out = []
        for va in sorted(self.functions[e]):
            for r in self.insns[va].refs:
                out.append((va, r))
        return out

    def strings_of(self, e):
        out = []
        for _, r in self.refs_of(e):
            if self.in_text(r) or r in self.imports:
                continue
            raw = self.img.cstr(r, 120)
            if len(raw) >= 4 and all(32 <= b < 127 or b in (9, 10, 13) for b in raw):
                out.append((r, raw.decode()))
        return out

    def imports_of(self, e):
        out = []
        for _, r in self.refs_of(e):
            if r in self.imports:
                out.append(self.imports[r])
        return out


def load():
    img = pe.Image()
    stamp = img.path.stat().st_mtime
    if CACHE.exists():
        try:
            cached = pickle.loads(CACHE.read_bytes())
            if cached.get("stamp") == stamp and cached.get("version") == VERSION:
                code = Code.__new__(Code)
                code.__dict__.update(cached["state"])
                code.img = img
                return code
        except Exception:
            pass
    code = Code(img)
    # The instance's state, not the instance: the class is `__main__.Code`
    # when this runs as a script and `funcs.Code` when it is imported.
    state = {k: v for k, v in code.__dict__.items() if k != "img"}
    CACHE.write_bytes(pickle.dumps({"stamp": stamp, "version": VERSION, "state": state}))
    return code


def citations():
    out = {}
    for root in CITED_IN:
        for p in (ROOT / root).rglob("*"):
            if p.suffix not in (".md", ".rs", ".py"):
                continue
            text = p.read_text(errors="ignore")
            for m in re.finditer(r"0x0*(4[0-9a-f]{5})\b", text):
                va = int(m.group(1), 16)
                if 0x401000 <= va < 0x4EA000:
                    out.setdefault(va, set()).add(str(p.relative_to(ROOT)))
    return out


def cited_functions(code):
    hit = {}
    for va, files in citations().items():
        e = code.containing(va)
        if e is not None:
            hit.setdefault(e, set()).update(files)
    return hit


def describe(code, e, cited):
    body = code.functions[e]
    print(f"{e:#010x}  {code.size(e)} bytes, {len(body)} instructions, "
          f"{min(body):#x}..{max(body):#x}")
    where = cited.get(e)
    print(f"  cited in: {', '.join(sorted(where)[:5]) if where else 'nothing'}")
    callers = sorted(code.callers.get(e, ()))
    print(f"  reached by: {code.how_reached(e)}{'' if e in code.live else ' - nothing that runs reaches it'}")
    pointed = " (also a pointer in data)" if e in code.pointed else ""
    mark = lambda v: f"{v:#x}{'*' if v in cited else ''}"
    print(f"  callers ({len(callers)}){pointed}: {' '.join(mark(c) for c in callers[:14])}")
    callees = sorted(code.callees.get(e, ()))
    print(f"  callees ({len(callees)}): {' '.join(mark(c) for c in callees[:14])}")
    imps = sorted(set(code.imports_of(e)))
    if imps:
        print(f"  imports: {', '.join(f'{d}!{n}' for d, n in imps[:10])}")
    for at, s in code.strings_of(e)[:10]:
        print(f"  string {at:#x}: {s[:72]!r}")
    tables = [v for v in body if v in code.tables]
    for t in tables:
        print(f"  jump table at {t:#x}: {len(code.tables[t])} entries")
    globs = sorted({r for _, r in code.refs_of(e)
                    if not code.in_text(r) and r not in code.imports and 0x500000 <= r < 0x900000})
    if globs:
        print(f"  globals ({len(globs)}): {' '.join(hex(g) for g in globs[:14])}"
              f"{' ...' if len(globs) > 14 else ''}")


def disassemble(code, e):
    """Each contiguous run of the function's body, rendered from a boundary we
    trust, so objdump never starts mid-instruction."""
    body = sorted(code.functions[e])
    runs = []
    start = prev = body[0]
    for va in body[1:]:
        if va != prev + code.insns[prev].length:
            runs.append((start, prev + code.insns[prev].length))
            start = va
        prev = va
    runs.append((start, prev + code.insns[prev].length))
    for lo, hi in runs:
        for line, parsed in code.img.objdump(lo, hi - lo):
            note = ""
            if parsed:
                ins = code.insns.get(parsed[0])
                if ins is not None:
                    if ins.kind == x86.CALL and ins.target in code.functions:
                        note = f"   ; -> {ins.target:#x}"
                    for r in ins.refs:
                        if r in code.imports:
                            d, n = code.imports[r]
                            note = f"   ; {d}!{n}"
                        elif not code.in_text(r):
                            s = code.img.cstr(r, 40)
                            if len(s) >= 4 and all(32 <= b < 127 for b in s):
                                note = f"   ; {s.decode()!r}"
            print(line + note)
        print("  ...")


def check(code):
    """Account for every byte of .text and every relocation in it. Code,
    jump tables, padding and the byte tables switches index through should
    be all there is; a relocated address nothing holds is code not found."""
    n = len(code.blob)
    tables = bytearray(n)
    for site in code.table_sites:
        tables[site - code.lo:site - code.lo + 4] = b"\1" * 4
    for name, lo, hi in (("game", code.lo, LIBRARY), ("library", LIBRARY, code.hi)):
        a, b = lo - code.lo, hi - code.lo
        insns = sum(1 for x in code.start[a:b] if x)
        tab = sum(tables[a:b])
        pad = other = 0
        leftovers = []
        i = a
        while i < b:
            if code.start[i] or tables[i]:
                i += 1
                continue
            j = i
            while j < b and not code.start[j] and not tables[j]:
                j += 1
            p = code._skip_padding(code.lo + i) - code.lo
            pad += min(p, j) - i
            if p < j:
                other += j - p
                leftovers.append((j - p, code.lo + p))
            i = j
        print(f"{name:8} {b - a:7} bytes: code {insns}, jump tables {tab}, padding {pad}, "
              f"other {other} in {len(leftovers)} runs")
        for size, va in sorted(leftovers, reverse=True)[:5]:
            print(f"           {va:#x} {size:5}b  {code.img.read(va, 16).hex()}")
    loose = [s for s in code.relocs if code.in_text(s) and not code.start[s - code.lo]
             and s not in code.table_sites]
    inside = sum(1 for s in code.relocs if code.in_text(s))
    print(f"relocations in .text: {inside}; held by no instruction or table: {len(loose)}")
    for s in sorted(loose)[:10]:
        print(f"  {s:#x}")
    return 1 if loose else 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = parser.add_subparsers(dest="cmd", required=True)
    for name in ("fn", "dis", "callers", "xref"):
        sub.add_parser(name).add_argument("va")
    tr = sub.add_parser("tree")
    tr.add_argument("va")
    tr.add_argument("--depth", type=int, default=2)
    im = sub.add_parser("imports")
    im.add_argument("dll", nargs="?")
    sub.add_parser("uses").add_argument("name")
    sub.add_parser("string").add_argument("pattern")
    sub.add_parser("coverage")
    un = sub.add_parser("uncovered")
    un.add_argument("--top", type=int, default=40)
    un.add_argument("--min-bytes", type=int, default=64)
    un.add_argument("--dead", action="store_true", help="include code nothing reaches")
    sub.add_parser("areas")
    de = sub.add_parser("dead")
    de.add_argument("--top", type=int, default=40)
    de.add_argument("--min-bytes", type=int, default=32)
    sub.add_parser("check")
    args = parser.parse_args()
    code = load()

    if args.cmd in ("fn", "dis", "callers", "tree"):
        e = code.containing(int(args.va, 16))
        if e is None:
            print("no function holds that address")
            return 1
    if args.cmd == "fn":
        describe(code, e, cited_functions(code))
        return 0
    if args.cmd == "dis":
        describe(code, e, cited_functions(code))
        print()
        disassemble(code, e)
        return 0
    if args.cmd == "callers":
        cited = cited_functions(code)
        for c in sorted(code.callers.get(e, ())):
            print(f"{c:#010x} {code.size(c):6} bytes  {', '.join(sorted(cited.get(c, {'-'}))[:3])}")
        return 0
    if args.cmd == "tree":
        cited = cited_functions(code)
        seen = set()

        def walk(v, depth, indent):
            if v not in code.functions:
                return
            imps = sorted({n for _, n in code.imports_of(v)})[:3]
            words = [s[:28] for _, s in code.strings_of(v)[:2]]
            print(f"{indent}{'*' if v in cited else ' '}{v:#x} ({code.size(v)}b) "
                  f"{' '.join(imps)} {' | '.join(repr(w) for w in words)}")
            if depth == 0 or v in seen:
                return
            seen.add(v)
            for c in sorted(code.callees.get(v, ())):
                walk(c, depth - 1, indent + "  ")

        walk(e, args.depth, "")
        print("\n* = cited in the repository")
        return 0
    if args.cmd == "xref":
        target = int(args.va, 16)
        for f, body in sorted(code.functions.items()):
            for va in sorted(body):
                ins = code.insns[va]
                how = None
                if ins.target == target:
                    how = ins.kind
                elif target in ins.refs:
                    how = "table" if ins.table == target else "ref"
                if how:
                    print(f"{va:#010x} in {f:#010x}  {how}")
        return 0
    if args.cmd == "imports":
        for slot, (d, n) in sorted(code.imports.items(), key=lambda kv: kv[1]):
            if args.dll is None or args.dll.lower() in d:
                print(f"{slot:#010x}  {d}!{n}")
        return 0
    if args.cmd == "uses":
        pat = re.compile(args.name, re.I)
        for e in sorted(code.functions):
            hits = sorted({f"{d}!{n}" for d, n in code.imports_of(e) if pat.search(n)})
            if hits:
                print(f"{e:#010x} {code.size(e):6}b  {', '.join(hits)}")
        return 0
    if args.cmd == "string":
        pat = re.compile(args.pattern, re.I)
        for e in sorted(code.functions):
            hits = [s for _, s in code.strings_of(e) if pat.search(s)]
            if hits:
                print(f"{e:#010x}  {hits[0][:70]!r}{f' +{len(hits) - 1}' if len(hits) > 1 else ''}")
        return 0

    cited = cited_functions(code)
    game = [e for e in code.functions if e < LIBRARY]
    if args.cmd == "coverage":
        total = code.bytes_of(game)
        got = code.bytes_of(e for e in game if e in cited)
        lib = code.bytes_of(e for e in code.functions if e >= LIBRARY)
        live = [e for e in game if e in code.live]
        print(f"{len(code.functions)} functions found; {len(game)} below {LIBRARY:#x} "
              f"({total} bytes), the rest library ({lib} bytes)")
        print(f"reached: {len(live)} of the game's functions, {code.bytes_of(live)} bytes; "
              f"the other {len(game) - len(live)} are never called or pointed at")
        print(f"cited: {sum(1 for e in game if e in cited)} of the game's functions, "
              f"{got} of its bytes ({100 * got / total:.1f}%); of the reached, "
              f"{100 * code.bytes_of(e for e in live if e in cited) / code.bytes_of(live):.1f}%")
        for lo, hi in ((0, 64), (64, 256), (256, 1024), (1024, 4096), (4096, 1 << 30)):
            band = [e for e in game if lo <= code.size(e) < hi]
            if band:
                c = sum(1 for e in band if e in cited)
                print(f"  {lo:>5}-{hi if hi < 1 << 30 else '':>5} bytes: {c:4}/{len(band):4} cited")
        return 0
    if args.cmd == "uncovered":
        todo = [e for e in game if e not in cited and code.size(e) >= args.min_bytes
                and (args.dead or e in code.live)]
        todo.sort(key=lambda e: -code.size(e))
        for e in todo[: args.top]:
            imps = sorted({n for _, n in code.imports_of(e)})[:2]
            words = "; ".join(s[:34] for _, s in code.strings_of(e)[:2])
            via = "ptr" if e in code.pointed else f"{len(code.callers.get(e, ())):2}c"
            print(f"{e:#010x} {code.size(e):6}b {via:>4}  {' '.join(imps)}  {words}")
        return 0
    if args.cmd == "dead":
        todo = [e for e in game if e not in code.live and code.size(e) >= args.min_bytes]
        todo.sort(key=lambda e: -code.size(e))
        for e in todo[: args.top]:
            imps = sorted({n for _, n in code.imports_of(e)})[:2]
            words = "; ".join(s[:40] for _, s in code.strings_of(e)[:2])
            mark = "*" if e in cited else " "
            print(f"{mark}{e:#010x} {code.size(e):6}b  {' '.join(imps)}  {words}")
        print(f"\n{sum(1 for e in game if e not in code.live)} functions, "
              f"{code.bytes_of(e for e in game if e not in code.live)} bytes, "
              "that nothing calls or holds a pointer to. * = cited")
        return 0
    if args.cmd == "check":
        return check(code)
    if args.cmd == "areas":
        groups = {}
        for e in game:
            if e in cited or e not in code.live:
                continue
            keys = {f"import {n}" for _, n in code.imports_of(e)}
            for _, s in code.strings_of(e):
                keys |= {w.lower() for w in re.findall(r"[A-Za-z]{5,}", s)}
            for k in keys:
                groups.setdefault(k, set()).add(e)
        ranked = sorted(groups.items(), key=lambda kv: -sum(code.size(v) for v in kv[1]))
        for k, vs in ranked[:70]:
            print(f"{k:<34} {len(vs):3} fns {sum(code.size(v) for v in vs):7}b")
        return 0
    return 1


if __name__ == "__main__":
    sys.exit(main())
