package shaman

import (
	"github.com/wowsims/classic/sim/core"
)

const EarthShockRanks = 7

// Forever beta client values. Every rank carries the full 0.386. Everything but the damage (scaled to level 60
// like Lightning Bolt's) comes from the client table (see shocks.go).
var EarthShockBaseDamage = [EarthShockRanks + 1][]float64{{0}, {19, 22}, {35, 38}, {51, 56}, {83, 90}, {134, 143}, {206, 220}, {293, 309}}
var EarthShockLevel = [EarthShockRanks + 1]int{0, 4, 8, 14, 24, 36, 48, 60}

func (shaman *Shaman) registerEarthShockSpell(shockTimer *core.Timer) {
	shaman.EarthShock = make([]*core.Spell, EarthShockRanks+1)

	for rank := 1; rank <= EarthShockRanks; rank++ {
		config := shaman.newEarthShockSpellConfig(rank, shockTimer)

		if config.RequiredLevel <= int(shaman.Level) {
			shaman.EarthShock[rank] = shaman.RegisterSpell(config)
		}
	}
}

func (shaman *Shaman) newEarthShockSpellConfig(rank int, shockTimer *core.Timer) core.SpellConfig {
	baseDamageLow := EarthShockBaseDamage[rank][0]
	baseDamageHigh := EarthShockBaseDamage[rank][1]
	level := EarthShockLevel[rank]

	row := spellData.EarthShock.ByRank(int32(rank))
	spell := shaman.newShockSpellConfig(core.ActionID{SpellID: row.SpellID}, row, shockTimer)

	spell.Flags |= core.SpellFlagBinary

	spell.SpellCode = SpellCode_ShamanEarthShock
	spell.ClassSpellMask = SpellMaskEarthShock
	spell.RequiredLevel = level
	spell.Rank = rank

	spell.ThreatMultiplier = 2

	spell.ApplyEffects = func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
		baseDamage := sim.Roll(baseDamageLow, baseDamageHigh)
		spell.CalcAndDealDamage(sim, target, baseDamage, spell.OutcomeMagicHitAndCrit)
	}

	return spell
}
