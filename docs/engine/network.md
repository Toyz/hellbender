---
title: Multiplayer
status: partial
covers: HELLBEND.EXE:0x434540, system/DPSERIAL.DLL, system/DPSOCKET.DLL, system/DPWSOCK.DLL, LEVELS\NETLVL1-3.LVL
worklog: 90
---

# Multiplayer

Hellbender ships with three DirectPlay service provider DLLs beside the game
and a menu that offers four ways in. Nothing of this is ported; this page is
what reading the image says about it.

## Getting in

`CreateNewDirectPlay` (`0x513998`) and the four connections the menu names:

| | |
| --- | --- |
| `WinSock IPX Connection` | `0x5138d0` |
| `Internet (TCP/IP) Connection` | `0x513940` |
| `WinSock TCP Connection` | `0x5137f4` |
| `Modem Connection`, `.\system\dpserial.dll` | `0x513864`, `0x513810` |

`DPSERIAL.DLL`, `DPSOCKET.DLL` and `DPWSOCK.DLL` are in `system/` on the
disc. The lobby's own strings are `Gathering Players - %s`,
`Waiting for %d players`, `%d. %s (host)`, `Talking to other players...%c`,
`Game cancelled by the host`, `The game was terminated by the host` and
`Player %d has wrong game version.`

Three levels are for it and are not in the [campaign](../formats/lvl.md#the-campaign):
`NETLVL1`, `NETLVL2` and `NETLVL3`, which `netlvl.ini` (`0x504d8c`) lists.

## The packets

`0x434540` is the receive loop. A packet's first dword is its type, the
switch takes 0 to 23 through a jump table at `0x434b6c`, and anything above
is `Unknown packet type %d`. Five of the twenty four arms fall straight
through to the common tail and do nothing, which is either a reserved type or
one the shipped build dropped.

| Type | Handler | What it does |
| --- | --- | --- |
| 0, 4, 12, 21, 22 | `0x434542` | nothing - falls to the tail |
| 1 | `0x4345c1` | `0x431370`, then 2's handler |
| 2 | `0x4345cf` | `0x431720` |
| 3 | `0x4345dd` | puts a line on the HUD (`0x480ee0`) |
| 5 | `0x434615` | - |
| 6 | `0x434624` | takes health off the player (`0x4653a0`) |
| 7, 8 | `0x43474d`, `0x4347b6` | - |
| 9 | `0x434806` | `bogus` - a rejected packet |
| 10 | `0x434860` | an actor was destroyed (`0x40cb00` into the explosion) |
| 11 | `0x4348fe` | `0x431220`, `0x430dd0` |
| 13 | `0x43493b` | a player's state; refuses one about yourself - `processPacketPlayer:won't let network tell me my own info` |
| 14 | `0x4349a7` | a taunt: `Remote Ridicule from %s`, plays `taunt%d.wav` |
| 15, 17 | `0x434a2f`, `0x434a3d` | a missile (`0x4795f0`) |
| 18 | `0x434a66` | a mine went off - `Bad mine exploded packet` on a short one |
| 19 | `0x434a97` | a kill: `Killed by %s`, `You killed %s` |
| 20 | `0x434af2` | a mine was laid - loads `mine.bin` and plays a sound |
| 23 | `0x434b29` | the moving geometry (`0x410ab0`, `0x410ad0`) |

The errors around them name three more shapes: `Bad packet size!`,
`Bad missile exploded packet`, `%s: buffer too small for packet`,
`%s: invalid player`, `%s: Active players exist on that session`.

The send side is `WriteTVPacket` (`0x512204` on): it can send to all
(`WriteTVPacket: send (all) [%d]`), refuses to send to yourself
(`WriteTVPacket:Send: I won't send a message to myself`) and range-checks the
destination. `ReadPacket:Receive:%d:[%d]` is the other half.

## Taunts

Seven, bound to F5 and F6 to F12 by [`[Control]`](overview.md#controls) -
`keyMultiTalk` and `keyMultiTaunt1` to `7`. A taunt sends type 14 and plays
`taunt%d.wav` at both ends; the sender sees `Sent Remote Ridicule #%d` and
the receiver `Remote Ridicule from %s`.

## Not read

What each packet carries. The handlers are named above by what they do, not
by their layout - no packet's fields have been read, and the five empty arms
have not been explained. Nor has the session setup beyond the strings, the
player table, or how a level's actors are kept in step, which is what types
10, 13, 15 and 23 are between them doing.
