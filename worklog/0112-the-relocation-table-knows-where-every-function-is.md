---
number: 112
title: The relocation table knows where every function is
date: 2026-09-22
area: tooling, decomp
files: tools/x86.py, tools/funcs.py, README.md, docs/formats/anim.md, docs/engine/cockpit.md, crates/hb-formats/src/anim.rs, crates/hb-fly/src/main.rs, crates/hb-render/src/cockpit.rs, crates/hb-world/src/lib.rs, crates/hb/src/main.rs
---

# 112. The relocation table knows where every function is

Every address in this project so far came out of `tools/pe.py dis`, which
disassembles linearly from wherever it is pointed. Pointed at the middle of an
instruction it prints something that looks like code, and nothing says it is
wrong. The question was how much of the binary had been read, and that tool
cannot answer it: it has no idea where a function starts or ends.

## A decoder and a function finder, by hand

`tools/x86.py` decodes 32-bit x86 far enough to know how long an instruction
is, where it sends control, and which absolute addresses it names. Run
linearly over all of `.text` it agrees with objdump on every one of 300,151
instructions except the `fwait` pairs objdump prints as one.

`tools/funcs.py` builds functions on it the usual way - recursive descent from
the entry point and every call target, following jump tables - and then does
the part that makes it trustworthy.

## The relocations

`HELLBEND.EXE` kept its `.reloc` section: 44,417 entries, one for every dword
in the image that holds an absolute address, because the loader would have to
patch each one if the image moved. The linker wrote them, so they are not a
guess. Three things fall out:

- **A pointer in data is a pointer exactly when it is listed.** Scanning `.data`
  for dwords that look like code addresses finds constants too; the listed ones
  are the real function pointers. 7,802 point into `.text`.
- **A jump table ends where its listed entries end.** Reading until a dword
  stops looking like an address runs on into the next table.
- **A listed address inside `.text` that no decoded instruction holds is code
  not yet found.** That is a completeness test, and it is what `check` runs.

Code that nothing reaches is still in the gaps between functions. The gap
search tries the first byte after the padding and each 16-byte boundary, and
believes what it finds only if the relocations agree with every instruction on
the way: a listed dword that straddles an instruction boundary rules it out.
The padding it skips was learned from the gaps, not assumed: `lea esp,[esp]`,
`lea ebx,[ebx]`, `lea ecx,[ecx]`, `mov edi,edi`, `add eax,0`, `nop`, `int3`.

The first pass left 2,312 of the 32,673 relocations inside `.text` unheld. The
last few were hand-written assembly - one reads the analogue joystick straight
off port `0x201` at `0x49ff20`, with `in` and `out` a compiler never emits.
Now none are. Every byte of the game's code range is code, a jump table,
padding, or the byte tables switches index through:

```
game     692224 bytes: code 665746, jump tables 5972, padding 18418, other 2088
relocations in .text: 32673; held by no instruction or table: 0
```

Ghidra, run once with default analysis and used only to check against, found
1,495 functions below `0x4aa000`. This finds 3,392. Of the 2,056 Ghidra
missed, 1,075 are reached through a pointer in data - actor routines, MRGL
handlers, callbacks - and 310 through an address in an instruction, which is
the reach the relocations give; 633 are in the gaps and nothing reaches them.

## What runs and what does not

With every pointer known, a function nothing calls and nothing holds a pointer
to cannot be reached. 2,681 of the 3,392 are reached from the entry point
(518 KB); **711 are not (148 KB)**. For the four of them the docs had leaned
on, a brute-force search agrees: no rel8 or rel32 branch anywhere in `.text`
lands on them, and their address appears nowhere in the file.

The dead code is the authoring side of the engine, left in:

- "Demented Boss Editor" (`0x46d180`), "Demented Part Editor" (`0x46b3d0`),
  "Demented part editor II" (`0x46b1a0`)
- the `.ASC` and `KEYFRAME.TXT` importers (`0x469d80`, `0x466930`) and the
  vertex pass behind them ("Working on vertex : %d of %d", `0x46b670`)
- writers for everything the game reads: the `.LVL` (`0x44c830`), enemies
  ("Cannot save enemy!", `0x405c80`), nav (`0x471430`), tunnels (`0x45d360`,
  `0x4624c0`), models (`0x46c240`), the ground light file (`0x48c6c0`)
- an animation debugger ("SC : %d PC : %d; Frame : %d", `0x468a50`)
- joystick calibration (`0x425110`), a save-game ("Game is saved", `0x483960`),
  and Fury3's network lobby (`furynet.ini`, `0x4369a0`)

## What that corrects

The repository cited 679 addresses in `.text`. 611 are instruction starts in
live code. The rest:

- **`0x44c830` is the `.LVL` writer, not its parser** (worklog 48). The parser
  is `0x44ba00`, which `lvl.md` already cites; the field offsets 48 read are
  the same struct either way.
- **The model rescale never runs.** `0x4664e0` is called only by the two
  importers. The game's loader (`0x46cd60` / `0x46c6c0`) takes the numbers as
  written (worklogs 19 and 56 said load time). The 32,767 in every file is the
  importer's output. `anim.md` and `anim.rs` say so now.
- **The part draw is `0x467a10`, not `0x467980`** - the latter is its
  never-called twin. Same six values to `0x42aa30`; the conclusion stands.
- **The cockpit hand is drawn at `0x420ace` inside `0x420a60`.** `0x4205b0`,
  where 96 read it, is an older copy with the same test and the same draw, and
  nothing calls it.
- **The opening camera is `0x45a740`.** `0x45a780` (worklog 110) is an operand
  inside it.

**Still unknown:** cited addresses that sit in dead code and whose live
counterpart has not been found yet - the next-weapon key `0x479ca0`
(`weapons.rs`, `simulation.md`), `0x47b9c5` (`tests/weapons.rs`), the
rasteriser's `0x482b39` and `0x484555`, the player's `0x464850`, and
`0x436260` in `combat.rs`, which is multiplayer code. And 24 citations land
inside an instruction rather than on one - `0x45a780` was one - which is what
linear disassembly from a guessed address produces.
