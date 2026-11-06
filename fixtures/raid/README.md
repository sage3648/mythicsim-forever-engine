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
