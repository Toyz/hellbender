---
name: worklog
description: Write an entry in the project worklog - one file per entry under worklog/, indexed by WORKLOG.md. Use whenever a unit of work finishes - a problem diagnosed, a subsystem built and verified, a decision made, a dead end ruled out, a measurement taken. Also use when the user says "worklog", "log this", "write it up", "/worklog".
---

# Worklog

The durable record of Hellbender. Code says what; the worklog says *how we
found out* and *why it is that way*. It is the primary artifact that survives
context compaction - write it for a reader who was not here, including yourself
in a later session.

## Where it lives

One entry per file:

```
worklog/0003-a-short-title.md    the entries, NNNN-slug.md
WORKLOG.md                            generated index - never edit by hand
```

One file per entry means the next number is a filename lookup rather than a read
of the whole log, an entry can be found by grepping front matter instead of
scrolling, and two entries written in the same session do not collide in a diff.

## Writing one

```sh
cairns new "Short title in plain words" --area format,decomp --files "a.rs,b.rs"
```

That creates the file with its number, date and front matter filled in, and
refreshes the index. Then write the prose into it.

An entry may be filed under **several areas at once** - comma separated, as
above, or by repeating `--area`. Where a piece of work genuinely sits in two,
say so; it is truer than picking whichever it was mostly.

`--area` is one or more of:

| area | what belongs there |
| --- | --- |
| `format` | a container or record layout decoded |
| `decomp` | facts pulled out of HELLBEND.EXE itself |
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
| `tooling` | the extractors and viewers under tools/ |
| `build` | cargo, layout, CI |
| `test` | harnesses and fixtures |

Other commands:

```sh
cairns next     # the number the next entry would take
cairns index    # regenerate the index
cairns open     # every unresolved question in the log
cairns check    # numbering sound, front matter complete, index current
```

Run `check` before finishing. It catches a stale index, a file whose name no
longer matches its title, a missing date, and an area nobody has heard of.

## When to write an entry

When a unit of work concludes:

- a problem diagnosed, with the root cause - not just the symptom
- a subsystem built and verified, with what verified it
- a decision made, with the alternatives that were rejected and why
- a hypothesis tested and **disproven** - dead ends are the most valuable
  entries, because they stop the next session re-walking them
- a measurement taken - a benchmark, a size, a count - because the number is
  the thing that is hard to get again
- a claim in an earlier entry overturned

Do not write an entry for trivial edits, formatting, or anything the diff
already explains on its own.

## Entry format

The file starts with front matter the tool wrote. Do not renumber it, and do not
edit the index to match - run `cairns index`.

```markdown
---
number: 12
title: A short title in plain words
date: 2026-09-20
area: format, decomp
files: src/thing.rs
supersedes: 6
---

# 12. A short title in plain words

<What was done and what was learned, in prose. Lead with the finding, not the
process. Include the concrete evidence: numbers, offsets, names, sample values,
measured timings. Show the table or the struct in a fenced block when there is
one.>

**Still unknown:** <what remains open, or "nothing" if closed out.>
```

Sub-headings inside an entry use `##` - the entry's own title is the `#`.

`**Still unknown:**` must begin a line, and it is what `cairns open` collects
across the whole log. It is the log's list of what the project does not yet
know, so it is worth writing honestly rather than leaving blank.

## Linking an entry to an earlier one

The log is append-only. Never rewrite or renumber an entry. Two front matter
fields carry everything that would otherwise tempt you to edit one, and both
take one or more entry numbers:

```sh
cairns new "What changed" --area format --supersedes 6
cairns new "What it turned out to be" --area format --resolves 6
```

**`supersedes`** - this entry overturns something an earlier one claimed. The
reader who lands on entry 6 is then told that 12 corrected it. This is the
single most valuable edge in the log, and it only exists if you record it.

**`resolves`** - this entry answers the question an earlier one left open. That
question then leaves the open-questions list, and entry 6 keeps it, struck
through, naming what closed it.

They are not interchangeable. Answering a question does not mean the entry that
asked it was wrong, and `check` will reject a `resolves` aimed at an entry that
left no question open.

## Rules

- Prose, not bullet soup. Bullets for genuine lists only - field tables,
  enumerated options.
- No emojis anywhere in the log.
- Absolute facts over impressions. If something is a guess, label it a guess and
  say what evidence would confirm it.
- Keep numbers, names and paths exact. A wrong constant in the log is worse than
  no log.
- Record the *negative* results: the thing that turned out not to be what it
  looked like, the approach that failed, the code that turned out to be dead.
- If the entry records a behaviour, there should be a test that holds it. Say
  which test, by name, so the claim and its proof are linked.

<!-- cairns:project -->

## This project

The worklog is the narrative; `docs/` is the reference - see the `docs` skill.
A format entry says how the layout was worked out and what is still guessed;
the matching page under `docs/formats/` states the layout flatly, for someone
who only wants to write a parser. **A format entry is not finished until
`docs/formats/<name>.md` exists and the entry links to it.**

Every claim about the original game carries a locator, so it can be
re-checked:

- a virtual address in `HELLBEND.EXE`, which is a PE loading at `0x00400000` -
  give addresses as VAs, and say which image (retail 1.00, or the v1.02
  `upgrade.exe` payload) they came from
- a POD entry path, as `DATA\FLOAT.QKE`
- a byte offset into a decoded record, as `+0x54`

Keep offsets, sizes, opcodes, ordinals and paths exact. A wrong constant in
the log is worse than no log.

The tools an entry is likely to cite: `tools/pe.py` (disassemble, cross
reference, read the image), `tools/x87.py` (trace the floating point stack
into an expression), `tools/mrgl.py` (walk a model's nodes), `tools/pod.py`
(the archives), `tools/docs.py` (the reference pages' index).
