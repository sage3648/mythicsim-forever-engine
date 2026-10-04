# MythicSim Forever Engine

A community-built Rust simulation engine for WoW Forever, starting with a small,
tested kernel and growing toward complete class support.

**Status: experimental.** Rust runs complete prepared fights through a class-independent
runtime that mirrors the pinned Go engine. The inventoried Frost Mage reference build,
the application's real request, reproduces the pinned Go engine exactly: every cast,
hit, crit, damage total, aura, mana flow and first-fight log line. This is one build
family, not general Mage support or a production replacement for MythicSim's Go
engine. Our goal is a purpose-built Forever engine with clear mechanics, reproducible
tests and community contributions.

## Get started

Install stable Rust, then run:

```sh
git clone https://github.com/sage3648/mythicsim-forever-engine.git
cd mythicsim-forever-engine
cargo test --locked
cargo run --locked --release -- sim --infile fixtures/static-frost-60.rust.json --trace
```

The command prints JSON containing DPS statistics, spell outcomes, mana and an
optional event trace. The crate depends only on Serde and serde_json. Go is not
required to run the kernel or Rust tests.

## What works today

All 29 of the application's production reference builds, captured at application
revision 18bbcd47, run in Rust through the prepared v2 contract and match the pinned Go
engine on the whole result and the first-fight log:

| Class | Builds |
| --- | --- |
| Mage | Arcane, Fire, Frost, Frostfire |
| Druid | Balance, Feral (cat), Feral (bear) |
| Shaman | Elemental, Enhancement |
| Warlock | Affliction, Demonology, Destruction |
| Priest | Shadow, Smite |
| Rogue | Assassination, Combat, Subtlety |
| Paladin | Retribution, Shockadin, Protection, Retribution/Protection, Holy/Protection |
| Warrior | Arms, Fury, Fury/Protection, Protection |
| Hunter | Beast Mastery, Marksmanship, Survival |

Race board variants for these builds match too
([casters](validation/2026-10-04-production-race-boards.json),
[melee and ranged](validation/2026-10-04-melee-race-boards.json),
[Feral, Warlock and Rogue](validation/2026-10-04-feral-warlock-rogue-race-boards.json)), as do
randomized sweeps of each build under `validation/`; an
[integration regression](validation/2026-10-04-integration-regression.json) reran every
validation input on the merged branch. A player tanking the target takes its swings, with
crushing blows, blocks, parry haste, Chance of Death and a hardcast's avoidance drop, its
live damage taken through stances and cooldowns, slows on the target's swing timer, and
heals over time.

The original prepared contract, which the bare `sim` path still accepts, covers:

- Level 60 caster, Frostbolt 25304 and one level 60 to 63 target.
- Cast timing, GCD, projectile travel, hit, crit and binary resistance.
- Mana spending, regeneration ticks, the five-second rule and mana starvation.
- Seeded random streams, aggregate statistics and event work counters.
- Eleven prepared scenarios checked against frozen results from the pinned Go engine.

Inputs currently contain resolved stats and spell parameters prepared by Go. Full
gear import, character construction, dynamic procs, multiple spells, cooldowns and
general rotation rules remain to be implemented. Unsupported inputs are rejected.
The binary does not accept production `RaidSimRequest` payloads.

The [prepared v2 contract](docs/prepared-v2.md) describes a complete reset Go
simulation of a real character, exported from the pinned engine. Rust validates it
strictly; `forever-engine check --infile PREPARED.json` lists the mechanics an input
still needs and `forever-engine sim` runs covered inputs. The runtime reproduces Go's
event order, shared or labeled random streams, casting, mana, auras, metrics and
first-fight debug log. The complete Frost reference build, with its rotation, proc
talents, Evocation, mana gems, potions, runes, the Robe and Cold Snap, matches the
pinned Go engine exactly in counts and to 1e-9 in metrics: 590.947 DPS on the
application's request, as in the frozen production observation. The whole Go result
matches, including time to out of mana and the target's metrics, and the application's
own parsers read identical first-fight and averaged timelines from both logs.

The application's Arcane reference build, with Arcane Blast and its stacks, Arcane
Power, Presence of Mind and the Undead racial Touch of the Grave, also matches: 538.672
DPS and the first-fight log. So does its Fire reference build, with Scorch and Improved
Scorch, Fireball and Pyroblast dots, Fire Blast, Heating Up, Combustion, Ignite, Master
of Elements and the Gnome racial Eureka!: 565.163 DPS. The production application's
Frostfire hybrid build, with every Frostfire Bolt rank and the Frostfire school's rules,
matches too: 597.621 DPS. All four production Mage reference builds, captured at
application revision 18bbcd47, match the pinned Go engine on the whole result and the
first-fight log. Randomized sweeps of every build match Go on every supported variant
([Frost](validation/2026-10-03-frost-sweep.json),
[Arcane](validation/2026-10-03-arcane-sweep.json),
[Fire](validation/2026-10-03-fire-sweep.json),
[production Fire](validation/2026-10-04-production-fire-sweep.json),
[Frostfire](validation/2026-10-04-frostfire-sweep.json)). Builds where community fix #622
would change the rotation are rejected; see [UPSTREAM.md](UPSTREAM.md). A
[Mage census](validation/2026-10-04-mage-census.json) changes one thing at a time on
the four production builds: every talent at its maximum or removed, every armor, every
race, every spell Go registers cast from the top of the rotation (Arcane Explosion,
Blizzard with Improved Blizzard, Cone of Cold, Frost Nova, Flamestrike and Blast Wave
included), and the pinned reference's rotation, talent and gear presets with their
on-use trinkets and rings. 433 of 435 variants match Go; the other 2 are #622
rejections.

The production Balance Druid build is the first class beyond Mage: a Night Elf Moonkin
with Elune's Light, a prepull Moonkin Form and Wrath, every Starfire and Wrath rank,
Moonfire and Insect Swarm dots, Innervate with its spirit regeneration credited as Go
credits it, Omen of Clarity, Nature's Grace and Eclipse match the pinned Go engine at
509.502 DPS, and a [sweep](validation/2026-10-04-balance-druid-sweep.json) across every
Balance race matches on every supported variant.

The production Elemental Shaman build, a Tauren with a prepull Lightning Bolt, every
Lightning Bolt and Chain Lightning rank with Lightning Overload, Flame Shock, Lava Burst,
Fire Nova, Searing Totem and Elemental Focus, matches the pinned Go engine at 396.972 DPS.
Sweeps of [its variants](validation/2026-10-04-elemental-shaman-sweep.json) and of
[every Shaman race](validation/2026-10-04-elemental-shaman-race-sweep.json) match on all
48 variants.

The production Enhancement Shaman build, an Orc with a Crusader two-hander, Rockbiter, the
party Windfury Totem and Battle Shout, Strength of Earth and Searing Totems, Stormstrike,
Flurry, Maelstrom Weapon, Elemental Devastation and Rage of the Farseer, matches the pinned
Go engine at 621.475 DPS. Sweeps of [its variants](validation/2026-10-04-enhancement-shaman-sweep.json)
and of [every Shaman race](validation/2026-10-04-enhancement-shaman-race-sweep.json) match
on every supported variant.

Shaman builds beyond the production one are supported too: Windfury, Flametongue and
Frostbrand Weapon on either hand, with a main hand Windfury Weapon taking the party
Windfury Totem's place; the shaman's own Windfury, Grace of Air, Mana Spring, Flametongue
and Magma Totems; Lightning Shield; Frost Shock; and Enhancement's weapon sync for dual
wielding. Focused fixtures match Go, and sweeps of
[self-cast totems](validation/2026-10-04-enhancement-shaman-self-totems-sweep.json),
[dual wield imbues](validation/2026-10-04-enhancement-shaman-dual-wield-imbues-sweep.json)
and [Frostbrand with swing sync](validation/2026-10-04-enhancement-shaman-frostbrand-sync-sweep.json)
match on every supported variant.

The production Shadow Priest build matches too: an Undead priest with a prepull
Shadowform and Mind Blast, Shadow Word: Pain, Devouring Plague, Shadow Word: Death with
Early Demise, Mind Flay channels the rotation interrupts, a strict Inner Focus and Mind
Blast sequence, Shadow Weaving, Dark Sacrifice and the Shadowfiend pet that nothing
summons, at 613.319 DPS. [Sweeps](validation/2026-10-04-shadow-priest-sweep.json) of the
request, also [across races](validation/2026-10-04-shadow-priest-race-sweep.json), match
Go on every supported variant, and so does every race on the application's
[Shadow Priest race board](validation/2026-10-04-shadow-priest-race-boards.json),
including the Dwarf with Stoneform.

The production Smite Priest hybrid matches as well, at 505.456 DPS: a prepull Holy Fire, a
strict Inner Focus and Holy Fire sequence on Holy Fire's remaining time, Penance channels,
Shadow Word: Pain, Devouring Plague and every Smite rank, with Power in Light's damage
taken modifier and Searing Light. [Sweeps](validation/2026-10-04-smite-priest-sweep.json),
also [across races](validation/2026-10-04-smite-priest-race-sweep.json), match Go on all
48 variants, and so does every race on its
[race board](validation/2026-10-04-smite-priest-race-boards.json).

Priest builds beyond the production requests match too: Starshards, Holy Nova with its
self heal, and the Shadowfiend summoned during the fight, inheriting attack power at each
summon and restoring mana with its Shadow swings.
[Shadowfiend](validation/2026-10-04-shadow-priest-shadowfiend-race-sweep.json),
[Holy Nova](validation/2026-10-04-smite-priest-holy-nova-race-sweep.json) and
[Starshards](validation/2026-10-04-shadow-priest-starshards-sweep.json) sweeps match Go on
all 56 variants.

The production Marksmanship Hunter is the first build with ranged auto attacks: a Human
with no pet, Auto Shot timed by the rotation's time to the next shot, a prepull Aspect of
the Hawk and Aimed Shot, Multi-Shot and Sniper Shot over ranged hasted casts, Serpent Sting
with its ranged attack power share, Summon Hawk and its two hawks, Rapid Fire, Deadly
Aspects' Quick Shots and the party Battle Shout, at 678.423 DPS. Its
[sweep](validation/2026-10-04-marksmanship-hunter-sweep.json) matches Go on all 24
variants, including those in melee range, where the main hand swings and Go's Raptor Strike
replacement reacts before each swing, and so does a
[race sweep](validation/2026-10-04-hunter-race-sweep.json) across Human, Dwarf, Night Elf,
Orc, Troll and Tauren.

The production Beast Mastery Hunter matches at 605.988 DPS: its Cat runs in from 12 yards
on a focus bar, bites and claws, and takes Intimidation, Bestial Wrath and Frenzy. The
production Survival Hunter matches at 761.654 DPS in melee range with Raptor Strike,
Mongoose Bite and Lacerating Strikes, Wing Clip, Immolation Trap, Resourcefulness and
Expose Prey; its Dragon's Call summons the Emerald Dragon Whelp mid-fight, a guardian with
its own mana bar that hardcasts Acid Spit and holds its swing for the cast. Sweeps of each
([Beast Mastery](validation/2026-10-04-beast-mastery-hunter-sweep.json),
[Survival](validation/2026-10-04-survival-hunter-sweep.json)) and race sweeps
([Beast Mastery](validation/2026-10-04-beast-mastery-hunter-race-sweep.json),
[Survival](validation/2026-10-04-survival-hunter-race-sweep.json)) match Go on all 96
variants, and so do all 125 [one change variants](validation/2026-10-04-hunter-option-census.json) of every talent, pet family and uptime. Every pet family, with its abilities, attack speed and uptime, matches across a
[pet family sweep](validation/2026-10-04-hunter-pet-family-sweep.json), as do Rapid
Recuperation, Arcane Shot, Renataki's Charm and the Classic Hunter sets, and all 32
[upstream preset combinations](validation/2026-10-04-hunter-preset-matrix.json). Across
every class, 660 of the 789 combinations of each spec's
[upstream preset rotations, talents and gear sets](validation/2026-10-04-preset-matrix.json)
matched Go at first, and 741 match on the
[rerun](validation/2026-10-04-preset-matrix-rerun.json), with none differing: the other 48
are Mage presets rejected for community #622. Several
pets may be out at once: a Cat beside Dragon's Call's whelp matches across a
[sweep](validation/2026-10-04-survival-hunter-cat-whelp-sweep.json), and a demon beside it
on every Warlock build.

The production Feral (cat) Druid matches at 578.870 DPS: a Night Elf starting in Cat Form
with a prepull Prowl into Ravage, Shred building combo points with Blood Frenzy, Rip's bleed
reading attack power at each tick, Ferocious Bite, Shifting Power, Faerie Fire, Berserk,
Rend and Tear and Omen of Clarity off melee hits. Innervate and the mana potion drop the
form, and the cat shifts back with Furor's energy carry over, swinging the equipped weapon
while out of form. [Sweeps](validation/2026-10-04-feral-druid-sweep.json), also
[as Tauren](validation/2026-10-04-feral-druid-race-sweep.json), match Go on all 48
variants.

The production Feral (bear) Druid tank matches too, at 407.214 DPS: a Night Elf in Bear
Form for the whole fight with the target swinging at it, Rage from both sides' swings,
Enrage, Demoralizing Roar lowering the target's attack power, Maul queued on the swing,
Lacerate's stacking bleed, Primal Bite, Natural Reaction, Blood Frenzy and parry haste,
until the bear dies. [Sweeps](validation/2026-10-04-feral-bear-druid-sweep.json), also
[as Tauren](validation/2026-10-04-feral-bear-druid-race-sweep.json) and
[with every variant in melee range](validation/2026-10-04-feral-bear-druid-melee-sweep.json),
match Go on all 65 variants. The bear may also leave Bear Form, for a potion, Thistle Tea or a
caster spell, and shift back, with its stats, armor, health, threat and Rage following the form
and the target's swings reading it;
[potion shift](validation/2026-10-04-feral-bear-druid-potion-shift-race-sweep.json) and
[caster interval](validation/2026-10-04-feral-bear-druid-caster-interval-race-sweep.json) sweeps
match Go on all 32 variants, and all 94 of the application's Feral Bear gear swaps match.

The Feral Druid also matches with Rake, Barkskin and Frenzied Regeneration on their
timings, set pieces whose procs give mana, energy or Rage or refund a finisher's energy,
and feral talents taken by a Moonkin.
[Rake](validation/2026-10-04-feral-druid-rake-race-sweep.json) and
[bear defensive](validation/2026-10-04-feral-bear-druid-defensives-race-sweep.json) sweeps
match Go on all 32 variants.

The application's published race boards, the requests behind every race's production DPS
for each supported spec, are a real production corpus: all 44 match the pinned Go engine
in Rust at 10,000 iterations, and every DPS the production engine published equals the
pinned result to within 4e-12 ([record](validation/2026-10-04-production-race-boards.json)).

The application's builder starters, the 29 builds users begin Build a character from, are
another: each starter in every race it offers, 166 requests built by the application's own
builder, all match the pinned Go engine in Rust at 3,000 iterations
([record](validation/2026-10-04-builder-starters.json)). At its default race each starter
builds its production reference request, so the 137 other race variants are the new coverage.

Every choice the application offers on the 29 production requests, one at a time, is a third
corpus: potions, warlock demons and pact sacrifices, imbues, totems, preset rotations, starting
distance, fight length, seals, simulation specs, Feral form and Hunter style, plus target level,
execute proportions and facing. Of 1097 distinct requests, 1074 match the pinned Go engine in
Rust, 17 are rejected with named reasons and 6 differ only in a first-fight log line
([record](validation/2026-10-04-class-options.json)).

The production Destruction Warlock build matches too: an Undead warlock with the Imp
sacrificed before the pull, a prepull Life Tap, every Shadow Bolt rank with Improved
Shadow Bolt's debuff, Immolate, Corruption, the ramping Bane of Agony, Curse of the
Elements on the target, Conflagrate and Shadowburn with Shadow and Flame, Searing Pain
and the raid's ramped Sunder Armor, at 510.849 DPS. Its
[sweep](validation/2026-10-04-destruction-warlock-sweep.json) matches Go on all 24
variants, and a [race sweep](validation/2026-10-04-warlock-race-sweep.json) drawing
Human, Gnome with Eureka!, Orc and Undead matches on all 12.

The production Combat Rogue build matches too, at 616.136 DPS: an Undead dagger rogue
whose energy ticks run as Go's simulation task, with Backstab and Puncturing Wounds,
Slice and Dice and Eviscerate splitting their metrics by combo points, Relentless Strikes,
Ruthlessness, Adrenaline Rush, Blade Flurry, Instant and Deadly Poison, Thistle Tea,
Shadowcraft Armor's energize, Windfury Totem, Crusader, Dragonbreath Chili and the Goblin
Sapper Charge, whose hit on the player removes health through Chance of Death.
[Sweeps](validation/2026-10-04-combat-rogue-sweep.json) of the request, also
[in melee range](validation/2026-10-04-combat-rogue-melee-sweep.json) and
[across every Rogue race](validation/2026-10-04-combat-rogue-race-sweep.json), match Go
on all 72 variants.

The production Assassination and Subtlety Rogues match as well. Assassination, at 564.460
DPS, casts Mutilate's two hits with their poisoned bonus, Seal Fate and Cold Blood.
Subtlety, at 540.676 DPS, opens from a prepull Stealth with Premeditation and Ambush,
breaks Stealth to resume its swings, Vanishes back into Stealth for another Ambush,
refreshes Vanish with Preparation, and adds Initiative, Cutthroat and a Rupture bleed whose
ticks stack Thousand Cuts. Sweeps of each in melee range and across every Rogue race
([Assassination](validation/2026-10-04-assassination-rogue-sweep.json),
[its races](validation/2026-10-04-assassination-rogue-race-sweep.json),
[Subtlety](validation/2026-10-04-subtlety-rogue-sweep.json),
[its races](validation/2026-10-04-subtlety-rogue-race-sweep.json)) match Go on all 96
variants.

Real Rogue builds beyond the presets are supported too: swords and axes with Hack and Slash's
extra attacks, Wound Poison, Venom, Ghostly Strike, Hemorrhage, Garrote openers, Quietus,
Kidney Shot with Improved Kidney Shot, Expose Armor in the target's major armor category
beside the raid's Sunder Armor, and a rogue that tanks the target, whose Kidney Shot pauses
the target's swings, whose parries ready Riposte and whose Ghostly Strike dodges them. Fixtures
of each match Go, and sweeps of the
[swords build](validation/2026-10-04-combat-swords-sweep.json) (also
[across races](validation/2026-10-04-combat-swords-race-sweep.json)),
[Wound Poison](validation/2026-10-04-combat-wound-poison-sweep.json),
[Venom](validation/2026-10-04-assassination-venom-race-sweep.json),
[Quietus with Ghostly Strike, Hemorrhage and Garrote](validation/2026-10-04-subtlety-quietus-race-sweep.json)
[Kidney Shot](validation/2026-10-04-combat-kidney-shot-race-sweep.json),
[Expose Armor with the raid's Sunder Armor](validation/2026-10-04-combat-expose-armor-race-sweep.json),
[Improved Expose Armor](validation/2026-10-04-combat-improved-expose-armor-race-sweep.json),
[Kidney Shot while tanking](validation/2026-10-04-combat-kidney-shot-tank-race-sweep.json)
[Riposte while tanking](validation/2026-10-04-combat-riposte-tank-race-sweep.json) and
[Ghostly Strike while tanking](validation/2026-10-04-subtlety-ghostly-strike-tank-race-sweep.json)
match Go on every variant.
The production Retribution Paladin build is the first melee build: a Human with a
two-handed weapon twisting Seal of Command and Seal of Righteousness through Twist of
Light's Echoes, Judgement, Holy Strike, Hammer of Wrath in the execute phase, Consecration,
Vengeance, Vindication, Sanctified Judgement and Sacred Arbiter, with Crusader, the party
Windfury Totem and Battle Shout, Dragonbreath Chili and the raid's ramped Sunder Armor, at
671.848 DPS. Its [sweep](validation/2026-10-04-retribution-paladin-sweep.json) and a
[race sweep](validation/2026-10-04-retribution-paladin-race-sweep.json) across Human,
Dwarf and Undead match Go on every supported variant. The production Shockadin hybrid
matches too, at 717.022 DPS: a prepull Divine Favor and Seal of Righteousness, Judgement,
Consecration with Consecrated Ground's mark, Hammer of Wrath, Holy Strike and Holy Shock,
with the Storm Gauntlets' Nature proc, and its
[race sweep](validation/2026-10-04-shockadin-paladin-sweep.json) matches on all 24
variants.

The three production Paladin tank builds match while tanking the target: Protection at
464.302 DPS, Retribution Protection at 476.579 DPS and Holy Protection at 467.333 DPS, with
Righteous Fury's threat, Holy Shield's charges, Redoubt, Reckoning, Shield Specialization,
Iron Creed, Swift Judgement, Hammer of Wrath cast with the tank's avoidance dropped,
Lightforge Armor's Crusader's Wrath and Dense Dynamite. Race sweeps across Human, Dwarf and
Undead of [Protection](validation/2026-10-04-protection-paladin-sweep.json),
[Retribution Protection](validation/2026-10-04-ret-protection-paladin-sweep.json) and
[Holy Protection](validation/2026-10-04-holy-protection-paladin-sweep.json) match on all
72 variants.

Paladin builds beyond the presets match too: Exorcism and Holy Wrath against Undead and
other targets, every Holy Shield rank, Light's Vigil turning the next Holy Shock into its
strike and refund, Seal of the Crusader with its attack power, faster swings and judgement,
whose Holy damage debuff replaces a weaker raid Judgement of the Crusader, Infusion of
Light, the Libram of Holy Alacrity, Eye for an Eye and Pursuit of Justice. A
[talent sweep](validation/2026-10-04-paladin-talent-sweep.json) that maximizes each Paladin
talent on the Retribution and Protection builds matches on all 69 variants, and every
[libram](validation/2026-10-04-paladin-libram-sweep.json) matches on the Retribution,
Shockadin and Protection builds, 36 of 36. Race sweeps of the
[Seal of the Crusader opener](validation/2026-10-04-retribution-crusader-paladin-sweep.json),
[held seal](validation/2026-10-04-retribution-crusader-held-paladin-sweep.json),
[Exorcism](validation/2026-10-04-retribution-exorcism-paladin-sweep.json),
[Light's Vigil Shockadin](validation/2026-10-04-shockadin-vigil-paladin-sweep.json) and
[Eye for an Eye tank](validation/2026-10-04-protection-eye-for-an-eye-paladin-sweep.json)
builds match on 109 of 120 variants; the other 11 drop the Seal of Command talent the
rotation still names and are rejected for #622.

Paladin heals match too: Holy Light, Flash of Light and Holy Shock's heal on the current
target, as a plain cast heals it in Go, or on the player through `castFriendlySpell`, with
the healing crit roll, Illumination's mana, Infusion of Light, the Libram of Holy Alacrity,
the Libram of Light and Greater Blessing of Light. Templar's Bulwark fires below the
defensive health threshold or from the rotation, and its absorb shield spends itself on
the target's swings. The Goblin Sapper Charge's hit on a paladin matches as well. Below a
defensive health threshold, Dwarf Stoneform's lower physical damage taken and Orc Shatter
Curse now fire on tanks too, through the player's live school damage taken multiplier, and
the Protection Warrior's Shield Wall with them. Race sweeps of the
[Holy Light Shockadin](validation/2026-10-04-shockadin-holy-light-paladin-sweep.json),
the [self-healing Shockadin](validation/2026-10-04-shockadin-self-heal-paladin-sweep.json),
the [threshold and self-healing Protection Paladin](validation/2026-10-04-protection-bulwark-heal-paladin-sweep.json),
the [threshold Protection Paladin](validation/2026-10-04-protection-paladin-threshold-sweep.json),
and the [Protection](validation/2026-10-04-protection-warrior-threshold-sweep.json) and
[Fury/Protection](validation/2026-10-04-fury-protection-warrior-threshold-sweep.json)
Warriors with thresholds across every Warrior race match on all 144 variants. Last Stand
now fires too, raising maximum health and the health with it, and every survival cooldown
with an implemented effect follows the threshold: Barkskin and Frenzied Regeneration on the
Feral Bear, Shield Wall, Templar's Bulwark, the racials and survival potions such as the
Greater Stoneshield Potion. Race sweeps of the
[Protection Warrior](validation/2026-10-04-protection-warrior-last-stand-sweep.json) and
[Fury/Protection Warrior](validation/2026-10-04-fury-protection-warrior-last-stand-sweep.json)
with Last Stand across every Warrior race, and of the
[Feral Bear](validation/2026-10-04-feral-bear-druid-threshold-sweep.json) as a Night Elf or
Tauren, all with thresholds, match on 72 of 72 variants.

Seal of Fury, which the application offers as a Protection Paladin's primary seal, matches
too: its per-hit Holy damage, the absorb shield it grants while a shield is equipped,
Improved Seal of Fury's mana when a shield is spent, Judgement of Fury, and Twist of Light's
Echo of it on a Retribution Paladin. Race sweeps of the
[Protection](validation/2026-10-04-protection-paladin-seal-of-fury-sweep.json),
[Retribution Protection](validation/2026-10-04-ret-protection-paladin-seal-of-fury-sweep.json)
and [Holy Protection](validation/2026-10-04-holy-protection-paladin-seal-of-fury-sweep.json)
Paladins with Seal of Fury and of the
[twisting Retribution Paladin](validation/2026-10-04-retribution-paladin-seal-of-fury-twist-sweep.json)
match on 94 of 96 variants; the other two drop the Seal of Command talent the rotation
still names and are rejected for #622.

The production Fury Warrior build is the first with a rage bar: a Human dual wielding
Ironfoe in Berserker Stance, rage from white hits and from the sapper's hit on the player,
Bloodthirst, Whirlwind with Raging Blows, Execute in the execute phase, Hamstring, a
prepull Bloodrage, Death Wish, Recklessness, Deep Wounds, Flurry, Unbridled Wrath, Anger
Management, Ironfoe's extra attacks, the Mighty Rage Potion, Windfury Totem, Crusader,
Dragonbreath Chili and the Goblin Sapper Charge, at 847.464 DPS. Queued Heroic Strike and
Cleave, which replace main hand swings, match too.
[Sweeps](validation/2026-10-04-warrior-sweep.json) of the request and
[across every Warrior race](validation/2026-10-04-warrior-race-sweep.json) match Go on all
48 variants, including those that drop Expose Armor, where the warrior's own Sunder Armor
shares the target's armor category with the raid's ramp. All ten requests on the Warrior
[race board](validation/2026-10-04-warrior-race-boards.json) match.

The production Arms Warrior dances between Berserker and Battle Stance through single
aura categories and the stances' threat, damage taken and damage dealt multipliers, with
Rend, Overpower, Mortal Strike, Slam, Spearing Strike, its own Battle
Shout, Bloodthrill and Weaponmaster, at 745.862 DPS. The production Protection and
Fury/Protection Warriors tank a level 63 target: Revenge opened by blocks, dodges and
parries, Shield Slam with the block value, Thunder Clap's slow on the target's swing
timer, a prepull stance dance into Retaliation, rage from Shield Specialization and Master
of Defense, Enrage, Defiance and Blood Craze's heal over time, and for Fury/Protection
Bloodthirst, Death Wish and an action sequence into Berserker Stance and Recklessness, at
465.183 and 541.081 DPS. Sweeps of each request and across every Warrior race
([Arms](validation/2026-10-04-arms-warrior-sweep.json),
[races](validation/2026-10-04-arms-warrior-race-sweep.json);
[Protection](validation/2026-10-04-protection-warrior-sweep.json),
[races](validation/2026-10-04-protection-warrior-race-sweep.json);
[Fury/Protection](validation/2026-10-04-fury-protection-warrior-sweep.json),
[races](validation/2026-10-04-fury-protection-warrior-race-sweep.json)) match Go on all 144
variants, including the warrior's own Sunder Armor stacks when Expose Armor is dropped and
Slam without Improved Slam, which stops the swings for its cast. All 30 requests on
the three race boards
([Arms](validation/2026-10-04-arms-warrior-race-boards.json),
[Protection](validation/2026-10-04-protection-warrior-race-boards.json),
[Fury/Protection](validation/2026-10-04-fury-protection-warrior-race-boards.json)) match.
Every Warrior rotation, talent and gear preset of the pinned reference also matches
([presets and gear](validation/2026-10-04-warrior-presets-and-gear.json), 66 of 66):
Sweeping Strikes in Battle Stance, Battlegear of Might's rage on hits taken, Battlegear
of Valor's Warrior's Resolve, the Premier High Warlord's Shield Wall striking back at the
target's swings, Rivenspike's Puncture Armor on the target, and Blood Craze on the live
healing power and maximum health that on-use trinkets change.

Trinkets that change Spirit, resistances or school spell damage, and the on-use trinkets
that deal damage, match Go on 504 of 522 variants across every production request
([record](validation/2026-10-04-stat-and-damage-trinkets.json)): Spirit reaches spirit
regeneration, Life Tap and Dark Sacrifice, resistances reach the Goblin Sapper Charge's hit
on the player, and Frost or Shadow spell damage reaches spell power, all through the stat
combinations; Smokey's Lighter, Everlook Pathcarver, Infernal Lasso and Linken's Boomerang
deal their rows' hits and damage over time on the one target.

The production Affliction Warlock build matches at 541.885 DPS: a Gnome warlock with
a summoned Succubus, which the runtime simulates as its own unit with auto attacks,
mana and Lash of Pain, Corruption, Bane of Agony with Amplify Curse, Immolate and
Shadow Bolt with Nightfall's instant Shadow Trance. Its
[sweep](validation/2026-10-04-affliction-warlock-sweep.json) matches Go on all 24
variants.

The production Demonology Warlock build matches at 610.212 DPS: a Gnome warlock with
Demonic Pact keeping the sacrificed Imp's buff while the Succubus is out, Decimation in
the execute phase, Demonic Brand charges the Succubus spends for extra hits, and
Demonic Energies' share of Life Tap for the demon. Its
[sweep](validation/2026-10-04-demonology-warlock-sweep.json) matches Go on all 24
variants.

Warlock builds beyond those three match as well: a summoned Imp casting Firebolt, Siphon
Life, Bane of Doom, the Drain Life and Wrack channels hasted by cast speed, Incinerate,
Bane of Havoc, Death Coil's heal, Curse of Recklessness beside the raid's Sunder Armor, every talent at full rank, and demons that inherit their warlock's
stat changes, such as an Orc's Blood Fury, at Go's heartbeat. Sweeps of a broadened
[Affliction](validation/2026-10-04-affliction-warlock-broad-sweep.json) and
[Destruction](validation/2026-10-04-destruction-warlock-broad-sweep.json) request match
Go on all 48 variants.

Every race that can be a Mage is supported: Human, Gnome, Undead, Troll with
Berserking, Orc with Blood Fury and Shatter Curse, and High Order Skyborne with Read
Ley Line for rotations that cast it. A [race sweep](validation/2026-10-04-mage-race-sweep.json)
of all three builds, drawing races from all six, matches Go on every supported variant.

See the [kernel guide](docs/kernel.md) for the input boundary and commands.

## Repository structure

The code uses one Cargo crate with shared `core` and `mechanics` modules.
Class spells live under `src/classes/<class>/spells/`, and spec behavior under
`src/classes/<class>/specs/`. Class/spec tests mirror those domains. See the
[contributor code map](docs/contributor-guide.md) to find a mechanic or add a class.

This map groups the implemented code by responsibility:

```mermaid
flowchart TD
    Crate["Forever Engine: one Cargo crate"]
    Crate --> Entry["Entry points and orchestration<br/>lib.rs, main.rs, engine.rs"]
    Crate --> Types["Input and output types<br/>contracts.rs, contracts/prepared_v2.rs, report.rs"]
    Crate --> Shared["Shared systems"]
    Crate --> Classes["Class domains: classes/"]
    Shared --> Core["core/<br/>Events, RNG, time,<br/>fight runtime"]
    Shared --> Mechanics["mechanics/<br/>Damage, mana"]
    Shared --> Rotation["rotation.rs<br/>APL subset"]
    Classes --> Mage["mage/"]
    Mage --> Agent["agent.rs<br/>Mage runtime hooks"]
    Mage --> Spells["spells/frostbolt.rs<br/>Shared Mage spell"]
    Mage --> Frost["specs/frost.rs<br/>Prepared Frostbolt simulation"]
    Tests["tests/classes/mage/frost/<br/>Kernel and Go reference checks"] -. validates .-> Frost
```

Mage Frost is the first implemented domain. Future classes follow the same
`spells/` and `specs/` pattern as their behavior is added. The
[contribution guide](CONTRIBUTING.md#code-ownership-and-dependencies) shows how
these modules depend on each other.

## Contribute

Start with the [contributor code map](docs/contributor-guide.md),
[CONTRIBUTING.md](CONTRIBUTING.md), the [roadmap](ROADMAP.md) and
[architecture](docs/architecture.md). The [engine implementation plan](docs/hybrid-migration-plan.md)
defines the first usable release, module layout, conversion sequence and
AI-assisted upstream fix process.
Implementation, tests, mechanics evidence,
documentation and reproducible bug reports are all welcome.

The first milestone is one complete Frost build. Small changes with mechanics
evidence are more useful than broad ports without regression coverage.
The [first-build inventory](docs/first-frost-inventory.md) freezes the application's
real Frost request, required mechanics, inherited caveats and report consumers.
Audit it offline with `python3 tools/inventory.py check`.
[Open an issue](https://github.com/sage3648/mythicsim-forever-engine/issues) to coordinate
a larger contribution before starting it.

## Correctness and upstream fixes

The existing Go engine remains the comparison reference. Current fixtures pin
revision `6823b49eb8aff741f197ef36d83766ef6a218285`. Matching it demonstrates
compatibility for covered mechanics, not independent proof of live-game behavior.

Applicable fixes from community repos will require review, ports and regression
cases. See [UPSTREAM.md](UPSTREAM.md) for sources, the baseline and reconciliation.

## Benchmark evidence

The matched Go/Rust experiment covered 11 scenarios and three seeds, with 693 timed
samples per language. The median Go/Rust time ratio was **1.42x** across normal
fight scenarios; Go was faster in the three-second boundary cases. Results and
work counters matched. These are local, single-threaded Frostbolt kernel timings,
not full-engine or production speedups.

The complete Frost reference build, which matches Go exactly, runs 3000 fights in a
median 81 ms in Rust against 205 ms for the pinned Go engine (2.5x), single threaded on
one machine. Before the allocation pass Rust took 273 ms. See the
[full-build snapshot](benchmarks/2026-10-03-frost-reference.json) for samples and
limitations; `forever-engine bench --infile PREPARED_V2.json` reproduces the Rust side.
Across the three Mage reference builds Rust runs 2.1 to 2.5 times faster than the pinned
Go engine ([snapshot](benchmarks/2026-10-03-mage-references.json)).

At 10000 iterations every one of the 27 production builds runs faster than the pinned Go
engine, from 1.30x (Feral cat Druid, Subtlety Rogue) to 1.98x (Frostfire Mage) with a
median of 1.47x; before the performance pass the tanking builds ran at 0.72x to 0.92x and
the melee builds at 0.95x to 1.31x. See the
[production snapshot](benchmarks/2026-10-04-production-builds.json);
`tools/production_bench.py` reproduces it and checks the Rust results are unchanged.
A [second pass](benchmarks/2026-10-04-production-builds-second-pass.json) at a later
revision, with packed pending-queue keys, made the builds a median 4.5% faster again.

- [Fair comparison and limitations](docs/forever-rust-fair-comparison-2026-10-03.md)
- [Raw benchmark snapshot](benchmarks/2026-10-03-fair.json)
- [Original prototype report](docs/forever-rust-prototype-2026-10-03.md)

The snapshots are historical measurements imported from the MythicSim prototype.
Full reproduction requires Go, Python 3, Git and protoc in addition to Rust:

```sh
python3 tools/fair_compare.py
```

## License and acknowledgments

MIT licensed, with new project work credited to MythicSim contributors.
Upstream wowsims notices are retained for adapted portions in [LICENSE](LICENSE).
RNG behavior, combat semantics and reference tooling derive from
[wowsims Forever](https://github.com/ElliotWood/Forever) and
[MythicSim's Go fork](https://github.com/sage3648/mythicsim-forever-engine-go).
See [NOTICE.md](NOTICE.md) for provenance. This project is not affiliated with
Blizzard Entertainment or the WoW Forever server team.
