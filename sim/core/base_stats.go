package core

import (
	"github.com/wowsims/forever/sim/core/proto"
	"github.com/wowsims/forever/sim/core/stats"
)

type BaseStatsKey struct {
	Race  proto.Race
	Class proto.Class
}

var BaseStats = map[BaseStatsKey]stats.Stats{}

// ClassBaseStats are the LEVEL 60 rows (health and the five attributes) from Wowhead's
// Forever gear planner, wow.gearPlanner.classicplus.baseStats (index 60 of each class's
// per-level arrays; snapshot in ElliotWood/Forever master, assets/db_inputs/
// wowhead_forever_gearplanner.txt). Its raceOffsets match RaceOffsets below exactly.
// These replace the level 70 rows fitted from TBC-anniversary logs, which a level 60
// character was running on (e.g. mage 151 intellect instead of 125).
//
// The client itself carries no attribute table, verified against build 1.60.1.69913:
// CharBaseInfo is race x class validity only; ChrClasses has AttackPowerPerStrength/Agility
// but no attributes; ChrRaces has no stat columns; RaceStat is one row per race and every
// value is 0; and PlayerExpectedStat carries BaseMana/CritPerAgility/SpellCritPerIntellect
// but no attributes.
//
// ClassBaseStats + RaceOffsets hold TRUE pre-racial base attributes: the
// multiplier racials (The Human Spirit ×1.05 spirit, Tauren Endurance ×1.05
// health and, under Forever, gnome Expansive Mind ×1.05 mana, applied via
// MultiplyStat in racials.go) are NOT included here. A naked character sheet
// shows floor(base × racial); multipliers (racial, Kings, %-stat talents)
// stack multiplicatively on the unfloored value with a single floor at the end.
//
// The game keeps one attribute row per race and class, but that table is a
// class row plus a race offset that is the same for every class, so the two
// maps below reproduce it exactly.

// Base Spell Crit is calculated by
//   1. Take as-shown value (troll shaman have 3.5%)
//   2. Calculate the bonus from int (for troll shaman that would be 104/78.1=1.331% crit)
//   3. Subtract as-shown from int bouns (3.5-1.331=2.169)
//   4. 2.169*22.08 (rating per crit percent) = 47.89 crit rating.
//
// TODO: the 22.08 in step 4 is the level-70 crit rating per percent. At level 60
// it is 14 (see SpellCritRatingPerCritPercent in base_stats_auto_gen.go), so this
// worked example no longer reproduces the numbers above it.

// Base mana can be looked up here: https://wowwiki-archive.fandom.com/wiki/Base_mana

// These are also scattered in various dbc/casc files,
// `octbasempbyclass.txt`, `combatratings.txt`, `chancetospellcritbase.txt`, etc.

var RaceOffsets = map[proto.Race]stats.Stats{
	proto.Race_RaceUnknown: stats.Stats{},
	proto.Race_RaceHuman:   stats.Stats{},
	proto.Race_RaceOrc: {
		stats.Agility:   -3,
		stats.Strength:  3,
		stats.Intellect: -3,
		stats.Spirit:    3,
		stats.Stamina:   2,
	},
	proto.Race_RaceDwarf: {
		stats.Agility:   -4,
		stats.Strength:  2,
		stats.Intellect: -1,
		stats.Spirit:    -1,
		stats.Stamina:   3,
	},
	proto.Race_RaceNightElf: {
		stats.Agility:   5,
		stats.Strength:  -3,
		stats.Intellect: 0,
		stats.Spirit:    0,
		stats.Stamina:   -1,
	},
	proto.Race_RaceUndead: {
		stats.Agility:   -2,
		stats.Strength:  -1,
		stats.Intellect: -2,
		stats.Spirit:    5,
		stats.Stamina:   1,
	},
	proto.Race_RaceTauren: {
		stats.Agility:   -5,
		stats.Strength:  5,
		stats.Intellect: -5,
		stats.Spirit:    2,
		stats.Stamina:   2,
	},
	proto.Race_RaceGnome: {
		stats.Agility:   3,
		stats.Strength:  -5,
		stats.Intellect: 3,
		stats.Spirit:    0,
		stats.Stamina:   -1,
	},
	proto.Race_RaceTroll: {
		stats.Agility:   2,
		stats.Strength:  1,
		stats.Intellect: -4,
		stats.Spirit:    1,
		stats.Stamina:   1,
	},
	proto.Race_RaceBloodElf: {
		stats.Agility:   2,
		stats.Strength:  -3,
		stats.Intellect: 4,
		stats.Spirit:    -1,
		stats.Stamina:   -2,
	},
	proto.Race_RaceDraenei: {
		stats.Agility:   -3,
		stats.Strength:  1,
		stats.Intellect: 1,
		stats.Spirit:    2,
		stats.Stamina:   -1,
	},
	// The Skyborne sit at the class baseline, which Wowhead's Forever gear planner
	// confirms: races 95 and 96 in its baseStats.raceOffsets are zero for agility,
	// strength, intellect, spirit and stamina alike, as the human's are. Snapshot in
	// assets/db_inputs/wowhead_forever_gearplanner.txt.
	proto.Race_RaceSkyborneHighOrder:  {},
	proto.Race_RaceSkyborneWindshaper: {},
}

var ClassBaseStats = map[proto.Class]stats.Stats{
	proto.Class_ClassUnknown: {},
	proto.Class_ClassWarrior: {
		stats.Health:      1689,
		stats.Agility:     80,
		stats.Strength:    120,
		stats.Intellect:   30,
		stats.Spirit:      45,
		stats.Stamina:     110,
		stats.AttackPower: float64(CharacterLevel)*3.0 - 20,
	},
	proto.Class_ClassPaladin: {
		stats.Health:      1381,
		stats.Agility:     65,
		stats.Strength:    105,
		stats.Intellect:   70,
		stats.Spirit:      75,
		stats.Stamina:     100,
		stats.AttackPower: float64(CharacterLevel)*3.0 - 20,
	},
	proto.Class_ClassHunter: {
		stats.Health:            1467,
		stats.Agility:           125,
		stats.Strength:          55,
		stats.Intellect:         65,
		stats.Spirit:            70,
		stats.Stamina:           90,
		stats.AttackPower:       float64(CharacterLevel)*2.0 - 20,
		stats.RangedAttackPower: float64(CharacterLevel)*2.0 - 20,
	},
	proto.Class_ClassRogue: {
		stats.Health:      1523,
		stats.Agility:     130,
		stats.Strength:    80,
		stats.Intellect:   35,
		stats.Spirit:      50,
		stats.Stamina:     75,
		stats.AttackPower: float64(CharacterLevel)*2.0 - 20,
	},
	proto.Class_ClassPriest: {
		stats.Health:      1397,
		stats.Agility:     40,
		stats.Strength:    35,
		stats.Intellect:   120,
		stats.Spirit:      125,
		stats.Stamina:     50,
		stats.AttackPower: -10,
	},
	proto.Class_ClassShaman: {
		stats.Health:      1280,
		stats.Agility:     55,
		stats.Strength:    85,
		stats.Intellect:   90,
		stats.Spirit:      100,
		stats.Stamina:     95,
		stats.AttackPower: float64(CharacterLevel)*2.0 - 20,
	},
	proto.Class_ClassMage: {
		stats.Health:      1370,
		stats.Agility:     35,
		stats.Strength:    30,
		stats.Intellect:   125,
		stats.Spirit:      120,
		stats.Stamina:     45,
		stats.AttackPower: -10,
	},
	proto.Class_ClassWarlock: {
		stats.Health:      1414,
		stats.Agility:     50,
		stats.Strength:    45,
		stats.Intellect:   110,
		stats.Spirit:      115,
		stats.Stamina:     65,
		stats.AttackPower: -10,
	},
	proto.Class_ClassDruid: {
		stats.Health:      1483,
		stats.Agility:     60,
		stats.Strength:    65,
		stats.Intellect:   100,
		stats.Spirit:      110,
		stats.Stamina:     70,
		stats.AttackPower: -20,
	},
}

// The LEVEL 60 rows of GameTables/SpellScaling.txt. Only tools/tooltip reads this
// (dbc_data_provider.go:249), so the sim is unaffected either way, but tooltip spell
// values used to be computed off the level 90 row -- a MoP-port leftover that was never
// updated for TBC and was wrong by two expansions at level 60.
//
// TODO: these numbers came from a MoP-era copy of that gametable. The checked-in
// assets/db_inputs/basestats/SpellScaling.txt is now the beta's own extraction, and
// every class column in it is 0 at every level, so there is no build-native
// confirmation of these numbers and none is currently obtainable.
var ClassBaseScaling = map[proto.Class]float64{
	proto.Class_ClassUnknown: 49.000000,
	proto.Class_ClassWarrior: 491.949980,
	proto.Class_ClassPaladin: 332.962490,
	proto.Class_ClassHunter:  355.055050,
	proto.Class_ClassRogue:   532.945800,
	proto.Class_ClassPriest:  336.625000,
	proto.Class_ClassShaman:  251.970830,
	proto.Class_ClassMage:    366.620820,
	proto.Class_ClassWarlock: 308.962490,
	proto.Class_ClassDruid:   282.633330,
}

// Base melee and spell crit at level 60, before agility and intellect: the Classic client's
// gtChanceToMeleeCritBase / gtChanceToSpellCritBase, as master carries them. The generated
// ExtraClassBaseStats hold TBC's level 70 fits (hunter -1.53%, warrior +1.14% melee), which a
// level 60 Forever character has no business with. The Forever client ships no gt tables;
// its PlayerExpectedStat level 60 CritPerAgility/SpellCritPerIntellect rows equal both
// engines' per-point rates, so only these constants differed.
var ClassBaseCritPercent = map[proto.Class]struct{ Physical, Spell float64 }{
	proto.Class_ClassWarrior: {0, 0},
	proto.Class_ClassPaladin: {0.7, 3.5},
	proto.Class_ClassHunter:  {0, 3.6},
	proto.Class_ClassRogue:   {0, 0},
	proto.Class_ClassPriest:  {3.0, 0.8},
	proto.Class_ClassShaman:  {1.7, 2.3},
	proto.Class_ClassMage:    {3.2, 0.2},
	proto.Class_ClassWarlock: {2.0, 1.7},
	proto.Class_ClassDruid:   {0.9, 1.8},
}

func AddBaseStatsCombo(r proto.Race, c proto.Class) {
	s := ClassBaseStats[c].Add(RaceOffsets[r]).Add(ExtraClassBaseStats[c])
	s[stats.PhysicalCritPercent] = ClassBaseCritPercent[c].Physical
	s[stats.SpellCritPercent] = ClassBaseCritPercent[c].Spell
	BaseStats[BaseStatsKey{Race: r, Class: c}] = s
}

func init() {
	AddBaseStatsCombo(proto.Race_RaceTauren, proto.Class_ClassDruid)
	AddBaseStatsCombo(proto.Race_RaceNightElf, proto.Class_ClassDruid)
	AddBaseStatsCombo(proto.Race_RaceSkyborneHighOrder, proto.Class_ClassDruid)
	AddBaseStatsCombo(proto.Race_RaceSkyborneWindshaper, proto.Class_ClassDruid)

	AddBaseStatsCombo(proto.Race_RaceBloodElf, proto.Class_ClassHunter)
	AddBaseStatsCombo(proto.Race_RaceDraenei, proto.Class_ClassHunter)
	AddBaseStatsCombo(proto.Race_RaceDwarf, proto.Class_ClassHunter)
	AddBaseStatsCombo(proto.Race_RaceNightElf, proto.Class_ClassHunter)
	AddBaseStatsCombo(proto.Race_RaceOrc, proto.Class_ClassHunter)
	AddBaseStatsCombo(proto.Race_RaceTauren, proto.Class_ClassHunter)
	AddBaseStatsCombo(proto.Race_RaceTroll, proto.Class_ClassHunter)
	AddBaseStatsCombo(proto.Race_RaceHuman, proto.Class_ClassHunter)
	AddBaseStatsCombo(proto.Race_RaceSkyborneHighOrder, proto.Class_ClassHunter)
	AddBaseStatsCombo(proto.Race_RaceSkyborneWindshaper, proto.Class_ClassHunter)

	AddBaseStatsCombo(proto.Race_RaceDraenei, proto.Class_ClassMage)
	AddBaseStatsCombo(proto.Race_RaceGnome, proto.Class_ClassMage)
	AddBaseStatsCombo(proto.Race_RaceHuman, proto.Class_ClassMage)
	AddBaseStatsCombo(proto.Race_RaceDwarf, proto.Class_ClassMage)
	AddBaseStatsCombo(proto.Race_RaceBloodElf, proto.Class_ClassMage)
	AddBaseStatsCombo(proto.Race_RaceTroll, proto.Class_ClassMage)
	AddBaseStatsCombo(proto.Race_RaceUndead, proto.Class_ClassMage)
	AddBaseStatsCombo(proto.Race_RaceOrc, proto.Class_ClassMage)
	AddBaseStatsCombo(proto.Race_RaceSkyborneHighOrder, proto.Class_ClassMage)

	AddBaseStatsCombo(proto.Race_RaceBloodElf, proto.Class_ClassPaladin)
	AddBaseStatsCombo(proto.Race_RaceDraenei, proto.Class_ClassPaladin)
	AddBaseStatsCombo(proto.Race_RaceHuman, proto.Class_ClassPaladin)
	AddBaseStatsCombo(proto.Race_RaceDwarf, proto.Class_ClassPaladin)
	AddBaseStatsCombo(proto.Race_RaceUndead, proto.Class_ClassPaladin)

	AddBaseStatsCombo(proto.Race_RaceHuman, proto.Class_ClassPriest)
	AddBaseStatsCombo(proto.Race_RaceDwarf, proto.Class_ClassPriest)
	AddBaseStatsCombo(proto.Race_RaceGnome, proto.Class_ClassPriest)
	AddBaseStatsCombo(proto.Race_RaceNightElf, proto.Class_ClassPriest)
	AddBaseStatsCombo(proto.Race_RaceDraenei, proto.Class_ClassPriest)
	AddBaseStatsCombo(proto.Race_RaceUndead, proto.Class_ClassPriest)
	AddBaseStatsCombo(proto.Race_RaceTroll, proto.Class_ClassPriest)
	AddBaseStatsCombo(proto.Race_RaceBloodElf, proto.Class_ClassPriest)

	AddBaseStatsCombo(proto.Race_RaceBloodElf, proto.Class_ClassRogue)
	AddBaseStatsCombo(proto.Race_RaceDwarf, proto.Class_ClassRogue)
	AddBaseStatsCombo(proto.Race_RaceGnome, proto.Class_ClassRogue)
	AddBaseStatsCombo(proto.Race_RaceHuman, proto.Class_ClassRogue)
	AddBaseStatsCombo(proto.Race_RaceNightElf, proto.Class_ClassRogue)
	AddBaseStatsCombo(proto.Race_RaceOrc, proto.Class_ClassRogue)
	AddBaseStatsCombo(proto.Race_RaceTroll, proto.Class_ClassRogue)
	AddBaseStatsCombo(proto.Race_RaceUndead, proto.Class_ClassRogue)
	AddBaseStatsCombo(proto.Race_RaceSkyborneHighOrder, proto.Class_ClassRogue)
	AddBaseStatsCombo(proto.Race_RaceSkyborneWindshaper, proto.Class_ClassRogue)

	AddBaseStatsCombo(proto.Race_RaceDraenei, proto.Class_ClassShaman)
	AddBaseStatsCombo(proto.Race_RaceOrc, proto.Class_ClassShaman)
	AddBaseStatsCombo(proto.Race_RaceTauren, proto.Class_ClassShaman)
	AddBaseStatsCombo(proto.Race_RaceTroll, proto.Class_ClassShaman)
	AddBaseStatsCombo(proto.Race_RaceDwarf, proto.Class_ClassShaman)
	AddBaseStatsCombo(proto.Race_RaceSkyborneWindshaper, proto.Class_ClassShaman)

	AddBaseStatsCombo(proto.Race_RaceBloodElf, proto.Class_ClassWarlock)
	AddBaseStatsCombo(proto.Race_RaceOrc, proto.Class_ClassWarlock)
	AddBaseStatsCombo(proto.Race_RaceUndead, proto.Class_ClassWarlock)
	AddBaseStatsCombo(proto.Race_RaceHuman, proto.Class_ClassWarlock)
	AddBaseStatsCombo(proto.Race_RaceGnome, proto.Class_ClassWarlock)
	AddBaseStatsCombo(proto.Race_RaceDwarf, proto.Class_ClassWarlock)
	AddBaseStatsCombo(proto.Race_RaceTroll, proto.Class_ClassWarlock)

	AddBaseStatsCombo(proto.Race_RaceDraenei, proto.Class_ClassWarrior)
	AddBaseStatsCombo(proto.Race_RaceDwarf, proto.Class_ClassWarrior)
	AddBaseStatsCombo(proto.Race_RaceGnome, proto.Class_ClassWarrior)
	AddBaseStatsCombo(proto.Race_RaceHuman, proto.Class_ClassWarrior)
	AddBaseStatsCombo(proto.Race_RaceNightElf, proto.Class_ClassWarrior)
	AddBaseStatsCombo(proto.Race_RaceOrc, proto.Class_ClassWarrior)
	AddBaseStatsCombo(proto.Race_RaceTauren, proto.Class_ClassWarrior)
	AddBaseStatsCombo(proto.Race_RaceTroll, proto.Class_ClassWarrior)
	AddBaseStatsCombo(proto.Race_RaceUndead, proto.Class_ClassWarrior)
	AddBaseStatsCombo(proto.Race_RaceSkyborneHighOrder, proto.Class_ClassWarrior)
	AddBaseStatsCombo(proto.Race_RaceSkyborneWindshaper, proto.Class_ClassWarrior)
}
