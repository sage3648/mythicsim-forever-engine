# Spell Piercing under Forever's rules, 28 November 2026

What changed in PR #334's Spell Piercing rule, the log count behind it, the resist and armor
paths checked beside it, and the A/B against the live bundle (bundle-17f22b4).

## The rule

Spell Piercing past the target's resistance in the spell's school makes a share
s = p / (p + 8 x caster level + 19). A hit rolls it as the partial resist roll turned over:
nothing at 1 - 7.5s, a tenth more at 5s, two tenths more at 2.5s, so the average gain is s.

| | Main (live) | #334 before | #334 now |
|---|---|---|---|
| Size | p / (5 x target level) on every hit and tick, 0.317% a point at level 60 against 63 | s, rolled | s, rolled |
| Share under 1% (1 to 5 piercing at level 60) | pays | pays | nothing |
| Landed binary spells (Frostbolt, Cone of Cold, Arcane Wrath, Shadow Word: Death, Flamestrike's cast) | gain | nothing | roll |
| Damage over time ticks | gain | none, save Devouring Plague and Hellfire | none, save Devouring Plague, Hellfire, Mind Flay, Insect Swarm, Starshards, Pyroblast's and Frostfire Bolt's ticks |
| Magma Totem, Searing Totem's attack | gain | Magma none (ticks), Searing rolls | none |
| Hurricane's and Rain of Fire's waves (hits of their tick spells here) | gain | Hurricane none (binary), Rain of Fire rolls | none |

Already true before this change: Frostfire spells meet the lower of Fire and Frost resistance,
and Holy meets none and takes no piercing.

## Log count (own, 28 November 2026)

forever.warcraftlogs.com, the 300 newest public dungeon reports of zone 3000 (all 28 November,
after the 19 November change; 297 read). Bonus hits are the events matching the expression
`resisted < 0` (the events feed drops the negative field, the server filter keeps it). A caster
counts where at least 30 of their plain nukes (Fire Blast, Arcane Explosion, Blizzard, Arcane
Missiles, Scorch, Arcane Blast, Mind Blast, Lightning Bolt, Chain Lightning, Shadow Bolt,
Searing Pain, Wrath, Starfire) landed and more than 1% of them gained; a caster and mob pair with
any positive partial resist is left out. E is each caster's own nuke rate times their hits of the
spell, summed. 172 casters qualified. Pets count for their owner.

| Hits | Gained | E | Gained / E | +10% : +20% |
|---|---|---|---|---|
| Blizzard (control) | 19,858 of 72,769 | 20,162 | 0.98 | 11,726 : 8,186 |
| Arcane Explosion (control) | 14,017 of 46,702 | 13,811 | 1.01 | |
| Arcane Missiles (control) | 5,728 of 18,903 | 5,662 | 1.01 | |
| Frostbolt, binary | 2,467 of 9,787 | 2,532 | 0.97 | 1,315 : 1,154 |
| Cone of Cold, binary | 2,155 of 8,195 | 2,187 | 0.99 | |
| Frost Nova, binary | 1,268 of 5,028 | 1,242 | 1.02 | |
| Arcane Wrath, binary | 1,027 of 3,285 | 995 | 1.03 | |
| Ice Lance, binary | 509 of 1,965 | 516 | 0.99 | |
| Earth Shock, binary | 58 of 903 | 55 | 1.05 | |
| Shadow Word: Death, binary | 9 of 128 | 19 | 0.48 | (3 casters) |
| Frostfire Bolt hit | 70 of 260 | 70 | 1.00 | |
| Flamestrike, cast and ground mixed | 1,440 of 18,183 | 4,847 | 0.30 | the ground ticks take none |
| Hellfire's pulses | 762 of 8,144 | 785 | 0.97 | |
| Insect Swarm ticks | 318 of 3,245 | 317 | 1.00 | |
| Pyroblast ticks | 868 of 2,372 | 836 | 1.04 | |
| Mind Flay ticks | 215 of 1,574 | 255 | 0.84 | |
| Devouring Plague ticks | 94 of 789 | 122 | 0.77 | |
| Frostfire Bolt ticks | 52 of 219 | 59 | 0.88 | |
| Starshards ticks | 2 of 24 | 5 | | too few |
| Bane of Agony ticks | 0 of 11,402 | 1,204 | 0 | |
| Corruption ticks | 0 of 6,749 | 712 | 0 | |
| Siphon Life, Wrack ticks | 0 of 4,615, 0 of 4,329 | 523, 489 | 0 | |
| Shadow Word: Pain, Moonfire, Flame Shock, Immolate ticks | 0 of 2,356, 2,316, 1,891, 1,146 | 373, 221, 182, 116 | 0 | |
| Fireball ticks | 1 of 1,045 | 353 | 0 | |
| Drain Life ticks | 0 of 121 | 10 | 0 | |
| Hurricane | 0 of 10,951 | 1,042 | 0 | |
| Rain of Fire | 0 of 1,424 | 134 | 0 | |
| Magma Totem | 0 of 7,398 | 578 | 0 | |
| Searing Totem's attack | 0 of 1,005 | 95 | 0 | |

Casters whose nukes never gained took 4 bonuses in 5,867 Frostbolts and none on any other spell
here, so the bonus comes from piercing. Logs of 14 to 18 November (#334's first count) showed
binary spells and Mind Flay at 0: the game changed on 19 November.

**Followed without an own count:** the 1% cut (a shared combat log test on build 71139: 0 of 265
hits gained at 5 piercing, 17 of 231 at 6; I cannot read a caster's piercing from these reports),
Starshards' ticks (24 ticks), lowest resistance of a two school spell against a mob whose two
resistances differ (no such mob seen).

## Other resist and armor paths (audited, not changed)

| Path | Ours | Other evidence | Verdict |
|---|---|---|---|
| Partial resist against a resistant target | Go classic: coefficient R / (5 x caster level), quarter steps by the 0.76 / 0.21 / 0.03 thresholds, capped at 1 | Shared counts: share R / (R + 8L + 19), rolled in tenths like the bonus, no step past 75%, binary spells resist whole at the share | Left. No board target has resistance, so no board moves; only user sims against resistant mobs. Needs an own count on resistant dungeon mobs (the 28 Nov count above excludes them by design) |
| Tick and channel shares against resistance | Same coefficient as hits | Shared counts: most ticks and channel ticks meet a tenth of the share; Devouring Plague, Pyroblast, Starshards, Insect Swarm, Mind Flay, Frostfire Bolt meet it in full | The bonus side is ported (above); the resist side goes with the row before |
| Level partial resist | None under Forever (#115) | Agrees | Equal |
| Armor | 1 - A / (A + 400 + 85L), floor 25%, flat armor penetration after the ignore factor | Same formula | Equal |
| Physical damage over time | Ignores armor | Shared counts: some rows' ticks meet a tenth of the armor or all of it, read from the spell row | Left: no own count, and the bleeds on the boards (Rend, Rip, Rake, Garrote, Rupture, Deep Wounds) need a row by row check |
| Armor debuffs | Puncture Armor and Armor Shatter blocked by Faerie Fire and Curse of Recklessness (main, #367) | Agrees | Equal |

## A/B, main bundle-17f22b4 against the branch

Requests: the 28 November board set (35 boards at 120 s, 300 s, cleave and AoE, plus 10 healer
requests, 10,000 iterations, board seeds) and 427 race requests of 24 November (10,000
iterations).

**Boards: 82 of 150 identical, 68 move, all down.** Healers, physical specs and Hunters are
identical.

| Board | 120 s | 300 s | Cleave | AoE |
|---|---|---|---|---|
| fire-mage | 967.32 to 931.88 (-3.66%) | 938.82 to 904.20 (-3.69%) | -4.27% | -4.20% |
| frost-mage | 852.53 to 829.69 (-2.68%) | 818.37 to 796.45 (-2.68%) | -3.10% | -2.71% |
| arcane-mage | 961.53 to 936.83 (-2.57%) | 861.18 to 839.80 (-2.48%) | -2.63% | -2.62% |
| frostfire-mage | 867.78 to 853.70 (-1.62%) | 722.97 to 710.67 (-1.70%) | -1.80% | -1.88% |
| affliction-warlock | 977.19 to 961.93 (-1.56%) | 958.65 to 943.67 (-1.56%) | -1.56% | -1.56% |
| shadow-priest | 841.50 to 828.35 (-1.56%) | 840.83 to 827.69 (-1.56%) | -1.56% | -1.56% |
| elehance-shaman | 899.20 to 887.55 (-1.30%) | 793.92 to 784.50 (-1.19%) | -1.35% | -1.41% |
| destruction-warlock-fire | 902.38 to 890.69 (-1.30%) | 889.96 to 878.71 (-1.26%) | -1.39% | -1.56% |
| destruction-warlock | 936.93 to 925.72 (-1.20%) | 914.91 to 903.34 (-1.26%) | -1.47% | -1.72% |
| demonology-warlock-fire | 902.53 to 892.67 (-1.09%) | 897.51 to 887.79 (-1.08%) | -1.30% | -1.37% |
| elemental-shaman | 841.60 to 833.09 (-1.01%) | 822.68 to 814.31 (-1.02%) | -1.26% | -1.19% |
| balance-druid | 865.90 to 857.24 (-1.00%) | 855.28 to 847.33 (-0.93%) | -1.11% | -2.16% |
| demonology-warlock | 987.26 to 978.06 (-0.93%) | 960.15 to 949.31 (-1.13%) | -1.30% | -1.48% |
| smite-priest | 672.56 to 669.62 (-0.44%) | 646.98 to 643.91 (-0.47%) | -0.84% | -1.28% |
| retribution, shockadin, holy-protection paladin | -0.03 to -0.05% | -0.03 to -0.05% | -0.05 to -0.07% | -0.08 to -0.10% |

Why: main pays 0.317% a point on everything. The branch pays about 0.19% a point on hits only.
Frost carries 23 piercing (gear and talent): 7.3% on main, 4.4% on the branch. Affliction,
Shadow and Elehance carry 5, under the cut: they lose all of main's 1.56%. The paladins' 2
piercing is under the cut too; their small move is main's flat bonus on their non-Holy
damage.

**Races: 269 of 427 identical, 158 move**, uniform within each spec (the four Mage builds -1.6 to -3.8%,
Affliction and Shadow -1.56%, the other casters -0.4 to -1.3%, paladins under -0.05%). One
top race changes: Fire Mage, Human to Gnome (a tie on main).

## Spell Piercing stat weight, warlock references

Board requests with 30 spell damage or 10 Spell Piercing added through bonus stats, 10,000
iterations, board seeds; EP is the piercing point's DPS over the spell damage point's.

| Reference | Main 120 s / 300 s | Branch 120 s / 300 s |
|---|---|---|
| affliction-warlock (5 on gear) | 3.52 / 3.52 | 0.12 / 0.14 |
| demonology-warlock (7) | 3.09 / 3.09 | 1.19 / 1.13 |
| demonology-warlock-fire (7) | 3.03 / 3.03 | 1.18 / 1.15 |
| destruction-warlock (7) | 3.40 / 3.41 | 1.27 / 1.24 |
| destruction-warlock-fire (7) | 3.30 / 3.31 | 1.26 / 1.24 |

These deltas read main higher than the app's published 2.3 to 2.6 EP (the app's own step
sizes differ); the ratio is the point: Destruction and Demonology keep about a third of main's
weight, Affliction almost none, since its damage is ticks that take no bonus.
