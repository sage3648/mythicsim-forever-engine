# Raid inputs

`reference-raid-20.json` is a raid input (`docs/raid.md`) of 20 MythicSim Forever reference
builds in four parties, with the Protection Warrior (raid index 0) tanking:

| Party | Members |
| --- | --- |
| 1 | protection-warrior, fury-protection-warrior, arms-warrior, warrior, enhancement-shaman |
| 2 | combat-rogue, assassination-rogue, feral-druid, retribution-paladin, hunter |
| 3 | fire-mage, frost-mage, arcane-mage, balance-druid, elemental-shaman |
| 4 | shadow-priest, demonology-warlock, affliction-warlock, destruction-warlock, marksmanship-hunter |

Each player is the request `forever.BuildRequest` builds for the reference build
(sage3648/mythicsim `worker/forever/data/reference_builds.json` at 5dd3c00865, 7 November
2026). The raid's buffs, the tank's encounter and each party's buffs are those of the party's
first member's request, the raid's debuffs the tank's without Expose Armor. 20 iterations from
seed 7.

`two-tanks-two-healers.json` is a small raid input for the tank inputs: the Protection Warrior and
the Fury Protection Warrior of the reference raid (raid indexes 0 and 1) swapping the one boss
every 30 seconds (`tankSwapSeconds`, `tankIndex` 0 and `secondTankIndex` 1), and the healer
fixtures' Holy Priest (2) and Restoration Druid (3) assigned to them (`assignments.tankHealers`,
2 to 0 and 3 to 1). 20 iterations from seed 7. The result's `mythicsim.tankHealing` has one entry
per healer.
