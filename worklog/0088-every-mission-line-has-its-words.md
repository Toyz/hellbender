---
number: 88
title: Every mission line has its words
date: 2026-09-21
area: decomp, audio, port
resolves: 86
files: crates/hb-sim/src/phrases.rs, crates/hb-sim/tests/mission.rs, crates/hb-render/src/level.rs, crates/hb-fly/src/main.rs, docs/engine/simulation.md
---

# 88. Every mission line has its words

[[86]] ended on "what fires each one is still open - 300 triggers spread
through the image". It is not 300 triggers. It is one lookup.

`0x505c20` has three references in the whole image. Two are the phrase player
at `0x454460`, which takes an id. The third is `0x4548a9`, and it does this:

```
mov ecx, 0x505c20
...
mov eax, [ecx+0x14]      ; the entry's sound
cmp bl, [edi]            ; against a string it was given
```

It walks the table **comparing file names**. The engine finds a phrase by the
sound it plays.

Which is the whole answer, because the level data is full of file names. A
`.NAV` point carries a completion sound and a proximity sound; a `.DEF` type
carries a destruction sound. The port has been playing all of them since
[[43]] and showing nothing, because the words are not in the level - they are
in the table, and the file name is the key.

```
comm-des.wav  ->  199  "Bion commando transports destroyed."
trp-des.wav   ->   47  "Troop transport destroyed."
```

Across the twenty three shipped missions the `.NAV` files name **432** sounds.
Every single one has an entry. Not most - all of them, which is now an
assertion rather than a hope.

So `hb-fly` looks a phrase up by name wherever it plays a sound the data
named, and puts the words in the panel [[87]] built. Blowing up a transport
says so.

## And a field I had wrong

[[86]] called the second field a duration because its values are 2.0, 4.0 and
5.0 in 16.16 and that is what a subtitle timer looks like. `0x454476`
switches on it: 1.0, 2.0, 3.0, 4.0 and 5.0 are five **kinds** of phrase with
an arm each. It is a category. How long a line stays is somewhere else, and
the port's four seconds is still the port's.

**Still unknown:** what the five kinds are - the arms at `0x4544f0`,
`0x4544fe`, `0x4545d2` and the rest are not read. And the phrases the level
data never names, which is most of the three hundred: the weapon pickups, the
hull warnings, the cloak, the countdown. Those are fired by code and each
needs its own trigger found, the slow way.
