---
number: 65
title: The cluster is a pair
date: 2026-09-20
area: decomp, port
files: crates/hb-sim/src/weapons.rs, crates/hb-sim/tests/weapons.rs, crates/hb-fly/src/main.rs, docs/engine/simulation.md
resolves: 44
---

# 65. The cluster is a pair

The last weapon. Worklog 44 left the cluster missile out because "its launch
fans a count this port could not pin", and the count turned out not to exist:
`0x47cec0` calls the missile launcher `0x477890` exactly twice, and the whole
function contains no loop at all. Counting the calls was the entire
investigation - `grep -c` on the disassembly of the routine.

Both are kind 25, which is the cluster itself, so firing a Legion puts two
Legions in the air. The first leaves from the ship plus half its forward row
and half the sum of its right and up rows; the second is a full right row
along from the first, which works out as half a unit either side of the nose
and half a unit up. One round covers the pair.

The name is the clue I should have taken first: "Legion", not "Cluster Bomb".
It is a salvo, not a warhead that opens.

`hb-fly` fires it on the `7` key. `the_cluster_fires_a_pair` holds that one
trigger puts two missiles up, symmetric about the nose and level with each
other, and that the stock goes down by one rather than two.

Every weapon the player has is now ported: the two lasers, the cannon, the
dispersion cannon, Dead-On, Scorcher, Viper, Legion, both MIRVs, the mine and
the Bion super weapon.

**Still unknown:** what the two random numbers the launch draws before it
fires scale - one lands between 0.75 and 0.875 and the other between 4.0 and
6.0, and both are passed into `0x477890`, whose later arguments this port
does not use. The cruise missile's own steering (`0x4780b0`) is still not
read; the port flies it like the rest.
