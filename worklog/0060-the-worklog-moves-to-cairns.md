---
number: 60
title: The worklog moves to cairns
date: 2026-09-20
area: tooling, build
files: .claude/skills/worklog/SKILL.md, README.md, worklog/
---

# 60. The worklog moves to cairns

`tools/worklog.py` is gone. The log is now kept by `cairns`, which grew out of
this log's own conventions and has since moved past them - one entry per file
under `worklog/`, `WORKLOG.md` generated, `cairns.toml` holding the project's
identity and its areas. The layout is unchanged, so the 59 existing entries
needed nothing: `cairns check` passed on them as they stood.

What did need fixing was a regression I had put in myself.

## Twenty-six entries with no open questions

Every entry from 1 to 33 ends with a line beginning `**Still unknown:**`.
Every entry from 34 to 59 - all twenty-six I wrote after that - ends with a
paragraph of prose saying much the same thing in a different shape: "Not
done:", "Still out:", "What is still unread is".

That reads fine and collects into nothing. `cairns open` walks the log and
prints every unresolved question in it, and it was printing the state of the
project as of entry 33. Half the log's most useful output had quietly stopped
working, and nothing complained, because a missing convention is not a broken
one.

So each of those twenty-six now carries the line, saying what that entry left
open at the time it was written. The prose it was distilled from is left where
it was.

## The edges the old format could not record

The new front matter has `supersedes` and `resolves`, and they are not the
same thing. An entry that overturns an earlier claim supersedes it; an entry
that answers a question an earlier one left open resolves it. Nine of those
links existed in fact and not in the file:

```
46 resolves 41    the swept step, against a collision that tested one point
48 resolves 35    where a dropped powerup's ceiling comes from
51 resolves 47    the motion line's five numbers and the flags
52 resolves 51    what throws a switch
53 resolves 52    the ground quakes
56 resolves 18,19 how a part is placed relative to its parent
55 resolves 54    class 1
57 resolves 55    class 14
59 resolves 52    the switch's texture swap
```

`56 resolves 18, 19` is the one worth pointing at. Entry 18 asked how a part
is placed relative to its parent in 2026-09-17 and entry 19 asked it again
after trying and failing; the answer, eleven months of entries later in log
terms, is that there is no parent chain at all. A reader landing on 18 is now
told where that ends up, and `cairns open` no longer carries a question that
was answered.

The count says the rest: 59 open questions before the links, 50 after.

**Still unknown:** whether `cairns check` should refuse an entry with no
`**Still unknown:**` line at all, rather than accepting a log that quietly
stops collecting. Writing "nothing" is a deliberate act; leaving it out is not.
