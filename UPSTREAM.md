# Upstream tracking

## Reference baseline

| Source | Role | Baseline |
| --- | --- | --- |
| [MythicSim Go engine](https://github.com/sage3648/mythicsim-forever-engine-go) | Fixtures and live reference | `2fe7678a0844a6d19f3e6a4ec1469e542763710b`, on community base `b7083fac1ec8cc88bfbba0518dba3e0b5e71f8e1` |
| [Community Forever engine](https://github.com/ElliotWood/Forever) | Changes to review for applicability | Adopted base `f4b776b4f41d5c7799b8141697a2c9e67c89d426`; reviewed through `b7083fac1ec8cc88bfbba0518dba3e0b5e71f8e1` (2026-10-16) |

Go is a reference implementation, not proof of live-game correctness. Forever can
intentionally differ from inherited Classic behavior. Fixture client build:
`1.60.1.70425`.

The pin is written once, in [upstream/sources.json](upstream/sources.json). The Rust
build script, the Python tools and the Go helpers they build (through `-ldflags -X`)
all read it from there. Fixtures, manifests, validation records and benchmark
snapshots keep the revision they were made with as provenance, and the checks
reject accepted fixtures whose revision or client build differs from the pin. The
matched Go kernel in `tools/matched-go` writes its accepted revision as a constant,
since it builds without the pin; the historical benchmarks keep the source digest
they were measured with.

To move the pin:

1. Change `pinned_revision`, `client_build` and, if the fork rebased,
   `community_base` in `upstream/sources.json`.
2. Run `python3 tools/prepared_v2.py repin --output <scratch>`. It re-exports every
   prepared input and Go golden at the new pin and lists each changed golden in
   `<scratch>/repin.json`. Every change is a reference behavior change to review, not
   to accept: port it to Rust until `python3 tools/prepared_v2.py compare` matches every
   case, and run `cargo test`. Re-export the Frost kernel fixtures with
   `tools/compare.py`'s oracle, check they keep their expected values, move the
   matched Go kernel's `revision` and run its `go test ./...`.
3. Re-run every recorded compatibility sweep, update `release/manifest.json`, and
   review the ledger range against the new community base. Mark the applicable changes
   the reference now includes as adopted.

The pin moved from `6823b49eb` to `20b551c6b` on 2026-10-05, the fork's merge of
community #613 to #641 plus its patch 84. 274 Go goldens changed: every aura with an
exclusive effect now reports that effect's uptime, and 28 builds changed DPS through
Mutilate (#632), Whirlwind and the warrior shout costs (#613, #614) and Piercing Ice
(#615). Rust matches all of them, and all 2439 recorded sweep variants
([record](validation/2026-10-05-reference-pin-20b551c6b-sweeps.json)).

The pin moved from `20b551c6b` to `6383c15a7` on 2026-10-06, one commit: the fork's
patch 87, where a Mongoose Bite that lands with the last Lacerating Strikes bleed still up
replaces it with its own 40% instead of a bleed that ticks for nothing, and the bleed
reports under its own id 1310536 instead of Mongoose Bite's with tag 1. The six Survival
Hunter goldens with Lacerating Strikes changed and Rust matches them; no other golden
moved, and all 2439 recorded sweep variants match
([record](validation/2026-10-06-reference-pin-6383c15a7-sweeps.json)).

The pin moved from `6383c15a7` to `74127c6c8` on 2026-10-06, two commits: the fork's
patches 88 and 89, which fix the two [reference defects](#reference-defects) below. Ignite
ignores hits on a unit that is not an enemy, so a Mage with Ignite and a Goblin Sapper Charge
no longer panics, and a pushback leaves a hardcast that finished during the spell batch
window alone. The one golden with a late pushback changed and Rust matches it; the Goblin
Sapper Fire Mage is accepted with Go goldens; no other golden moved
([record](validation/2026-10-06-reference-pin-74127c6c8-sweeps.json)).

The pin moved from `74127c6c8` to `cd7d44aec` on 2026-10-07, the fork's merge of community
#642 to #676 (69 commits, 35 of them changelog, leaderboard or data bookkeeping) and its
patches 85 to 96. Patches 93 and 94 are the Ignite and pushback fixes Rust already
followed, which were 88 and 89 at the old pin. 342 of the 343 Go goldens changed. Every
golden with damage gained the hit, crit, tick and crit tick ranges the fork reports since
its patch 92, and 133 changed in nothing else; the other 209 changed in behavior:

- Mage: Missile Barrage rolls when a bolt lands, Blizzard's and Flamestrike's ticks crit,
  Blizzard's cast rolls a hit on every enemy and Arcane Concentration skips the missiles and
  the ticks, Arcane Missiles spends Arcane Blast's stacks as it starts, Pyroblast's hit takes
  the bonus as direct damage, Master of Elements reads the base cost, Presence of Mind and
  Combustion share a cooldown, and Ignite reads Can Proc From Procs from the row (#643, #646,
  #647, #656, #660, #661).
- Shaman: Windfury Weapon strikes twice with the special hits 439440 and 439441 and no extra
  attack, Lightning Overload rolls its own rows, Elemental Devastation and Flurry read Can Proc
  From Procs, and Rockbiter Weapon is an attack power effect (#644, #646, #659, #673).
- Warrior: a refreshed Deep Wounds keeps its tick timer (patch 89) and its ticks ignore the
  warrior's modifiers, Sweeping Strikes needs a second target, Unbridled Wrath gives the same rage
  for every weapon, and Charge adds no threat (#645, #653, #667).
- Hunter: trap burns, Scorpid Poison and Volley ticks crit, Volley rolls the ranged tables, a
  hawk's dive bomb ticks roll the melee special table, and the traps keep their own cooldown
  categories (#650, #657).
- Warlock: Rain of Fire's cast rolls a hit on every enemy and its ticks crit, Demonic Brand
  rolls the pet's crit, and Life Tap adds no threat (#648, #652, #672).
- Rogue: Mutilate strikes main hand first, Hemorrhage raises each Rupture tick, Preparation
  finishes every other cooldown, Hack and Slash reads the right effects and Thistle Tea restores a
  flat 100 (#654, #665). Priest: Dark Sacrifice is Undead only and adds no threat (patch 85).
- Items: Dragon's Call's whelp waits out a 45 second cooldown (patch 86).

Rust matches all of them, and so does every recorded sweep: 9,052 of the 9,076 variants
match Go, the other 24 are the multi-target tank builds Rust refuses, and none differ
([record](validation/2026-10-07-reference-pin-cd7d44aec-sweeps.json)).

The pin moved from `cd7d44aec` to `2d93e423e` on 2026-10-07, the fork's merge of community
#677 to #719 (76 commits, 35 of them changelog or leaderboard bookkeeping) and its patches 97
and 98. The fork dropped its patches 3, 76, 91, 93 and 94, which the community now carries:
the Ignite and pushback fixes Rust already followed are #702 and #701 there. 270 of the 356
Go goldens changed. 63 changed only in the uptime a Windfury Totem, the party's or cast, now
reports in the Flametongue Totem category, 18 bear goldens only in the energy regeneration
metrics and 7 Smite goldens only in the Penance ranks now registered; the other 182 changed in
behavior:

- Hunter: a hawk swings on arrival and then every 2.5 seconds hasted by real ranged haste, for
  0.35 of the dive bomb base and without a crit, so dots hasted by real haste are supported;
  pet focus refills 2.5 every 250 ms; Scorpid Poison stacks to five; Intimidation is no longer
  a major cooldown (#694, #703, #704, #713).
- Mage: Arcane Missiles' missiles keep the Arcane Blast stacks the channel spends, 15% each
  (#715).
- Druid: Hurricane's ticks crit, Moonfire applies its dot as one hit event and Thorns can be
  cast in Moonkin form (#688). Energy refills 1 every 100 ms for cats and rogues alike (#709).
- Rogue and Warrior: Cold Blood is not spent by Mutilate's hand strikes, and Unbridled Wrath
  procs from white hits only (#690, #692).
- Priest: Shadowform is a form Holy Nova's and Chastise's cast requirements refuse, and a
  helpful Holy cast such as Power Infusion no longer ends it; every Penance rank is registered;
  Holy Precision and Holy Specialization leave Chastise out (#678, #679, #686, #719).
- Shaman: Searing Totem attacks every 2.43 seconds, Flametongue Totem's hit takes no talent,
  and a Windfury Totem switches Flametongue Totem off (#677, #682, #697, patch 98).
- Warlock and Paladin: the Imp waits 0.4 seconds between Firebolts, Malediction leaves
  Hellfire alone, Demonic Embrace keeps its -1% Spirit, and Sanctified Judgement returns 10/9
  of its stated share (#680, #687, #698, #705).

Rust matches all of them, and the two fixtures accepted at the pin: a Shadow Priest whose
Shadowform refuses Holy Nova and survives Power Infusion, and the fork's patch 97, Totem of
Thunder's 1% crit on Lightning Bolt. The recorded sweeps ran again on the production Mac,
capped to two engine processes at background priority: 8,120 of the 9,076 variants, all of
which match Go, the 24 multi-target tank builds Rust refused at `cd7d44aec` among them. The
other 956 are the Warlock multi-target census past its first 480 variants and the Warlock
multi-target sweep past the first four variants of each base, generated and not compared
([record](validation/2026-10-07-reference-pin-2d93e423e-sweeps.json)).

The pin moved from `2d93e423e` to `041dec8ef` on 2026-10-12, the fork's merge of community
#720 to #763 (73 commits, 31 of them changelog or leaderboard bookkeeping) and its patches 100 to
112. The client data moved from 70245 to 70338, a caster rebalance among other things: Frostbolt's
gain per level cut from 0.5 to 0.2, Thorns 22 to 18, Retribution Aura 30 to 20, flat armor auras
moved from aura 22 to 674, "Immolation Trap Effect" renamed, Predatory Instincts renamed Natural
Instinct, Rule of Rage removed, and the Judgement of Light and Wisdom triggered ladders gained ranks.
The behavior changes preparation writes:

- Spell ranges: Mage, Warlock and Shaman spells state their client range, the point-blank area spells
  (Arcane Explosion, Blast Wave, Cone of Cold, Frost Nova) reach 10 yards, and the talents Arcane
  Geometry, Flame Throwing, Arctic Reach, Destructive Reach, Elemental Reach, Shadow Reach, Holy
  Reach and Nature's Reach extend them (#728 to #733).
- Retribution Aura reads the Holy spell power of the unit it is on, which retires the party's
  `retribution_aura_spell_power` input (#736).
- Firestone and Spellstone are weapon imbues, chosen by the new `weapon_stone` option, and a
  temporary enchant sits beside an imbue (`mh_temp_enchant_id`, `oh_temp_enchant_id`).
- Last Stand reads its 30% from client 70291's `A_MOD_INCREASE_HEALTH` aura (patch 112), Mongoose
  Bite is a main hand attack, Holy Nova's heal carries its defense type, Ice Lance's coefficient is
  .1 (#726), Howling Idol and Guardian's Dualblade are registered.

256 of the 584 Go goldens changed, 217 of them in DPS. The behavior changes the fight runtime
follows:

- A completed hard cast restarts the melee swing a full weapon speed after it ends, whether or not a
  swing came due during the cast (the fork's `castResetAt`). Paladin heals and wraths, Shaman bolts
  and druid casts hold the swing through it, and Volley holds it for its channel (#743).
- Hellfire keeps channelling at low health (#735).
- Thistle Tea may spill 15 energy, not 10 (patch 102), which moved every Rogue golden with it.
- Rake adds 5.26% of attack power to its hit and every tick, the bleed reading it at each tick
  (patch 99), and a potion no longer drops Bear or Cat Form (patch 111).
- A Tallstrider holds Dust Cloud while Sunder Armor or Expose Armor is on the target (patch 110).

Rust matches all of them: all 585 accepted fixtures with a Go golden, including the two accepted at
the pin. The recorded sweeps ran again on the production Mac, capped to two engine processes at
background priority: all 14,341 variants of 124 records, the 9,076 of the 107 records the earlier
moves reran and 5,265 of 17 records they left out. 14,278 match Go, Rust refuses 63 and none differ.
The 63 are 51 the rotation unit and tank records already refused, of the 57 they list, and 12 Mage
census presets, which the census reads from the pinned reference and which now read `unitDistance`
and run prepull actions; the 18 Windshaper Skyborne prepull variants Rust refused now match
([record](validation/2026-10-12-reference-pin-041dec8ef-sweeps.json)).

The ledger lists the 73 commits: 31 bookkeeping, 17 rotation presets, 13 client data updates, 9
behavior changes Rust prepares and 3 runtime changes the golden fixtures compare. The Mage
area fixtures stand the mage at 20 yards, outside the point-blank radius, so Go never casts Cone of
Cold or Blast Wave there; the same requests from 8 yards are accepted as
`frost-mage-3-targets-point-blank-area-lines` and `fire-mage-2-targets-point-blank-blast-wave`.

The pin moved from `041dec8ef` to `d2409ccd0` on 2026-10-13, the fork's patches 112 to 129 and
its patch 130, the Last Stand fix Rust already followed as patch 112 at the old pin (the fork
renumbered). The community base `f7598154e` and client build 70338 did not move, and the Go database
is byte for byte the same, so the game data is re-imported unchanged and the ledger has no new
commit. 223 of the 585 Go goldens changed, 221 of them in DPS: 141 of the 143 Warrior goldens, 37 of
the 39 Rogue, 33 of the 36 Bear, 10 Hunter and 2 Shaman. The behavior changes:

- Rage: a critical auto attack pays 100% extra rage, not 75%, for warriors, bears and the cat's bear
  form alike (patch 113), and the extra leaves out the two-hander's factor, so a one-hander doubles
  and a two-hander gets about 1.77 times its swing (patch 128).
- Windfury Totem procs off the main hand only, as its tooltip says, and still off a shield strike
  (patch 114). Deep Wounds from an off-hand crit is sized off the off-hand weapon (patch 116). Seal of
  Righteousness takes its spell power share from the weapon in the hand, so a swap between a
  two-hander and a one-hander changes it (patch 129).
- Items: Iceblade Hacker's and Warblade of Caer Darrow's Frost cannot miss, be dodged or parried and
  still crits (patch 112). Frost Oil is a weapon imbue, a 10% Frostbolt on each landed hit of the oiled
  hand (patch 126). Ironfoe gets two switches whose defaults keep the client's 6% on either hand
  (patch 115).
- Hunter: the Wolf casts Furious Howl (patch 119), Strider Kick speeds the hunter up 30% for 3
  seconds (patch 120), and a switch, off, makes a pet pool focus for its cooldowns (patch 118).
- Requests: `Encounter.no_hit_suppression` drops the boss's 1% melee and ranged hit suppression
  (patch 125) and `Party.battle_squawks` adds one Battle Squawk stack a party engineer, 5% melee
  attack speed each (patch 127). A speed change mid-move keeps the move's destination (patch 121).

Preparation follows all of these that the prepared state shows. The rage bar writes each hand's
swing rage and the rage a crit adds to it (the swing's rage before the two-hander's factor times the
crit multiplier of 2 less one), the attack tables take no hit suppression when the encounter asks, the
Battle Squawk aura applies Go's `math.Pow` rounding to its stacks, Strider Kick registers its aura and
active movement speed effect and names it in its effect, the Wolf has its Furious Howl auras on itself
and its owner, a `hunter_pet_furious_howl` effect and a stat aura, and an oil in an imbue or
temporary enchantment slot has its Frostbolt and one weapon proc for each oiled hand. The exporter's
trigger lists follow the narrowed Windfury Totem (a shield strike is a main hand attack to the server,
so an off hand special is heard only where no off hand weapon is wielded), and Iceblade Hacker's and
Warblade's Frost names the crit only table. A fight that outlasts Battle Squawk's four minutes, or a
pet that follows the owner's melee speed, is unrepresented. Before this move Rust prepared the two new
request fields as if they were unset, and now writes what the exporter writes for them on the
production builds of every class. A Wolf golden joins the Hunter preparation cases.

The fight runtime follows the rest: a crit pays the swing's rage plus the crit rage of its hand, so a
two-hander's bonus leaves out the 1.3 (patches 113 and 128), Windfury Totem rolls on main hand hits
only (patch 114), an off hand crit's Deep Wounds reads the off hand weapon (patch 116), Iceblade
Hacker's and Warblade's Frost rolls the crit and never misses (patch 112), Strider Kick activates its
speed aura and a speed change during a move re-plans it to the same destination (patches 120 and 121),
the Wolf casts Furious Howl while its aura is down and the hunter takes the attack power while within
15 yards of the target (patch 119), Frost Oil casts its Frostbolt at 10% on each landed hit of the
oiled hand (patch 126), the Battle Squawk stacks are held from the reset (patch 127), and the
boss's hit suppression drops out of the miss chance when the encounter asks (patch 125). The switches
whose defaults keep the old behavior (patches 115 and 118) need no change. Nine requests join the
fixtures with Go goldens: `beast-mastery-hunter-wolf`, `marksmanship-hunter-wolf-out-of-howl-range`,
`survival-hunter-strider-kick-while-moving`, `combat-rogue-frost-oil`,
`combat-rogue-frost-oil-beside-poisons`, `combat-rogue-battle-squawks`, `warrior-battle-squawks`,
`warrior-no-hit-suppression` and `survival-hunter-no-hit-suppression`.

Rust matches all of them: all 595 accepted fixtures with a Go golden, the nine new ones included, and
preparation writes exactly what the exporter writes for every one. The recorded sweeps ran again on the
production Mac, capped to two engine processes at background priority: all 14,341 variants of the 124
records the 041dec8ef rerun compared, byte for byte the same requests. 14,278 match Go, Rust refuses 63
and none differ, the counts of the 041dec8ef rerun record by record, the same 63 refused for the same
reasons. The rotation remaining sweep draws its aura reads from the base fixtures' prepared auras, and
the Survival Hunter fixtures now prepare Strider Kick, so its generator draws 10 of their variants
differently; the 10 the record lists were compared, and the 10 it now draws match Go as well
([record](validation/2026-10-13-reference-pin-d2409ccd0-sweeps.json)).

The pin moved from `d2409ccd0` to `b477c1574` on 2026-10-14, the fork's patches 131 to 134 (branch
`mythicsim/oct14-integration`, the engine production runs from that day). The community base
`f7598154e` and client build 70338 did not move, so the ledger has no new commit. 107 of the 595 Go
goldens changed, 27 of them in DPS:

- Pets: a party aura that lacks the client's `ATTR_EX_3_ONLY_TARGET_PLAYERS` reaches the party's
  summoned pets (patch 131): Mana Spring Totem, Trueshot Aura and the Atiesh auras, not the raid-wide
  buffs or the Greater Blessings, which carry it. The Warlocks' Imp regains Mana Spring's mana while it
  casts (3 goldens move in DPS), and every Priest's Shadowfiend, every Warlock's demons and the Beast
  Mastery Hunters' pets report the auras in their metrics and logs.
- Hunter: a Mongoose Bite on a running Lacerating Strikes bleed rolls what the bleed still owes into
  the new one and keeps its tick timer, as Forever's Deep Wounds does (patch 132). 11 Survival goldens
  move, a melee weave wearing Blackhand's Breadth through patch 134 as well.
- Items: 13 items take their client equip spells' typed hit and crit or the planner's stats (patch 134):
  Blackhand's Breadth's 1% melee and ranged crit and Eye of the Beast's 1% spell crit replace 2% to every
  pool, Sulfuras gains 12 Strength and 12 Stamina, Thunderfury 5 Agility and 8 Stamina. 13 more goldens
  move.
- Shaman: Rockbiter, Flametongue and a main hand stone follow the weapon in the hand at an item swap,
  and the request gains `ConsumesSpec.mh_imbue_swap_id` (patch 133). Rust refuses item swapping, so
  this changes nothing it prepares; the field is read nowhere without one, as in Go.

Preparation follows all of it: the pet party auras, the item data the import brings, and the field.
The exporter now describes what a pet's party aura adds that it used to refuse or leave out: an inert
pet's permanent aura whose exclusive effects are each alone in their category, such as Mana Spring
Totem's MP5 on a Warlock's other demons, reports their uptime with the aura's (`exclusive_effects`);
a dynamic pet's dismissal line tracks a permanent aura's stats; and any other pet logs its stats with
its permanent auras still up when it is dismissed at the reset or during a fight
(`active_dismiss_log`), since Go's `expireAll` leaves them up, while a Shadowfiend still out at the
fight's end logs them expired (`fight_end_stats`). The fight runtime follows Lacerating Strikes'
roll-over and reports and logs the pets' auras.

Rust matches all of them: all 594 accepted fixtures with a Go golden, and preparation writes exactly
what the exporter writes for every one of the 595. The recorded sweeps ran again on the production Mac,
four engine processes at background priority: all 14,341 variants of the 124 records the d2409ccd0
rerun compared, byte for byte the same requests. 14,278 match Go, Rust refuses 63 and none differ, the
counts of the d2409ccd0 rerun record by record, the same 63 refused for the same reasons. Two Shadowfiend
variants that end with the pet out differed at first, the fight's end logging Mana Spring Totem's MP5,
until the Shadowfiend's `fight_end_stats`
([record](validation/2026-10-14-reference-pin-b477c1574-sweeps.json)).

The pin moved from `b477c1574` to `0025d7398` on 2026-10-15, one commit: the fork's patch 135 (branch
`mythicsim/oct14-integration`), weapon skill. The community base `f7598154e` and client build 70338 did not
move, so the ledger has no new commit. Forever keeps Classic's weapon skill on items, now as item stats 90 to
103, and on set bonuses, which the Go engine dropped: the boss table was fixed at 300 skill. Patch 135 ports
wowsims/classic's table (`sim/core/weapon_skill.go`):

- Skill above the 5 a level every character has is kept per skill line as PseudoStats 28 to 41, summed over
  the equipment and skill auras for each hand by the type of its weapon (two-handed and one-handed lines
  apart, fist weapons as unarmed); druids and pets keep none. An attack takes its hand's skill by proc mask,
  main hand, off hand or ranged, and any other attack attacks at base.
- Against an enemy at or above the attacker's level, the gap between its defense and the skill sets the miss
  chance (5% and 0.1% a point, 0.2% past a 10 point gap, 0.04% a point off past the defense), the dodge
  chance (0.1% a point), the hit suppression (0.2% a point past 10, none under `no_hit_suppression`) and the
  share a glance keeps (between max(min(1.3 - 0.05 gap, 0.91), 0.01) and max(min(1.2 - 0.03 gap, 0.99),
  0.2)). Glancing chance, parry and crit suppression stay on base skill, and without skill every lookup is
  the table's own field.
- Items: 16 items carry their skill in the Go database (Death's Sting, Huge Thorium Battleaxe, Devilsaur Claws
  and the client's others; Aged Core Leather Gloves, Distracting Dagger, Edgemaster's Handguards and the
  Classic planner's others), and Defias Leather's 5 piece takes its +1 daggers.

One of the 595 Go goldens changed: `arms-warrior-sweeping-strikes`, which dual wields two swords with
Edgemaster's Handguards, from 510.9 to 560.3 DPS. Preparation imports the items' skill, keeps each hand's, and
writes, for each hand whose skill moves the table, the table an attack with that hand reads
(`melee.main_hand_skill`, `off_hand_skill` and `ranged_skill`: the skill, miss chance, hit suppression,
defender dodge and glance), as the exporter resolves them through Go's own lookups; a build without weapon
skill exports as before. The Defias Leather 5 piece is ported, though preparation still refuses the set's
humanoid bonus. The fight runtime reads an attack's hand table where there is one. Four requests join the
fixtures with Go goldens: `combat-rogue-dagger-weapon-skill` (Redfall's Aged Core Leather Gloves and
Distracting Dagger, 586.8 to 625.0 DPS against the production Combat Rogue), `arms-warrior-two-handed-axe-skill`
(a 13 point gap, past 10), and `combat-rogue-thero-shan-weapon-skill` with and without hit suppression.

Rust matches all of them: all 598 accepted fixtures with a Go golden, and preparation writes exactly what the
exporter writes for every one of the 599. The recorded sweeps ran again on the production Mac, four engine
processes at background priority: all 14,341 variants of the 124 records the b477c1574 rerun compared, byte for
byte the same requests. 14,278 match Go, Rust refuses 63 and none differ, the counts of the b477c1574 rerun
record by record, the same 63 refused for the same reasons. None of those variants wears an item with weapon
skill, so the 66 variants of the 2026-10-04 Warrior presets and gear record ran as well: all 66 match Go,
the 15 wearing Edgemaster's Handguards with both hands' tables moved
([record](validation/2026-10-15-reference-pin-0025d7398-sweeps.json)).

Item swapping is ported for production's Manual Crowd Pummeler opener (MythicSim's default for
Shockadin, Enhancement, Feral Cat and Bear, Elehance on 2 or more targets, and Retribution or Arms with
the opener ticked): a swap of the main hand alone between the request's equipment and the item swap
set, by a Paladin, Warrior, Shaman or Druid, through the rotation's `itemSwap` action and
`activeItemSwapSet` value. The exporter describes it as the `item_swap` effect, and preparation and the
fight runtime follow `SwapItems` with every callback the slot registers: weapon specializations, item
and enchant procs, Crusader's buffs, the Pummeler's use, the shaman imbues and main hand stones of
patch 133, Windfury Totem's weapon buff, Weaponmaster, the seals and the feral paws, the weapon skill of
patch 135, the equipment stats through `AddDynamicEquipStats`, the stopped swings and the global
cooldown of at least 1.5 seconds. What it does not follow is refused (`item_swap_unsupported`), such as a
swap that puts Rockbiter on or off with Spirit Weapons, which moves the threat multiplier.
Fifteen requests join the fixtures: the eight MythicSim defaults and six targeted swaps (weapon skill on
the swap weapon, dual wield, a shield, a swap while casting, one on a running global cooldown, and an Orc
with the party Windfury Totem) with Go goldens and first-fight logs, and a Spirit Weapons swap accepted as
refused. Rust matches all 612 accepted fixtures with a Go golden, and preparation writes exactly what the
exporter writes for every one of the 614. Every variant of the 0025d7398 rerun was compared again with this
engine against the Go results that rerun kept: 14,278 match, 63 are refused for the same reasons, none
differ, and the 66 weapon skill and 10 regenerated variants match as well
([record](validation/2026-10-15-item-swap-sweeps.json)).

The pin moved from `0025d7398` to `b1137248e` on 2026-10-16, one commit: the fork's patch 136 (branch
`mythicsim/racials-20261016`), a pet's crit from its master's ranged crit. The community base `f7598154e`
and client build 70338 did not move, so the ledger has no new commit. From 16 October 2026 a hunter pet's
crit is its master's Auto Shot crit, not its own Agility: it inherits PhysicalCritPercent plus the
ranged-only RangedCritPercent on the pet heartbeat, and its enable and disable callbacks add and take back
the Auto Shot spell's crit modifiers (the weapon racials' take-back until patch 137). Ferocity and
Intimidation still add on the pet.

- Both exporters accept a pet stat that several owner stats feed, checking the sum of the terms bit for
  bit, and a pet whose class states that its enable and disable callbacks only move stats. Such a pet
  exports `disable_stats`, what its `OnPetDisable` adds, which the runtime applies after the inheritance
  leaves and before the auras expire, as Go's `Disable` does; the runtime sums the terms that feed one stat
  before adding them.
- The runtime holds the owner's ranged-only crit fixed, so coverage refuses a fight that changes it.

21 of the 614 Go goldens changed, all Hunter builds with a pet, and the Hunter preparation goldens follow.
The fixtures were re-exported by the pinned exporter built for linux/arm64 under emulation: an amd64
build differs from the accepted goldens in the last bits of 591 others. Rust matches all 612 accepted
fixtures with a Go golden, and preparation writes exactly what the exporter writes for every one of the
614. The pet uptime test now allows the rotation's next evaluation, 1.1 seconds after half the fight,
where Go's log has it. The recorded sweeps were not rerun.

The pin moved from `b1137248e` to `713b825aa` on 2026-10-16, one commit: the fork's patch 137 (branch
`mythicsim/racials-20261016`), the racials of the 16 October 2026 beta development notes. The community
base `f7598154e` and client build 70338 did not move, so the ledger has no new commit; community #806 and
#809 make the same changes on the other line.

- Human Ruthless (2% crit) and Orc Savage (1%) replace the sword and axe specializations and need no
  weapon, so they no longer leave Auto Shot out; Dwarf Earth Fury replaces Mace Specialization with 2%
  crit damage, as Tauren Endurance does instead of 1% hit. The weapon racials' item swap callbacks go,
  and `Player.disable_weapon_specialization` no longer changes anything.
- Blood Fury's cooldown is 3 minutes, Shatter Curse takes 5% off magic damage (was 15%) and Wind Blessed
  is 3% haste (was 1%).
- Eureka! is an 8 second buff with no charges: its effect drops `spending_spells` and the runtime its
  stacks.
- Touch of the Grave procs 30% (20% for paladins, rogues and warriors) behind a 25 second cooldown, and
  its drain ignores every modifier.

361 of the 614 Go goldens changed, 350 in DPS, and the preparation goldens of every class follow. Rust
matches all 612 accepted fixtures with a Go golden, and preparation writes exactly what the exporter
writes for every one of the 614. Two tests move with the random stream: the hunter pet exports no
disable stats once Ruthless counts for Auto Shot, and the Cyclone test now stops where Go's `keepsTie`
hands the category to a Cyclone the clap no longer outlasts, which its new seed reaches. The recorded
sweeps were not rerun.

The pin moved from `713b825aa` to `d0bd691f0` on 2026-10-16, one commit: the fork's patch 138 (branch
`mythicsim/oct16-notes`), part one of the 16 October 2026 beta development notes: every class change
that states its numbers or rule outright and needs no new client row. The community base `f7598154e` and
client build 70338 did not move, so the ledger has no new commit; community #807, #808, #810 and #811 make
some of the same changes on the other line.

- Core: Auto Shot needs 0.5 seconds of standing still after a move. Leader of the Pack and Moonkin Aura
  reach the whole raid and stack: Leader of the Pack reads its row's crit as melee and ranged crit,
  Moonkin Aura as spell crit, through the new parse option `ReadAuraAs`.
- Paladin: Consecration's coefficient 0.042, Righteous Fury's Holy threat 90%, Shield Specialization 4%
  mana and Champion of the Light 10/20/30%. Reckoning banks up to 4 charges that the next main hand auto
  spends on one extra attack each, through an always-up spender aura.
- Hunter: the pet inherits its owner's physical crit plus the higher of the Auto Shot's ranged-only crit
  and the swing's melee-only crit, an offset the owner keeps, and its Expertise; a pet stat inheritance
  takes the simulation, and the exporters probe it on a separate reset copy. Resourcefulness reduces
  Strider Kick's cost and hears only melee and ranged ability crits.
- Priest: Devouring Plague 24 seconds, Shadow Word: Death 18, Vampiric Embrace 30; Twin Disciplines adds
  Shadow Word: Death, Mind Flay, Starshards and Penance, and Power in Light also waits on Shadow Word:
  Pain (`dot_spells`).
- Rogue: a Mutilate spends Cold Blood once both hands struck if either crit.
- Warlock: Grim Reach replaces Malediction, range only; Shadow Mastery is 2% a point.
- Warrior: Shield Bash and Shield Slam no longer trigger Weaponmaster's Sword Specialization.
- Druid: Faerie Fire no longer triggers Omen of Clarity in Cat or Bear Form.
- Mage: Ice Lance +200% on a frozen target; a Fire Blast crit adds 2 Heating Up stacks, and a Pyroblast
  under Presence of Mind keeps them.
- Shaman: Chain Lightning costs 35% less, Stormstrike 8% of base mana, Fire Nova's coefficient is 0.143
  and Call of Flame adds 3 seconds of Flame Shock a rank. Grace of Air no longer shares an air totem slot
  with Windfury Totem: the category goes, the party's Grace of Air stands beside the party's Windfury
  Totem, and only the party's Windfury Totem contests the shaman's own.

602 of the 614 Go goldens changed, 525 in DPS, and the preparation goldens of every class follow. Rust
matches all 612 accepted fixtures with a Go golden, and preparation writes exactly what the exporter
writes for every one of the 614. The recorded sweeps were not rerun.

The pin moved from `d0bd691f0` to `771c6349a` on 2026-10-16, two merges: the fork takes community #764
to #811 and client build 1.60.1.70374, and the community base moves to `efd2a43eb`. Where the fork's
patch 138 and the community made the same 16 October change, the community's version stands, so several
of patch 138's shapes change again. The ledger reviews the 85 new community commits.

- Data: game data and fixtures at client 70374.
- Core: a critical auto attack adds its swing's rage at 100% (#794). Retribution Aura and Thorns scale
  with 6% of their caster's spell power, on the player's own copy only (#800, #803). A dot can snapshot a
  share of melee attack power that each tick reads again, as Go's `SnapshotAttackPowerShare`.
- Racials (#806): Ruthless and Savage are base crit and Earth Fury the crit damage multiplier alone, with
  no auras.
- Paladin (#792, #810): Reckoning's charges sit on one aura behind the triggers' shared 1.5 second
  cooldown, and the next main hand auto spends them on instant main hand swings that leave the swing timer
  alone; the always-up spender aura goes. Improved Seal of Fury's and Shield Specialization's mana makes
  no threat (#801).
- Priest: Shadow Weaving is a debuff on each target, read by a damage taken modifier (#777); Inner Focus's
  crit no longer reaches dot ticks (#793); Silent Resolve reaches every spell (#811).
- Mage: Fingers of Frost rolls at Frostbolt's and Frostfire Bolt's cast end (#776); every Fire Blast cast
  adds a Heating Up stack through its own trigger, and only a Pyroblast with a cast time spends them
  (#808).
- Rogue: Instant Poison adds 0.5% of attack power and Deadly Poison 0.45% a stack a tick (#781); the
  off-hand strike always spends Cold Blood (#785).
- Warlock: Siphon Life's ticks crit (#782), the Succubus carries 6.2% spell crit (#790), and the Imp's
  Demonic Brand hit has normal threat (#802).
- Druid: Rake takes 5% of attack power and its hit ignores resists (#772), Swipe 5% (#787), Bear Form's
  threat is 50% (#798), and Shifting Power cannot be cast at full energy (#805).
- Hunter: Improved Tracking counts a target of no set type (#774).
- Shaman: Maelstrom Weapon procs at 10 PPM (#775), and Fire Nova never triggers Elemental Devastation
  (#789).
- Warrior: Blood Craze no longer triggers off Bloodthirst (#797).

486 of the 614 Go goldens changed, 242 in DPS, and the preparation goldens of every class follow. Rust
matches all 612 accepted fixtures with a Go golden, and preparation writes exactly what the exporter
writes for every one of the 614. The recorded sweeps were not rerun.

The pin moved from `771c6349a` to `1007c66bf` on 2026-10-16, three commits: the fork's patches 139
to 141 (branch `mythicsim/oct16-evening`), from in-game beta testing on 16 October. The community
base `efd2a43eb` and client build 70374 did not move, so the ledger has no new commit.

- Mage (patch 139): a Fingers of Frost charge is spent when the consuming spell lands, ahead of the
  landing's own damage and procs, so an instant Ice Lance while a Frostbolt flies on the last charge
  still sees it. A bolt that procced at launch still spends none.
- Shaman (patch 140): Earth Shock takes Stormstrike's bonus without spending a charge; only Lightning
  Bolt, Chain Lightning and their overloads spend it.
- Shaman (patch 141): Windfury Weapon's main-hand strike (439440) procs Flametongue Totem as the main
  hand's autos do; the exported trigger spells list it.

51 of the 614 Go goldens changed, 22 in DPS, and the Shaman preparation goldens with an Enhancement build follow. Rust
matches all 612 accepted fixtures with a Go golden, and preparation writes exactly what the exporter
writes for every one of the 614. The recorded sweeps were not rerun.

The pin moved from `1007c66bf` to `5af130344` on 2026-10-16, two commits: the fork's merge of
community #813 and #814 (client build 1.60.1.70425 and Wowhead data) and its patch 142, the 70425
client data with the 16 October talent trees and redesigns. The community base moves to `b7083fac1`;
the ledger reviews the two commits.

- Data: game data at client 70425; the buff tables take Mana Tide Totem rank 4. A total stat
  percentage names its stats in the stat mask, which wins when set.
- Trees: Mage 19/17/19 and Priest 18/17/17, and the Shaman and Warlock trees moved. Every talent
  string in the repository is remapped field by field, as the fork remapped its own: renamed
  talents keep their points, and the points of removed or capped ones go where patch 142 put them.
- Mage: Arcane Blast's stacks give 15% of Intellect as spell damage each, a stacking stat aura, for
  15 sec from the first stack, and nothing spends them. Arcane Wrath is new. Missile Barrage has two
  ranks and procs off a missile and Arcane Wrath too. Fire Power counts the mage's Fire effects on the
  target, up to four, through a damage done by caster handler the exporters now describe. Wake of Fire
  is Improved Fire Blast, and Winter's Chill raises Cone of Cold's damage.
- Priest: Improved Mind Flay ramps each tick; Dark Inspiration raises Spirit after Mind Blast or a
  full Mind Flay channel, a stat aura behind its triggers' batch window; Silent Resolve reaches Holy and
  Shadow. Shadow Affinity and Improved Fade are gone.
- Shaman: Elemental Mastery is a major cooldown that speeds Lightning Bolt and Lava Burst and cancels
  Chain Lightning's cooldown, through a new runtime cooldown modifier. Lava Burst has rank 4, and a
  rotation naming an older rank presses it through the spell's `rank_aliases`. Trial by Fire lengthens
  Magma Totem; Chain Lightning's cost and Fire Nova's coefficient come from their rows.
- Warlock, warrior and paladin: Demonic Embrace lost its Spirit penalty, Grim Reach takes
  Malediction's slot, and Bloodthrill opens the 5 sec Overpower window.
- Items: Blade of Hanna (2801) is new and runs in Rust; Lord Sakrasis' Scepter registers no effect, as
  in Go; The Green Tower follows its generated name.

329 of the 614 Go goldens changed, 276 in DPS, and the preparation goldens of every class follow. Rust
matches all 612 accepted fixtures with a Go golden, and preparation writes exactly what the exporter
writes for every one of the 614. The recorded sweeps were not rerun.

The pin moved from `5af130344` to `12ffe586e` on 2026-10-17, four commits: the fork's patches 143 to
146 (branch `mythicsim/oct16-evening`), from in-game measurements on client 70425. The community base
`b7083fac1` and client build 70425 did not move, so the ledger has no new commit.

- Paladin (patch 143): spell power shares as measured in game: Holy Shield 0.07, Holy Strike 0.286,
  Seal of Fury 0.15 a hit and 0.67 on its judgement, Judgement of Righteousness 0.70. One-Handed Weapon
  Specialization raises every school but Consecration and Retribution Aura, heals aside, through a
  custom modifier that follows the main hand; the exporter reads the swap callback from
  `followMainHand`.
- Mage (patch 144): Ice Lance spends its Fingers of Frost charge at cast, while the bolts spend theirs on
  landing.
- Shaman (patch 145): Water Shield's shared internal cooldown is a timer of its own, and the shaman's
  critical spells spend a globe too; preparation registers both, and no supported build casts it.
- Core (patch 146): Spell Piercing past the target's resistance, floored at 0, adds 1 / (5 x target
  level) of damage a point, on binary spells too, multiplying the partial resist.

174 of the 614 Go goldens changed, 134 in DPS, and the paladin, shaman and classic weapon preparation
goldens follow. Rust matches all 612 accepted fixtures with a Go golden, and preparation writes exactly
what the exporter writes for every one of the 614. The recorded sweeps were not rerun.

The pin moved from `12ffe586e` to `2fe7678a0` on 2026-10-17, one commit: the fork's patch 147 (branch
`mythicsim/oct17-traps`), measured in game on client 70425. The community base `b7083fac1` and client
build 70425 did not move, so the ledger has no new commit.

- Hunter (patch 147): Immolation Trap and Explosive Trap's explosion take 13% of the higher of ranged
  and melee attack power, with no spell power. Immolation Trap splits it over its five ticks and each
  tick reads the attack power it picked at the snapshot as it stands, as Go's
  `SnapshotAttackPowerShare` does with its ranged flag; the burning ground Explosive Trap leaves takes
  none. The exporter and Rust preparation export the share.

12 of the 641 Go goldens changed, all Survival hunter builds that lay traps, and the Hunter preparation
goldens follow. Rust matches all 639 accepted fixtures with a Go golden, and preparation writes exactly
what the exporter writes for every one of the 641. The recorded sweeps were not rerun.

## Ledger

The [upstream/](upstream/) directory records what has been reviewed and how:

| File | Content |
| --- | --- |
| [sources.json](upstream/sources.json) | Reference pin, adopted community base, last seen and last reviewed community revisions, and the fork's patch register |
| [changes.json](upstream/changes.json) | Every community commit in the reviewed range with a disposition and reason; applicable changes carry adoption state and regressions |
| [mechanics-map.json](upstream/mechanics-map.json) | Frost-scope mechanics: Go files, Rust modules, tests, port status, linked community changes and fork patch numbers |

Last seen, last reviewed and adopted are separate. Reviewing a change does not adopt
it: a deferred or pending change to a covered mechanic stays visible and Rust must
either implement it or reject the affected inputs. Audit the ledger offline, and
against a local clone of the community repository to prove the range is complete:

```sh
python3 tools/upstream.py check
python3 tools/upstream.py check --community /absolute/path/to/Forever
```

The first review covered 53 community commits after the adopted base: 48 irrelevant
to Frost scope, 4 deferred client data updates and 1 applicable fix. The pin move to
`20b551c6b` extended it to 55 commits, all now in the reference, and marked the changes
Rust covers by then as applicable and adopted. The move to `cd7d44aec` extended it to 124:
39 of the new 69 are bookkeeping, evidence notes or UI, 5 the fork already carried as its
own patches, 1 differs by design and 24 are applicable, each with the golden that regresses it.
The move to `2d93e423e` extended it to 200: 46 of the new 76 are bookkeeping, rotation presets,
evidence notes, comments or gear sets, 2 are fixes Rust already followed as the fork's patches,
2 restart the swing after a hard cast where the fork keeps its patch 17, and 26 are applicable
and adopted, 6 of them client data. The first fix,
[#622](https://github.com/ElliotWood/Forever/pull/622) (`252f57aa8`), changes how a
rotation reads an aura the character cannot have. The pinned reference drops such a
condition, so a Frost build without Fingers of Frost casts Ice Lance on every global
cooldown (about 81 casts and 211 DPS per fight, against 593 DPS for the talented
reference). The Arcane preset reads Missile Barrage without a guard, so an Arcane build
without it casts Arcane Missiles whenever the rule is reached (449.7 DPS and no Arcane
Blasts, against 396.6 DPS for the rotation as written). Until the reference adopted the
fix, Rust compiled each condition both ways and rejected the rotations where they acted
differently. Since `20b551c6b` both read a missing aura as inactive, with no stacks and
no time left. The regressions are the `frost-no-fingers`, `arcane-no-missile-barrage`
and `reference-no-missile-barrage` prepared fixtures.

## Reference defects

Bugs in the reference. When one blocks a comparison, Rust refuses the affected inputs until
the reference is fixed, since there is no Go result to match. When Go still gives a result,
Rust reproduces it, so it matches the engine production runs. Both defects below were fixed in
the fork at `74127c6c8`, as its patches 93 and 94, and in the community engine by #702 and #701,
which the fork takes in their place since `2d93e423e`.

### Ignite on the Goblin Sapper Charge's hit on the player

The engine panicked with a nil pointer dereference at `sim/mage/talents_fire.go:135`
when a Mage with Ignite throws a Goblin Sapper Charge and the charge's hit on the Mage
crits. Community commit `8fb1a2d75a`, in the adopted base, deals that hit through its own
spell with `ProcMaskSpellDamage` and School Fire. The Mage's `OnSpellHitDealt` hears it,
and Ignite's trigger (`ProcMaskSpellDamage`, `OutcomeCrit`, Fire) calls
`mage.Ignite.Dot(result.Target)` with the Mage as the target. Dots exist only on enemies,
so `dot.IsActive()` dereferences nil. The handler is unchanged on community `master` at
`b49be9e13` (2026-10-04).

- Reproduction: run the engine before `74127c6c8` on
  `fixtures/mage/prepared-v2/fire-mage-goblin-sapper.request.json`, the production Fire
  request with `goblinSapper` set. The first charge whose hit on the Mage crit panicked,
  inside `Character.newBasicExplosiveSpellConfig` (`sim/core/consumes.go:650`).
- Affected records: the Arcane and Fire Mage `goblinSapper` variants in
  `validation/2026-10-04-production-gear-swaps.json`, recorded as `go_error`.
- Report: [ElliotWood/Forever#699](https://github.com/ElliotWood/Forever/issues/699)
  (2026-10-06), with the reproduction and the trace above.
- Fixed in the fork's patch 93 (`8c34bdf376`, patch 88 and `8979ea9ac0` at the old pin), in the
  reference since `74127c6c8`: Ignite's
  trigger also requires an enemy target. Rust's Ignite ignores hits on the player, the Mage
  gate no longer refuses an Ignite build that throws the charge, and
  `fire-mage-goblin-sapper` is accepted with Go goldens. Before the fix the gate refused
  those builds, and before the guard Rust rolled the Mage's own crit into an Ignite on the
  target.

### A pushback after the hardcast completed

The "Pushback trigger" in `sim/core/character.go` checks that a hardcast is in progress
when a landed hit passes its conditions, but its handler runs a spell batch window later
and does not check again. If the hardcast completes inside that 10 ms window, the handler
still calls `Hardcast.pushBack` (`sim/core/cast.go`), which extends the finished cast's
`Expires` by up to 500 ms, and `newHardcastAction` schedules the cast again. The cast
completes a second time at once and its effect lands twice. Unlike the Ignite crash, this
did not block a comparison: Rust reproduced it in `Fight::pushback_handler` until the fix.

- Reproduction: run the engine before `74127c6c8` on
  `fixtures/mage/prepared-v2/feral-bear-druid-boomerang-pushback-after-cast.request.json`,
  a tanking Feral (bear) Druid that hardcasts Linken's Boomerang for 500 ms. The target's
  swing lands 10 ms before the cast completes. The Go log showed `Completed cast
  {ItemID: 11905}` at 0.50, then `pushed back 500ms while casting` and a second `Completed
  cast` at the same time, and the Boomerang deals two hits at 1.00.
- Report: [ElliotWood/Forever#700](https://github.com/ElliotWood/Forever/issues/700)
  (2026-10-06), proposing to re-check `Hardcast.Expires > sim.CurrentTime` in the handler.
- Fixed in the fork's patch 94 (`23f4e681e1`, patch 89 and `74127c6c84` at the old pin), in the
  reference since `74127c6c8`: the
  handler returns before the pushback roll when the hardcast has ended. Rust's
  `Fight::pushback_handler` does the same, and the
  `feral-bear-druid-boomerang-pushback-after-cast` golden now completes the cast once, with
  no pushback. A hit in the last batch window of a cast no longer pushes it back at all.

## Reconcile a fix

The [migration plan](docs/hybrid-migration-plan.md#community-fix-workflow-with-ai-assistance)
defines the AI-assisted port workflow. Record each review in the ledger above.

1. Identify the source repository and exact commit or issue.
2. Classify it as applicable, already covered, irrelevant to supported scope,
   deferred, or intentionally different for Forever.
3. Capture applicable bugs in regressions and port the behavior into Rust.
   Go preparation fixes and Rust runtime fixes may affect different layers.
4. Compare against the corrected reference and relevant game evidence.
5. Record the disposition in an issue or PR with the source link. Advance reviewed
   baselines only when the intervening range is reconciled.

Merging Go source does not update Rust mechanics. Compatible data can be imported;
code changes need translation and review. Shared formats reduce but do not eliminate
that work.

There is no automatic upstream monitor or synchronization job yet. The ledger's
reviewed range, not the fixture pin, states which community changes were reviewed.

## Reproduce the reference comparison

Requires stable Rust, Go 1.25.6 or later, Python 3, Git and protoc:

```sh
python3 tools/compare.py
python3 tools/fair_compare.py
```

The tools clone pinned Go source into ignored scratch storage. Use
`--source /absolute/path` for a local repository containing the commit. Neither
command modifies production or the supplied source checkout.
