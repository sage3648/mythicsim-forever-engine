# Levelling characters against public combat logs, 2026-11-09

Source: foreverlogs.gg public reports, read through the site's own JSON pages in a browser (no API
key). Data credit: foreverlogs.gg public reports. Only the derived numbers below are kept; no log,
character or player name is stored here or in the fixtures.

Short version: where the capture leaves nothing unknown, the engine's levelling numbers match the
logs. A low rank's base damage at the level, the client row's spell power coefficient on the
gear's spell damage and Garrote's attack power share reproduce the logged non-crit hits to the
point for dot ticks and within 3.5% for direct spells. The first pass left three gaps open
(Rend under the engine, Hunter shots and two Affliction Warlocks over it). A second pass the same
day, reading the aura events of the timelines, settled all three as auras, buffs and logged gear
the first comparison did not count; the engine needed no change ([The open gaps,
settled](#the-open-gaps-settled)). Tests: `tests/levels_logs_20261109.rs` and
`tests/levels_logs_gaps_20261109.rs`, fixtures `fixtures/levels/logs/`.

## What is public

- 13 public reports, 5 to 8 November 2026, client 1.60.1.70862. All are dungeon runs (Ragefire
  Chasm, Wailing Caverns, Deadmines, Shadowfang Keep, Blackfathom Deeps, The Stockade, Razorfen
  Kraul, Ruins of Lordaeron, The Hall of Thanes, City of Dalaran, Excavation Site: Wetlands). No
  report at level 36 or above.
- 1230 encounters, 323 player sources with damage events, 87 with at least 80 of them: Druid 12,
  Hunter 11, Mage 8, Paladin 12, Priest 7, Rogue 12, Shaman 7, Warlock 12, Warrior 6.
- 65 of those 87 have character captures, which give the level: levels 9 to 14 (7 characters),
  15 to 19 (19), 20 to 24 (18), 25 to 29 (12), 30 to 35 (9). Fifteen character and report pairs
  level up inside the report.
- The crawl used about 1750 requests over two 15 minute windows, two at a time.

## Method

- Level: a capture's talent points plus 9. A character with unspent points reads low, so a
  level is a lower bound; the spell ranks used agree with it for every sample below. Where a
  character levelled inside a report, the captures were bisected, and an encounter takes the
  level of the captures either side of it (encounters between two levels are dropped).
- Gear and attributes: the capture's items and its sheet (`ci_resolved.stats`, gear and buffs
  included). Spell damage and attack power come from the engine preparing that character
  (`fixtures/levels/logs/`): the class's reference request at the level, the logged items, the
  sheet's attributes, no talents. The armory's own item summary is not used: it gives the level 14
  Mage 3 spell damage where the items give 27, and the hits need 27 to 30.
- Hits: the median non-crit hit of one character's spell at one level, against creatures only.
  The median keeps partial resists and stray buffs out. Crits are excluded and read separately.
- Talents: a capture carries only the points in each tree, so a damage talent cannot be
  controlled. A sample is called a match when the engine is within 3.5% (direct spells) or half a
  point (ticks); larger gaps are listed with the talent or buff that would explain them.

## Measurements

Engine numbers are the prepared request's at pin 9238c90 (Forever rules): for a direct spell the
row's average at the level plus coefficient times spell damage, for a tick the row's tick plus the
dot's coefficient times spell damage.

| Class, level | Spell, rank | Samples | Spell damage | Engine | Logged median | Verdict |
| --- | --- | ---: | ---: | ---: | ---: | --- |
| Mage 14 | Frostbolt 3 (837) | 185 | 27 | 46 + 0.597 x 27 = 62.1 | 64 | matches, 3% high |
| Mage 14 | Arcane Explosion 1 (1449) | 43 | 27 | 35.9 | 36 | matches |
| Mage 14 | Fire Blast 2 (2137) | 38 | 27 | 69.6 | 68 | matches |
| Mage 21 | Frostbolt 4 (7322) | 52 | 32 | 70 + 0.706 x 32 = 92.6 | 94 | matches; 10 hits at 101 to 106 fit Eureka! (+10%) |
| Mage 21 | Arcane Explosion 1 (1449) | 501 | 32 | 38.6 | 38 | matches |
| Mage 21 | Fire Blast 2 (2137) | 12 | 32 | 76.7 | 77 | matches |
| Mage 20 | Frostbolt 4 (7322) | 101 | 34 | 94.0 | 97 | matches, 3% high |
| Mage 25 | Arcane Explosion 2 (8437) | 539 | 20 | 58.9 | 60 | matches |
| Mage 26 | Arcane Explosion 2 (8437) | 871 | 36 | 62.1 | 62 | matches |
| Mage 25, 26 | Arcane Missiles 3 (7270) | 213, 193 | 20, 36 | 52.7, 57.3 | 55, 61 | 4 to 7% high; Arcane talents in both |
| Mage 25 | Frostbolt 4 (7322) | 58 | 20 | 87.1 | 94 | 8% high; can't tell (Frost talents) |
| Warlock 16 | Shadow Bolt 3 (705) | 53 | 30 | 42 + 0.8 x 30 = 66.0 | 68 | matches, 3% high |
| Warlock 17 | Shadow Bolt 3 (705) | 30 | 30 | 67.0 | 69 | matches, 3% high |
| Warlock 16, 17 | Corruption 2 (6222) tick | 177, 146 | 30 | 13 + 0.2 x 30 = 19 | 19, 19 | matches exactly |
| Warlock 16 | Drain Life 1 (689) tick | 32 | 30 | 10 + 0.1 x 30 = 13 | 13 | matches exactly |
| Warlock 16 | Immolate 2 (707) tick | 71 | 30 | 6 + 0.13 x 30 = 9.9 | 10 | matches |
| Warlock 12 | Corruption 1 (172) tick | 332 | 1 | 10.2 | 11 (272 of 332) | 8%; 3 Affliction points |
| Warlock 18 | Corruption 2 (6222) tick | 576 | 40 | 21 | 23 | 10% high; 9 Affliction points |
| Priest 22 | Shadow Word: Pain 3 (970) tick | 110 | 35 | 18 + 0.2 x 35 = 25 | 25 | matches exactly |
| Priest 24 | Shadow Word: Pain 3 (970) tick | 19 | 21 | 22.2 | 23 | matches |
| Priest 18 | Shadow Word: Pain 3 (970) tick | 18 | 17 | 21.4 | 21 | matches |
| Druid 25 | Wrath 4 (5179) | 60 | 37 | 38 + 0.571 x 37 = 59.1 | 60 | matches |
| Druid 25 | Insect Swarm 1 (5570) tick | 79 | 37 | 13.8 | 14 | matches |
| Druid 25 | Moonfire 4 (8926) hit, tick | 90 | 37 | 41.6, 17.8 | 46, 19 | 7 to 11% high; 15 Balance points (an Improved Moonfire) |
| Paladin 25, 29 | Exorcism 1 (879), 2 (5614) | 25, 10 | 0 | 85.0, 142.0 | 86, 143 | matches |
| Paladin 34 | Exorcism 2 (5614) | 9 | 0 | 149.0 | 166 | 11% high; 25 Retribution points, 9 hits |
| Rogue 18 | Garrote 1 (703) tick | 90 | AP 135 | 24 + 0.03 x 135 = 28.1 | 28 | matches |
| Rogue 18, 20 | Garrote 1 (703) tick | 85, 55 | AP 137, 168 | 28.1, 29.0 | 29, 30 | matches |
| Rogue 24 | Garrote 2 (8631) tick | 33 | AP 173 | 34 + 0.03 x 173 = 39.2 | 40 | matches |
| Rogue 23, 25 (Subtlety) | Garrote 2 (8631) tick | 11, 34 | AP 180, 195 | 39.4, 39.9 | 47, 48 | 20% high: a 5 rank 4% Subtlety talent on attacks from behind |
| Warrior 15 to 27 | Rend 2 (6546), 3 (6547) tick | 586 | AP 133 to 269 | base + 0.02 x AP | 0.5 to 1.4 under | differs, see below |

Crits: a spell crit is 1.5 times the non-crit (Frostbolt 3: 92 to 97 against 62 to 66; Shadow
Bolt 3: 98 to 107 against 64 to 72), a physical crit twice it (Garrote 1: 56 against 28). The
engine's Frostbolt rank 3 on the level 14 fixture crits for 93.1 on a 62.1 hit (1.50).

What this settles:

- Level scaling: a rank's damage rises by the row's points per level up to its cap and stops
  there (Frostbolt 4 at 20, 21 and 25; Arcane Explosion 1 and Fire Blast 2 at 14 and 21; Shadow
  Bolt 3 at 16 and 17).
- Coefficients below level 20 (the open item in `docs/levels.md`): the client rows already carry
  the low rank coefficients, and the logs take them as they are. A Classic style penalty on top
  (Corruption rank 2, spell level 14: 0.775 of 0.2) would make the level 16 Warlock's tick 17;
  the log reads 19 on 177 ticks, which is 13 + 0.2 x 30.
- Item spell damage: the engine's item rows give the logged gear the spell damage the hits need.

## Findings of the first pass

The three gaps below are settled in [The open gaps, settled](#the-open-gaps-settled); the first
pass's text stays as it was written.

### Rend ticks read under the engine's 0.02 of attack power

Settled: the Warriors tick in Defensive Stance under Battle Shout or Blessing of Might, and the
engine's 0.02 holds with both counted.

Six samples from three Protection Warriors, levels 15 to 27, 586 non-crit ticks:

| Level | Rank | AP | Ticks | Engine (base + 0.02 AP) | Logged mean |
| --- | --- | ---: | ---: | ---: | ---: |
| 15 | 2 | 133 | 70 | 9.66 | 9.50 |
| 17 | 2 | 169 | 26 | 10.38 | 9.88 |
| 18 | 2 | 198 | 39 | 10.96 | 10.26 |
| 22 | 3 | 224 | 63 | 13.48 | 12.43 |
| 23 | 3 | 227 | 201 | 13.54 | 12.18 |
| 27 | 3 | 269 | 187 | 14.38 | 13.28 |

A share fitted to the means is 0.0154 (0.0175 if a tick floors to a whole number). The 0.02 is an
old reference literal, not a client row. Not changed here: the attack power comes from the sheet,
which already counts the buffs up when it was read, and the logs do not say whether a tick floors;
an attack power error of 30 to 60 or a floor explains most of the gap. A clean check is a Warrior
with known attack power (no buffs, sheet read in the fight) applying Rend to a target dummy.

### Hunter shots read 8 to 30% over the engine

Settled: Hunter's Mark and Aspect of the Hawk, which the engine has at the ranks the level knows.

Arcane Shot rank 2 at levels 16 to 18 and rank 3 at 21 to 26, Serpent Sting ranks 2 to 4 (one
Marksmanship Hunter, 100 to 760 hits a sample), against the engine's base plus 0.15 (Arcane Shot)
and 0.05 a tick (Serpent Sting) of ranged attack power: logged medians 60, 65, 73, 95, 111 against
55 to 95 for Arcane Shot, 15 to 53 against 14 to 41 for Serpent Sting. The Hunter swaps Aspect of
the Hawk and Aspect of the Monkey and has Trueshot Aura and Rapid Killing up part of the time, so
the ranged attack power behind each shot is unknown. Can't tell; a Hunter with no aspect and no
Trueshot Aura, its sheet read in the fight, would settle it.

### Two Affliction Warlocks hit 17 to 55% over their gear

Settled: the engine's item rows and Improved Corruption, against the armory's item summary and
logged gear that carries no random suffix.

At level 24 to 26, Shadow Bolt 4, Corruption 3, Drain Life 2 and Immolate 3 sit 14 to 25% (one
character) and 30 to 55% (another) over base plus coefficient on the items' spell damage. The
first has Soul Harvest (1242853) up part of the time; the second has nothing the logs name, and
its capture may predate a gear change. A third Warlock (levels 18 and 20) reads 21 to 28% high with
captures of 9 and 14 spell damage. Can't tell without the talent ranks and gear per encounter.

### The armory's item data is not the engine's

The armory states weapon damage that differs from the engine's item rows on 32 of the 94 weapons
in these captures. One can be checked against hits: a level 20 Shaman's two-hand mace (5187) is 30
to 46 in the armory and 47 to 71 in the engine; the Shaman's white hits reach 104 non-crit, which
30 to 46 cannot reach with that character's attack power, so the engine's row holds there. The
armory's summed spell damage also misses most of it (above). Read gear from item ids, never from
the armory's resolved stats.

## The open gaps, settled

Second pass, 9 November 2026, on the same public reports plus the one published since (9
November, The Deadmines, 53 encounters, client 1.60.1.70862); no other public report has appeared.
About 690 requests over three 15 minute windows, one or two at a time. Data credit: foreverlogs.gg
public reports. Only derived numbers are kept here.

Method: the encounter timelines carry aura events, both the buffs gained and lost during a pull
and, at each pull's start, the auras already up. The reports used for Warriors and Hunters list
stances or aspects among them, except the 9 November one, which lists neither for anyone. Every
tick and shot is classed by what was up when it landed: the Warrior's stance (from the stance
auras, or from a Revenge, Shield Block or Taunt cast, which need Defensive Stance), Battle Shout,
Blessing of Might, the Hunter's own Hunter's Mark on the target, Aspect of the Hawk, Trueshot Aura
and Rapid Killing. A character's attack power and spell damage are the engine's for its logged
level, items and sheet (the fixtures below), plus the logged buffs at the rank the level knows.
The level is the capture's talent points plus 9; a Hunter's capture carries no talent points, so
its level is the one its ranks and its two shots agree on.

### Rend: Defensive Stance and the shouts

Protection Warriors, mean non-crit tick:

| Level (Arms points) | Rank | Stance, buffs | Attack power | Ticks | Engine | Logged |
| --- | --- | --- | ---: | ---: | ---: | ---: |
| 15 (0) | 2 | Defensive, Battle Shout | 129 + 21 | 70 | 9.00 | 9.49 |
| 18 (0) | 2 | Defensive, Battle Shout | 173 + 22 | 115 | 9.81 | 9.79 |
| 22 (2) | 3 | Defensive, Blessing of Might | 224 + 25 | 158 | 12.58 | 12.15 |
| 22 (2) | 3 | Defensive, Might and Battle Shout | 224 + 48 | 143 | 13.00 | 12.50 |
| 22 (2) | 3 | Battle, Blessing of Might | 224 + 25 | 7 | 13.98 | 13.29 |
| 23 (3) | 3 | Defensive, nothing | 269 | 93 | 12.94 | 13.05 |
| 23 (3) | 3 | Defensive, Battle Shout | 269 + 33 | 95 | 13.54 | 13.51 |

Engine: (rank base + 0.02 x attack power) x 0.9 in Defensive Stance. The level 22 Warrior's ticks
span levels 22 and 23 of one report; the others read the sheet of the stretch they come from.

- Defensive Stance reaches Rend. The one Warrior who changed stance with Rend up ticked 1.094
  times as hard in Battle Stance as in Defensive Stance under the same Blessing (7 against 158
  ticks), and its white hits 1.097 times on the same creatures (33 swings in Battle Stance).
  Defensive Stance's passive (7376) is 10% less damage done, which the engine applies to the tick.
- The share is 0.02. Each row's tick over 0.9, less the rank's base, over its attack power gives
  0.0235, 0.0199, 0.0177, 0.0176, 0.0179 (Battle Stance), 0.0203 and 0.0197: a mean of 0.0200.
  Battle Shout on and off within one Warrior and one stretch of the run adds 0.35 to 0.79 a tick
  (three Warriors), 0.014 to 0.023 of the shout's attack power after the stance; noisy, around
  0.9 x 0.02. The first pass took every tick as unreduced and the sheet as the whole attack power;
  the two errors partly cancel, and the fit came out at 0.0154.
- Ticks round at random. Within one Rend, ticks read one value or two adjacent ones: of 189
  applications with three or more non-crit ticks, 80 read one value, 103 two adjacent ones and 6 a
  wider spread (a low outlier tick among them). A fixed value floored would read one value every
  time. Random rounding keeps the mean at the unrounded tick, which is what the engine
  deals (it does not round).
- Left over: levels 15 and 22 read 5% over and 4% under. The sheet is read at boss pulls only,
  and the level 22 and 23 Warriors have Arms points the capture does not name (Improved Rend, 12%
  a rank first, is one place they can go; the level 23 one matches without it).
- Verdict: unchanged. Rend's client row and Forever's tooltip ("28 damage over 12 sec" for rank 2)
  state only the base; the 0.02 is a literal the logs now measure.

### Hunter shots: Hunter's Mark and Aspect of the Hawk

One Marksmanship Hunter in two reports, mean non-crit hit or tick:

| Level, report | Shot, rank | Up | Ranged attack power | Hits | Engine | Logged |
| --- | --- | --- | ---: | ---: | ---: | ---: |
| 17 | Arcane Shot 2 | nothing | 130 | 32 | 54.50 | 54.66 |
| 17 | Arcane Shot 2 | Hawk | 130 + 20 | 4 | 57.50 | 57.25 |
| 17 | Arcane Shot 2 | Hawk, Mark | 130 + 46 | 10 | 61.40 | 61.40 |
| 17 | Serpent Sting 2 | nothing | 130 | 44 | 13.50 | 13.50 |
| 17 | Serpent Sting 2 | Mark | 130 + 26 | 10 | 14.80 | 14.60 |
| 17 | Serpent Sting 2 | Hawk, Mark | 130 + 46 | 6 | 15.80 | 16.00 |
| 22 | Arcane Shot 3 | Hawk | 178 + 35 | 38 | 81.95 | 83.08 |
| 22 | Arcane Shot 3 | Hawk, Mark | 178 + 94 | 27 | 90.80 | 91.89 |
| 22 | Serpent Sting 3 | Hawk | 178 + 35 | 96 | 24.65 | 24.71 |
| 22 | Serpent Sting 3 | Hawk, Mark | 178 + 94 | 69 | 27.60 | 27.81 |

Engine: Arcane Shot's base (35 and 50, the live hotfix) plus 0.15 of ranged attack power, Serpent
Sting's tick (7 and 14) plus 0.05; Hunter's Mark 26 at rank 1 and 59 at rank 2, Aspect of the Hawk
20 at rank 1 and 35 at rank 2, as the engine reads them at the level. The level 22 report lists no
aspect for anyone, and the shots read as Aspect of the Hawk up throughout.

- Hunter's Mark alone splits each spell into two tight groups. At 22 it adds 8.81 to Arcane Shot
  and 3.10 to Serpent Sting (engine 8.85 and 2.95). A third Hunter, in another report with Aspect
  of the Hawk up throughout and no capture, reads Arcane Shot 3 at 89.60 (10) and 98.60 marked
  (73), Serpent Sting 3 at 27.07 (100) and 29.98 marked (318): the Mark adds 9.00 and 2.91, and the
  two shots ask for the same ranged attack power (264 and 261). The shares and the Mark's value
  hold whatever the sheet.
- The first pass compared a Hunter who levelled from 16 to 26 across two reports against its sheet
  alone. Its shots carry Hunter's Mark, Aspect of the Hawk, Trueshot Aura and Rapid Killing (20%
  on the next shot), and Improved Stings (20% Serpent Sting damage, Marksmanship's second row) is
  open to it. It is not refitted here, as its level changes inside both reports.
- Verdict: unchanged.

### Affliction Warlocks: item rows and random suffixes

Mean non-crit hit or tick:

| Level (Affliction points) | Spell, rank | Hits | Engine | Logged |
| --- | --- | ---: | ---: | ---: |
| 24 (15) | Shadow Bolt 4 | 17 | 61 + 0.857 x 66 = 117.6 | 119.2 |
| 24 (15) | Corruption 3 tick | 322 | (22 + 0.2 x 66) x 1.1 = 38.7 | 38.5 |
| 24 (15) | Drain Life 2 tick | 87 | 14 + 0.1 x 66 = 20.6 | 20.4 |
| 24 (15) | Immolate 3 tick | 66 | 12 + 0.13 x 53 = 18.9 | 18.8 |
| 25 (16) | Shadow Bolt 4 | 99 | 62 + 0.857 x 36 = 92.9 | 93.2 |
| 25 (16) | Corruption 3 tick | 432 | (22 + 0.2 x 36) x 1.1 = 32.1 | 32.0 |
| 25 (16) | Drain Life 2 tick | 33 | (14 + 0.1 x 36) x 1.2 = 21.1 | 21.7 |
| 18 (9) | Corruption 2 tick | 569 | (13 + 0.2 x 39) x 1.08 = 22.5 | 22.5 |
| 18 (9) | Immolate 2 tick | 304 | 6 + 0.13 x 39 = 11.1 | 10.7 |

- The engine's items and the client's talents explain them. Corruption takes Improved Corruption
  (2% a point, the Affliction tree's first row: 5 points, and 4 at level 18), Drain Life Improved
  Drains (20%) for the level 25 Warlock, which has the points for both; Shadow Bolt and Immolate
  take no talent these characters can have. Later in its report the level 25 Warlock's new weapon
  gives 50 spell damage, and Shadow Bolt reads 107.5 against 104.9, Corruption 35.9 against 35.2.
- The 30 to 55% the first pass saw is the armory's Staff of Soran'ruk: 9 Shadow and Fire damage
  in the armory, 34 each in the engine's row and in Forever's live tooltip. The hits need the 34.
- The level 18 and 20 Warlock's hits ask for one spell damage, not a talent: at 18 Shadow Bolt 3
  (61.06 over 77 hits), Immolate 2's tick (8.66 over 47) and Corruption 2 (19.18 over 539, with
  Improved Corruption) each need 21 to 23, where the logged items give 9; at 20 they need 27 to 31
  where the items give 14. A percent talent would scale Shadow Bolt, Immolate and Corruption
  alike; one spell damage value fits three spells with different coefficients and bases. Four of
  its items (Shimmering Boots, Shimmering Cloak, Hook Dagger, Willow Branch) have a random suffix,
  and none of the 362 logged items in the captures read here carries a suffix id.
- Verdict: unchanged. Read a logged character's gear for spell damage only where it has no random
  suffix items.

### The smaller ones

No measurement beyond the talents the capture's points allow:

- Moonfire 7 to 11% high at 15 Balance points: Improved Moonfire (Balance's second row, 10% more
  damage at 2 points).
- Exorcism 11% high at level 34 with 25 Retribution points: Two-Handed Weapon Specialization (6%
  Holy damage with a two-hander) and Vengeance (3% a stack after a crit, up to 5).
- Arcane Missiles 4 to 7% high in Arcane Mages: Arcane Blast (Arcane's third row) gives 12% of
  Intellect as spell damage a cast. An Arcane Mage in another report casts it 158 times, and its
  Missiles spread from 60 to 86 with the stacks; not measured stack by stack.

### Pinned

`tests/levels_logs_gaps_20261109.rs`, on nine new fixtures (four Protection Warriors, levels 15,
18, 22 and 23; a Marksmanship Hunter at 17 and 22; Affliction Warlocks at 18, 24 and 25): Rend's
ticks with stance and buffs, Defensive Stance's multiplier against the stance ratio, both Hunter
shots with Mark and Hawk at the ranks the level knows, the Warlocks' spells on the item rows, and
the level 18 Warlock's missing spell damage. No engine source changed, so no Go golden and no level
60 board or healer request moves.

## What the logs cannot answer

- Crit chance against the sheet: the capture carries the six attributes, not crit.
- Glancing, dodge and parry rates against mobs a few levels up: damage events carry no glance
  flag and no creature level, and dungeon packs mix levels.
- White hit ranges and the white hit ratio: armor is unknown, glances are folded into non-crit
  hits, and a Rogue's main and off hand share one Auto Attack id. Two-hand Warriors and Paladins
  could give ratios, but the abilities they press (Revenge, Holy Strike, Thunder Clap) are not
  weapon damage.
- Threat (no threat in logs) and talent ranks (points a tree only). Buffs are in the timelines'
  aura events (second pass); the attack power they add is the engine's value at the rank.
- Anything at level 36 or above, which no public report reaches yet.
