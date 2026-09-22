#!/usr/bin/env python3
"""Symbolic x87: follow the FPU stack through a stretch of HELLBEND.EXE and
print what each floating-point store holds, as an expression.

    tools/x87.py <hex-start> <hex-end>

The engine's physics, aiming and steering are x87 code, and reading them by
hand means keeping an eight-deep stack in your head across hundreds of lines.
This does it mechanically. It is not a decompiler:

- Straight-line only. It walks the addresses in order and prints branches and
  labels as it passes them; at a label the stack is whatever it was, which is
  right for a fall-through and may be wrong for a jump target.
- Memory is named by its operand - `[esp+0x30]` becomes `f[s+0x30]` with the
  stack pointer's movement since the start folded in, so a value stored and
  loaded back after pushes still matches. A value loaded from memory it has not
  seen stored is shown by name; initialised globals show their value.
- The arithmetic is decoded from the opcode bytes, not from objdump's mnemonic,
  so the reversed forms (fsubr, fdivr, and their popping variants) mean what
  the Intel manual says they mean.
"""

import re
import struct
import sys
import pathlib

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from pe import Image  # noqa: E402

IMAGE = Image()


def disassemble(start, end):
    for _, ins in IMAGE.objdump(start, end - start):
        if ins is not None:
            yield ins


def global_value(addr, size):
    data = IMAGE.read(addr, size)
    if len(data) != size:
        return None
    if size == 8:
        return struct.unpack("<d", data)[0]
    if size == 4:
        return struct.unpack("<f", data)[0], struct.unpack("<i", data)[0]
    return None


class Tracer:
    def __init__(self):
        self.st = []
        self.mem = {}
        self.regs = {}
        self.esp = 0  # bytes pushed since the start (esp moved down)

    def name(self, operand, kind):
        """A stable name for a memory operand."""
        m = re.search(r"(DWORD|QWORD|WORD|TBYTE) PTR (?:ds:)?(\[.*\]|0x[0-9a-f]+)", operand)
        if not m:
            return operand
        size, where = m.group(1), m.group(2)
        e = re.fullmatch(r"\[esp(?:\+(0x[0-9a-f]+|\d+))?\]", where)
        if e:
            off = int(e.group(1), 0) if e.group(1) else 0
            return f"{kind}[s{off - self.esp:+#x}]"
        if where.startswith("0x"):
            addr = int(where, 16)
            return f"{kind}[{addr:#x}]"
        return f"{kind}{where}"

    def load(self, operand, kind):
        n = self.name(operand, kind)
        if n in self.mem:
            return self.mem[n]
        m = re.search(r"PTR (?:ds:)?0x([0-9a-f]+)$", operand)
        if m:
            addr = int(m.group(1), 16)
            size = 8 if "QWORD" in operand else 4
            v = global_value(addr, size)
            if v is not None and IMAGE.off(addr) is not None:
                if size == 8:
                    return f"{v:.10g}"
                f, i = v
                return f"{i}" if kind == "i" else f"{f:.8g}"
        return n

    def push(self, e):
        self.st.insert(0, e)

    def pop(self):
        return self.st.pop(0) if self.st else "?"

    def get(self, i):
        return self.st[i] if i < len(self.st) else f"st{i}?"

    def set(self, i, e):
        while len(self.st) <= i:
            self.st.append(f"st{len(self.st)}?")
        self.st[i] = e

    def store(self, operand, kind, e, emit):
        n = self.name(operand, kind)
        self.mem[n] = e if len(e) < 400 else n
        emit(f"{n} = {e}")

    REGS = ("eax", "ebx", "ecx", "edx", "esi", "edi", "ebp")

    def operand(self, o):
        o = o.strip()
        if o in self.REGS:
            return self.regs.get(o, o)
        if "PTR" in o:
            return self.load(o, "i")
        try:
            return str(int(o, 0))
        except ValueError:
            return o

    def integer(self, mn, ops, emit):
        """Just enough integer tracking to follow values into memory."""
        parts = [p.strip() for p in ops.split(",")] if ops else []
        dst = parts[0] if parts else ""
        if mn == "mov" and len(parts) == 2:
            v = self.operand(parts[1])
            if dst in self.REGS:
                self.regs[dst] = v
            elif "PTR" in dst:
                self.mem[self.name(dst, "i")] = v
                self.mem[self.name(dst, "f")] = f"bits({v})"
            return
        if mn in ("add", "sub") and len(parts) == 2 and dst in self.REGS:
            a, b = self.regs.get(dst, dst), self.operand(parts[1])
            self.regs[dst] = f"({a} {'+' if mn == 'add' else '-'} {b})"
            return
        if mn == "neg" and dst in self.REGS:
            self.regs[dst] = f"-({self.regs.get(dst, dst)})"
            return
        if mn == "imul" and len(parts) == 1:
            self.regs["edx:eax"] = f"({self.regs.get('eax', 'eax')} * {self.operand(parts[0])})"
            return
        if mn == "shrd" and parts[:2] == ["eax", "edx"] and parts[2] == "0x10":
            self.regs["eax"] = f"fx{self.regs.get('edx:eax', '?')}"
            return
        if mn == "idiv" and len(parts) == 1:
            self.regs["eax"] = f"({self.regs.get('eax', 'eax')} / {self.operand(parts[0])})"
            return
        if mn in ("shl", "sar", "shr", "and", "or", "xor", "lea", "cdq", "inc", "dec") and dst in self.REGS:
            if mn == "xor" and len(parts) == 2 and parts[0] == parts[1]:
                self.regs[dst] = "0"
            elif mn not in ("shl", "sar", "cdq"):
                self.regs[dst] = f"{mn}({self.regs.get(dst, dst)}{', ' + parts[1] if len(parts) > 1 else ''})"
            return

    def step(self, addr, code, text, emit):
        mn = text.split()[0] if text else ""
        ops = text[len(mn):].strip()
        # Stack pointer bookkeeping.
        if mn == "push":
            self.esp += 4
            return
        if mn == "pop":
            self.esp -= 4
            return
        m = re.fullmatch(r"(sub|add)\s+esp,(0x[0-9a-f]+|\d+)", text.replace(" ", "").replace("sub", "sub ").replace("add", "add "))
        if mn in ("sub", "add") and ops.startswith("esp,"):
            v = int(ops.split(",")[1], 0)
            self.esp += v if mn == "sub" else -v
            return
        if mn.startswith("j") or mn in ("call", "ret"):
            emit(f"-- {text}")
            if mn == "call":
                # cdecl: the callee leaves st0 as a float result sometimes;
                # nothing to model. ftol (0x4abddc) pops st0 into eax.
                if "0x4abddc" in ops:
                    emit(f"   eax = ftol({self.pop()})")
            return
        if not code or code[0] < 0xD8 or code[0] > 0xDF:
            self.integer(mn, ops, emit)
            return
        op, modrm = code[0], code[1]
        reg = (modrm >> 3) & 7
        is_reg = modrm >= 0xC0
        i = modrm & 7
        m32 = lambda: self.load(ops, "f")
        m64 = lambda: self.load(ops, "d")
        arith = {0: "+", 1: "*", 4: "-", 5: "r-", 6: "/", 7: "r/"}

        def combine(a, sym, b):
            if sym == "r-":
                return f"({b} - {a})"
            if sym == "r/":
                return f"({b} / {a})"
            return f"({a} {sym} {b})"

        if op in (0xD8, 0xDC) and not is_reg:
            src = m32() if op == 0xD8 else m64()
            if reg in (2, 3):
                emit(f"   compare {self.get(0)} with {src}")
                if reg == 3:
                    self.pop()
            else:
                self.set(0, combine(self.get(0), arith[reg], src))
            return
        if op == 0xDA and not is_reg:
            src = f"int({self.load(ops, 'i')})"
            if reg in (2, 3):
                emit(f"   compare {self.get(0)} with {src}")
                if reg == 3:
                    self.pop()
            else:
                self.set(0, combine(self.get(0), arith[reg], src))
            return
        if op == 0xD8 and is_reg:
            if reg in (2, 3):
                emit(f"   compare {self.get(0)} with {self.get(i)}")
                if reg == 3:
                    self.pop()
                return
            self.set(0, combine(self.get(0), arith[reg], self.get(i)))
            return
        if op in (0xDC, 0xDE) and is_reg:
            # st(i) = st(i) op st0, with the Intel manual's register forms:
            # C0 add, C8 mul, E0 subr, E8 sub, F0 divr, F8 div.
            if op == 0xDE and modrm == 0xD9:
                emit(f"   compare {self.get(0)} with {self.get(1)} (and pop both)")
                self.pop(); self.pop()
                return
            a, b = self.get(i), self.get(0)
            kind = modrm & 0xF8
            e = {0xC0: f"({a} + {b})", 0xC8: f"({a} * {b})", 0xE0: f"({b} - {a})",
                 0xE8: f"({a} - {b})", 0xF0: f"({b} / {a})", 0xF8: f"({a} / {b})"}.get(kind)
            if e is None:
                emit(f"   ?? {text}")
                return
            self.set(i, e)
            if op == 0xDE:
                self.pop()
            return
        if op == 0xD9:
            if not is_reg:
                if reg == 0:
                    self.push(m32())
                elif reg == 2:
                    self.store(ops, "f", self.get(0), emit)
                elif reg == 3:
                    self.store(ops, "f", self.pop(), emit)
                else:
                    emit(f"   {text}")
                return
            if 0xC0 <= modrm <= 0xC7:
                self.push(self.get(i))
            elif 0xC8 <= modrm <= 0xCF:
                a, b = self.get(0), self.get(i)
                self.set(0, b)
                self.set(i, a)
            else:
                unary = {0xE0: "-({})", 0xE1: "abs({})", 0xFA: "sqrt({})", 0xFE: "sin({})", 0xFF: "cos({})"}
                if modrm in unary:
                    self.set(0, unary[modrm].format(self.get(0)))
                elif modrm == 0xE8:
                    self.push("1")
                elif modrm == 0xEE:
                    self.push("0")
                elif modrm == 0xEB:
                    self.push("pi")
                elif modrm == 0xF3:
                    x = self.pop()
                    self.set(0, f"atan2({self.get(0)}, {x})")
                elif modrm == 0xFB:
                    x = self.get(0)
                    self.set(0, f"sin({x})")
                    self.push(f"cos({x})")
                else:
                    emit(f"   ?? {text}")
            return
        if op == 0xDD:
            if not is_reg:
                if reg == 0:
                    self.push(m64())
                elif reg == 2:
                    self.store(ops, "d", self.get(0), emit)
                elif reg == 3:
                    self.store(ops, "d", self.pop(), emit)
                return
            if 0xD8 <= modrm <= 0xDF:
                self.set(i, self.get(0))
                self.pop()
            elif 0xD0 <= modrm <= 0xD7:
                self.set(i, self.get(0))
            return
        if op == 0xDB and not is_reg:
            if reg == 0:
                self.push(f"int({self.load(ops, 'i')})")
            elif reg in (2, 3):
                self.store(ops, "i", f"round({self.get(0)})", emit)
                if reg == 3:
                    self.pop()
            return
        if op == 0xDF and not is_reg:
            if reg == 5:
                self.push(f"int64({self.load(ops, 'q')})")
            elif reg == 7:
                self.store(ops, "q", f"round({self.pop()})", emit)
            return
        emit(f"   ?? {text}")


def main():
    start, end = (int(a, 16) for a in sys.argv[1:3])
    t = Tracer()
    for addr, code, text in disassemble(start, end):
        t.step(addr, code, text, lambda s: print(f"{addr:08x}  {s}"))


if __name__ == "__main__":
    main()
