#!/usr/bin/env python3
"""A 32-bit x86 decoder, written by hand, for HELLBEND.EXE.

It decodes what the analysis needs and no more: how long an instruction is,
whether and where it transfers control, and which absolute addresses it names.
It does not print assembly - `tools/pe.py dis` does that - and it does not know
what most instructions compute. That is enough to find functions, follow calls,
read jump tables and answer "who touches this address" by instruction rather
than by raw bytes, which is where `pe.py xref` falls short.

The instruction set is what a 1996 MSVC build and its hand-written spans use:
the integer set, x87, MMX, and the two-byte map around them.
"""

from dataclasses import dataclass, field

# Kinds of control flow an instruction can have.
OTHER = "other"
CALL = "call"          # call rel32
CALL_IND = "call*"     # call through a register or memory
JMP = "jmp"            # jmp rel8/rel32
JMP_IND = "jmp*"       # jmp through a register or memory - maybe a table
JCC = "jcc"            # conditional, falls through as well
RET = "ret"
STOP = "stop"          # hlt, int3, ud2: nothing follows
BAD = "bad"            # not something this decoder knows


@dataclass
class Insn:
    va: int
    length: int
    kind: str = OTHER
    # Where a relative branch goes.
    target: int | None = None
    # Absolute addresses the instruction names: a disp32 with no base
    # register, an immediate, or a disp32 beside an index (a table).
    refs: list = field(default_factory=list)
    # For `jmp [reg*4 + table]`: the table's address.
    table: int | None = None
    # The ModRM's reg field and the opcode, for callers that want to tell a
    # `call [..]` from a `push [..]`.
    opcode: int = 0
    reg: int = 0


# Which one-byte opcodes take a ModRM byte.
_MODRM = set()
for base in range(0x00, 0x40, 8):
    _MODRM.update(range(base, base + 4))
_MODRM.update({0x62, 0x63, 0x69, 0x6B, 0x80, 0x81, 0x82, 0x83})
_MODRM.update(range(0x84, 0x90))
_MODRM.update({0xC0, 0xC1, 0xC4, 0xC5, 0xC6, 0xC7})
_MODRM.update(range(0xD0, 0xD4))
_MODRM.update(range(0xD8, 0xE0))
_MODRM.update({0xF6, 0xF7, 0xFE, 0xFF})

# Immediate bytes after the ModRM (or opcode), keyed by one-byte opcode. "z"
# is 4 bytes, or 2 under an operand-size prefix.
_IMM = {}
for base in range(0x00, 0x40, 8):
    _IMM[base + 4] = 1
    _IMM[base + 5] = "z"
_IMM.update({0x68: "z", 0x69: "z", 0x6A: 1, 0x6B: 1})
_IMM.update({op: 1 for op in range(0x70, 0x80)})
_IMM.update({0x80: 1, 0x81: "z", 0x82: 1, 0x83: 1})
_IMM.update({0xA8: 1, 0xA9: "z"})
_IMM.update({op: 1 for op in range(0xB0, 0xB8)})
_IMM.update({op: "z" for op in range(0xB8, 0xC0)})
_IMM.update({0xC0: 1, 0xC1: 1, 0xC2: 2, 0xC6: 1, 0xC7: "z", 0xC8: 3, 0xCA: 2, 0xCD: 1})
_IMM.update({0xD4: 1, 0xD5: 1})
_IMM.update({op: 1 for op in range(0xE0, 0xE8)})
_IMM.update({0xE8: 4, 0xE9: 4, 0xEB: 1})

_PREFIXES = {0x26, 0x2E, 0x36, 0x3E, 0x64, 0x65, 0x66, 0x67, 0xF0, 0xF2, 0xF3}


def _modrm(code: bytes, at: int, addr16: bool):
    """Decode a ModRM (and SIB, and displacement) at `at`.

    Returns (bytes used, reg field, absolute address or None, table base or
    None, is a register operand).
    """
    if at >= len(code):
        return None
    m = code[at]
    mod, reg, rm = m >> 6, (m >> 3) & 7, m & 7
    used = 1
    absolute = None
    table = None
    if mod == 3:
        return used, reg, None, None, True
    if addr16:
        if mod == 0 and rm == 6:
            used += 2
        elif mod == 1:
            used += 1
        elif mod == 2:
            used += 2
        return used, reg, None, None, False
    has_sib = rm == 4
    base = rm
    index = None
    if has_sib:
        if at + used >= len(code):
            return None
        sib = code[at + used]
        used += 1
        base = sib & 7
        index = (sib >> 3) & 7
        if index == 4:
            index = None
    if mod == 0 and base == 5:
        if at + used + 4 > len(code):
            return None
        disp = int.from_bytes(code[at + used:at + used + 4], "little")
        used += 4
        if has_sib and index is not None:
            table = disp
        else:
            absolute = disp
    elif mod == 1:
        used += 1
    elif mod == 2:
        if at + used + 4 > len(code):
            return None
        disp = int.from_bytes(code[at + used:at + used + 4], "little")
        used += 4
        # [reg + disp32]: a disp32 this large is an address with a register
        # stepping through it, not a small structure offset.
        if disp >= 0x400000:
            table = disp
    return used, reg, absolute, table, False


def decode(code: bytes, at: int, va: int) -> Insn:
    """One instruction from `code[at:]`, which sits at `va`."""
    start = at
    opsize16 = False
    addr16 = False
    while at < len(code) and code[at] in _PREFIXES and at - start < 14:
        if code[at] == 0x66:
            opsize16 = True
        elif code[at] == 0x67:
            addr16 = True
        at += 1
    if at >= len(code):
        return Insn(va, max(1, at - start), BAD)
    op = code[at]
    at += 1

    if op == 0x0F:
        return _decode_0f(code, start, at, va, opsize16, addr16)

    ins = Insn(va, 0, OTHER, opcode=op)
    reg = 0
    if op in _MODRM:
        got = _modrm(code, at, addr16)
        if got is None:
            return Insn(va, at - start, BAD)
        used, reg, absolute, table, is_reg = got
        at += used
        ins.reg = reg
        if absolute is not None:
            ins.refs.append(absolute)
        if table is not None:
            ins.refs.append(table)
            ins.table = table
    imm = _IMM.get(op)
    # test in group 3 carries an immediate; the others in it do not.
    if op in (0xF6, 0xF7) and reg in (0, 1):
        imm = 1 if op == 0xF6 else "z"
    if imm == "z":
        imm = 2 if opsize16 else 4
    if op in (0xA0, 0xA1, 0xA2, 0xA3):
        imm = 2 if addr16 else 4
        if at + imm <= len(code) and imm == 4:
            ins.refs.append(int.from_bytes(code[at:at + 4], "little"))
    if op in (0x9A, 0xEA):
        imm = 6 if not opsize16 else 4
    value = None
    if imm:
        if at + imm > len(code):
            return Insn(va, at - start, BAD)
        value = int.from_bytes(code[at:at + imm], "little", signed=op in
                               (0xE8, 0xE9, 0xEB) or 0x70 <= op <= 0x7F or 0xE0 <= op <= 0xE3)
        # An immediate that could be an address - a push of a string, a mov
        # of a function pointer.
        if imm == 4 and op not in (0xE8, 0xE9) and value >= 0x400000:
            ins.refs.append(value & 0xFFFFFFFF)
        at += imm
    ins.length = at - start

    if op == 0xE8:
        ins.kind, ins.target = CALL, (va + ins.length + value) & 0xFFFFFFFF
    elif op in (0xE9, 0xEB):
        ins.kind, ins.target = JMP, (va + ins.length + value) & 0xFFFFFFFF
    elif 0x70 <= op <= 0x7F or 0xE0 <= op <= 0xE3:
        ins.kind, ins.target = JCC, (va + ins.length + value) & 0xFFFFFFFF
    elif op in (0xC2, 0xC3, 0xCA, 0xCB, 0xCF):
        ins.kind = RET
    elif op in (0xCC, 0xF4):
        ins.kind = STOP
    elif op == 0xFF:
        if reg in (2, 3):
            ins.kind = CALL_IND
        elif reg in (4, 5):
            ins.kind = JMP_IND
        elif reg == 7:
            ins.kind = BAD
    elif op == 0xFE and reg > 1:
        ins.kind = BAD
    elif op in (0x9A,):
        ins.kind = CALL_IND
    elif op in (0xEA,):
        ins.kind = JMP_IND
    return ins


# Two-byte opcodes (after 0x0F) that take a ModRM, and those that also take
# an immediate byte.
_0F_MODRM = set()
_0F_MODRM.update({0x00, 0x01, 0x02, 0x03, 0x0D})
_0F_MODRM.update(range(0x10, 0x18))
_0F_MODRM.update(range(0x18, 0x20))
_0F_MODRM.update(range(0x20, 0x24))
_0F_MODRM.update(range(0x28, 0x30))
_0F_MODRM.update(range(0x40, 0x50))
_0F_MODRM.update(range(0x50, 0x77))
_0F_MODRM.update(range(0x78, 0x80))
_0F_MODRM.update(range(0x90, 0xA0))
_0F_MODRM.update({0xA3, 0xA4, 0xA5, 0xAB, 0xAC, 0xAD, 0xAE, 0xAF})
_0F_MODRM.update(range(0xB0, 0xC8))
_0F_MODRM.update(range(0xD0, 0x100))
_0F_MODRM.discard(0xFF)
_0F_IMM8 = {0x70, 0x71, 0x72, 0x73, 0xA4, 0xAC, 0xBA, 0xC2, 0xC4, 0xC5, 0xC6}


def _decode_0f(code, start, at, va, opsize16, addr16):
    if at >= len(code):
        return Insn(va, at - start, BAD)
    op = code[at]
    at += 1
    ins = Insn(va, 0, OTHER, opcode=0x0F00 | op)
    if 0x80 <= op <= 0x8F:
        size = 2 if opsize16 else 4
        if at + size > len(code):
            return Insn(va, at - start, BAD)
        rel = int.from_bytes(code[at:at + size], "little", signed=True)
        at += size
        ins.length = at - start
        ins.kind, ins.target = JCC, (va + ins.length + rel) & 0xFFFFFFFF
        return ins
    if op in (0x0B, 0xFF, 0xB9):
        ins.length = at - start + (0 if op == 0x0B else 1)
        ins.kind = STOP if op == 0x0B else BAD
        return ins
    if op in _0F_MODRM:
        got = _modrm(code, at, addr16)
        if got is None:
            return Insn(va, at - start, BAD)
        used, reg, absolute, table, _ = got
        at += used
        ins.reg = reg
        if absolute is not None:
            ins.refs.append(absolute)
        if table is not None:
            ins.refs.append(table)
        if op in _0F_IMM8:
            at += 1
    ins.length = at - start
    return ins


def decode_run(code: bytes, va: int):
    """Decode linearly from the start of `code`, which sits at `va`."""
    at = 0
    while at < len(code):
        ins = decode(code, at, va + at)
        yield ins
        at += max(1, ins.length)
