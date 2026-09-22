---
number: 90
title: Twenty four packet types
date: 2026-09-21
area: decomp, net
files: docs/engine/network.md, docs/README.md
---

# 90. Twenty four packet types

Toyz: "we forgot to map the networking protocol also lol". We had. Eighty
nine entries and the multiplayer had one paragraph in the overview.

It is DirectPlay, with the three service provider DLLs on the disc beside the
game - `DPSERIAL`, `DPSOCKET`, `DPWSOCK` - and four ways in that the menu
names: IPX, TCP/IP, WinSock TCP and a modem.

## The switch

`0x434540` is the receive loop, and the shape of the protocol is one
instruction:

```
mov eax, [ebx]        ; the packet
cmp eax, 0x17
ja  unknown
jmp [eax*4 + 0x434b6c]
```

A packet's first dword is its type, there are twenty four of them, and
anything else is `Unknown packet type %d`.

Naming the arms is a matter of what each one calls or says. Type 3 calls
`0x480ee0`, which is the HUD message, so it is chat. Type 6 calls `0x4653a0`,
which takes health off the player. Type 10 goes into `0x40cb00` and out
through the explosion, so it is "this actor is dead". Type 14 plays
`taunt%d.wav` and writes `Remote Ridicule from %s`. Type 19 writes
`Killed by %s`. Type 20 loads `mine.bin`. Type 23 goes into the quake module,
which is the doors and the moving ground.

Type 13 names itself: `processPacketPlayer:won't let network tell me my own
info`. It is a player's state, and the engine refuses one about you.

Five of the twenty four - 0, 4, 12, 21 and 22 - fall straight through to the
common tail and do nothing at all. Reserved, or dropped from the shipped
build.

## The taunts are a feature

Seven of them, on F6 to F12, with `keyMultiTalk` on F5. [[75]] read those
bindings out of `[Control]` and had nothing to attach them to. They send type
14, and both ends play the sound: the sender gets `Sent Remote Ridicule #%d`
and the receiver `Remote Ridicule from %s`.

**Still unknown:** every packet's layout. This entry names what the handlers
*do*, which is not the same as reading what they are given, and none of the
twenty four has had its fields read. Nor has the session setup past the
strings, the player table, or how the actors are kept in step - which is what
types 10, 13, 15 and 23 are between them for.
