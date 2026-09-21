---
number: 75
title: The keys are in the INI, and so is everything else
date: 2026-09-20
area: decomp, port, flight
files: crates/hb-formats/src/ini.rs, crates/hb-formats/tests/against_the_game.rs, crates/hb-fly/src/keys.rs, crates/hb-fly/src/main.rs, crates/hb-sim/src/weapons.rs, crates/hb-sim/tests/weapons.rs, docs/engine/overview.md
---

# 75. The keys are in the INI, and so is everything else

The port's keys were mine: arrows because arrows, `h` because HUD, `g`
because I needed a key for the reticle. The game's are in `[Control]` of
`.\system\hellbend.ini`, sixty four of them, and the defaults are readable
without the file ever existing.

`GetPrivateProfileInt` takes a default, and the engine passes each setting's
*current* value as that default (`0x42d274` on). So the defaults are just the
globals' initial values in `.data`, sitting in a row from `0x512758`. Walking
the `.text` for `mov eax, ds:<global>` followed by `push <name>` pairs every
name with its global, and the globals give the numbers.

They are set 1 scan codes, the same numbers a `.DMO` records, and they read
straight off once converted:

```
upKey 72 down 80 left 75 right 77      rollLeft 71 (home) rollRight 73 (pgup)
throttleUp 45 (X) throttleDown 44 (Z)  fireKey 57 (space)  weaponKey 33 (F)
keyCrosshair 20 (T)  keyCockpitLabel 53 (/)  keyMissileLock 47 (V)
keyBeacon 48 (B)  keyTransferWeapon 51 (,)  keyTransferShields 52 (.)
keyVulcanCannon 41 (`) then 2..11, which is 1 to 0
```

The weapon keys being a clean run of ` and 1 to 0 in table order is what
[[12]] guessed the player-facing weapon list from. It is now measured.

Three of mine were wrong: the crosshair is T and not G, the cockpit labels
are `/` and not L - L is the headlight - and `c`, which the port used for
collision, is the cloak. Two more the port never had: `-` steps back through
the weapons, and `0` is the mine.

## Previous is not next backwards

`keySelectNextWeapon` runs `0x479ca0`, which walks forward through all
thirty two rows and stops on the first with stock whose row says the key
stops there. `keySelectPrevWeapon` runs `0x47e0f6`, which does nothing of the
sort: it indexes a byte table at `0x47e254` into a jump table, and each arm
sets one specific weapon. It is a ring, written out by hand:

```
SKL - dispersion - Valkyrie - super - mine - guided MIRV - MIRV
    - cluster - Viper - cruise - Dead On - RFL20 - back to SKL
```

Twelve weapons, and anything not on it leaves the selection alone. The skip
for an empty weapon is the same as the forward key's. So the two directions
are different code in the engine because they are different *ideas*, and the
port now has both, with tests.

## In the port

`hb_formats::ini` is a small reader and the table of defaults. `hb-fly` reads
`system/hellbend.ini` if the disc or the install has one and falls back
setting by setting, then resolves each to the key the window reports.
Everything the game binds is bound; the port's own five switches - the HUD,
the cockpit, the music, collision and the level cycle - moved onto H, K, Y, G
and P, which are the codes `[Control]` leaves alone, so nothing collides.

**Still unknown:** `weaponKey` (F) and `keyInstrument` (I), which are bound
and whose handlers have not been read; `keyChangeViews` (O) and the four view
keys, which need the other three cockpit arts and a camera the port does not
have; and `keyNaviComp`, `keyMap` and the two zoom keys, which are a map
screen nobody has looked at. The afterburner is still shift here: the engine
makes it row 22 of the weapon table and selects it like a weapon, which is
not how it is flown in the port.
