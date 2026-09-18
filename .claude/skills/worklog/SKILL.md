---
name: worklog
description: Write an entry in the project worklog - one file per entry under worklog/, indexed by WORKLOG.md. Use whenever a unit of work finishes - a format cracked, a function decoded, a subsystem ported, a bug found and fixed, a dead end ruled out. Also use when the user says "worklog", "log this", "write it up", "/worklog".
---

# Worklog

The durable record of this project: reverse engineering Hellbender (Microsoft
/ Terminal Reality, 1996) and porting it to Rust. Code says what; the worklog
says *how we found out* and *why it is that way*. It is the primary artifact
that survives context compaction - write it for a reader who was not here.

The worklog is the narrative. `docs/` is the reference - see the `docs` skill.
A format entry says how the layout was worked out and what is still guessed; the
matching page under `docs/formats/` states the layout flatly, for someone who
only wants to write a parser.

## Where it lives

One entry per file:

```
worklog/0003-the-pod-archive-format.md       the entries, NNNN-slug.md
WORKLOG.md                                   generated index - never edit by hand
tools/worklog.py                             the only thing that touches either
```

One file per entry means the next number is a filename lookup rather than a read
of the whole log, an entry can be found by grepping front matter instead of
scrolling, and two entries written in the same session do not collide in a diff.

## Writing one

```sh
tools/worklog.py new "Short title in plain words" --area format --files "a.rs,b.rs"
```

That creates the file with its number, date and front matter filled in, and
refreshes the index. Then write the prose into it.

`--area` is one or more of:

| area | what belongs there |
| --- | --- |
| `format` | a container or record layout decoded |
| `decomp` | facts pulled out of `HELLBEND.EXE` itself |
| `engine` | game loop, entity model, simulation rules |
| `render` | rasteriser, texturing, palettes, the sky |
| `world` | terrain, levels, collision, missions |
| `flight` | ship physics, controls, weapons |
| `audio` | music, sound effects, the mixer |
| `video` | Smacker cutscenes |
| `net` | DirectPlay multiplayer |
| `ui` | menus, HUD, cockpit, fonts |
| `content` | what is in the PODs, inventories, counts |
| `port` | Rust port architecture and progress |
| `tooling` | the extractors and viewers under `tools/` |
| `build` | cargo, layout, CI |
| `test` | harnesses and fixtures |

Several may be given, comma separated - decoding a mesh and writing the viewer
is `format, tooling`, and saying so is truer than picking whichever it was
mostly.

Other commands:

```sh
tools/worklog.py next     # the number the next entry would take
tools/worklog.py index    # regenerate WORKLOG.md from the entries
tools/worklog.py check    # numbering sound, front matter complete, index current
```

Run `check` before finishing. It catches a stale index, a file filed under the
wrong number, a missing date, and an area nobody has heard of.

## When to write an entry

When a unit of work concludes:

- a file format decoded (even partially - record what is still unknown)
- a routine or subsystem reverse engineered from the original binaries
- a subsystem ported to Rust and verified
- a bug diagnosed, with the root cause
- a hypothesis tested and **disproven** - dead ends are the most valuable
  entries, because they stop the next session from re-walking them
- a decision made about the port's architecture, with the alternatives rejected
- a measurement taken - a benchmark, a size, a count - because the number is the
  thing that is hard to get again

Do not write an entry for trivial edits, formatting, or anything the diff
already explains on its own.

## Entry format

The file starts with front matter the tool wrote. Do not renumber it, and do not
edit `WORKLOG.md` to match - run `tools/worklog.py index`.

```markdown
---
number: 12
title: The .BWD mesh format
date: 2026-09-17
area: format
files: tools/bwd.py, docs/formats/bwd.md
---

# 12. The .BWD mesh format

<What was done and what was learned, in prose. Lead with the finding, not the
process. Include the concrete evidence: hex offsets, struct layouts, ordinals,
addresses, sample values, measured timings. Show the struct or the decoded
table in a fenced block when there is one.>

**Still unknown:** <what remains open, or "nothing" if closed out.>
```

Sub-headings inside an entry use `##` and are fine - the entry's own title is
the `#`.

## Rules

- The log is append-only in spirit: never rewrite or renumber an earlier entry.
  Correct one by writing a later entry that says what changed and why, and link
  to it as `[12](0012-the-bwd-mesh-format.md)`.
- Prose, not bullet soup. Bullets only for genuine lists (field tables,
  enumerated ordinals).
- No emojis anywhere in the log.
- Absolute facts over impressions. If something is a guess, label it a guess and
  say what evidence would confirm it.
- Every claim about the original game should carry a locator - a virtual
  address in `HELLBEND.EXE`, a POD entry path, or a byte offset into a decoded
  record - so it can be re-checked. `HELLBEND.EXE` is a PE loading at
  `0x00400000`; give addresses as VAs and say which image (retail 1.00 or the
  v1.02 `upgrade.exe` payload) they came from.
- Keep offsets, sizes, opcodes, ordinals and paths exact. A wrong constant in
  the log is worse than no log.
- Record the *negative* results: formats that turned out not to be what they
  looked like, encodings that failed, functions that turned out to be dead code.
- If the entry records a behaviour, there should be a test that holds it. Say
  which test, by name, so the claim and its proof are linked.
- A format entry is not finished until `docs/formats/<name>.md` exists and the
  entry links to it.
