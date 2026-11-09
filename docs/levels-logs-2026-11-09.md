# Levelling characters against public combat logs, 2026-11-09

Source: foreverlogs.gg public reports, read through the site's own JSON pages in a browser (no API
key). Data credit: foreverlogs.gg public reports. Only the derived numbers below are kept; no log,
character or player name is stored here or in the fixtures.

Short version: where the capture leaves nothing unknown, the engine's levelling numbers match the
logs. A low rank's base damage at the level, the client row's spell power coefficient on the
gear's spell damage and Garrote's attack power share reproduce the logged non-crit hits to the
point for dot ticks and within 3.5% for direct spells. Rend reads 5 to 10% under the engine and
Hunter shots 8 to 30% over it, but neither can be settled from these logs. Tests:
`tests/levels_logs_20261109.rs`, fixtures `fixtures/levels/logs/`.

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

## Findings for review

### Rend ticks read under the engine's 0.02 of attack power

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

Arcane Shot rank 2 at levels 16 to 18 and rank 3 at 21 to 26, Serpent Sting ranks 2 to 4 (one
Marksmanship Hunter, 100 to 760 hits a sample), against the engine's base plus 0.15 (Arcane Shot)
and 0.05 a tick (Serpent Sting) of ranged attack power: logged medians 60, 65, 73, 95, 111 against
55 to 95 for Arcane Shot, 15 to 53 against 14 to 41 for Serpent Sting. The Hunter swaps Aspect of
the Hawk and Aspect of the Monkey and has Trueshot Aura and Rapid Killing up part of the time, so
the ranged attack power behind each shot is unknown. Can't tell; a Hunter with no aspect and no
Trueshot Aura, its sheet read in the fight, would settle it.

### Two Affliction Warlocks hit 17 to 55% over their gear

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

## What the logs cannot answer

- Crit chance against the sheet: the capture carries the six attributes, not crit.
- Glancing, dodge and parry rates against mobs a few levels up: damage events carry no glance
  flag and no creature level, and dungeon packs mix levels.
- White hit ranges and the white hit ratio: armor is unknown, glances are folded into non-crit
  hits, and a Rogue's main and off hand share one Auto Attack id. Two-hand Warriors and Paladins
  could give ratios, but the abilities they press (Revenge, Holy Strike, Thunder Clap) are not
  weapon damage.
- Threat (no threat in logs), talent ranks (points a tree only), attack power from buffs.
- Anything at level 36 or above, which no public report reaches yet.
