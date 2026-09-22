package shaman

import (
	"github.com/wowsims/classic/sim/core"
)

const FrostShockRanks = 4

// Forever beta client values. Everything but the damage (scaled to level 60 like Lightning Bolt's) comes from
// the client table (see shocks.go).
var FrostShockBaseDamage = [FrostShockRanks + 1][]float64{{0}, {68, 73}, {126, 135}, {190, 201}, {278, 295}}
var FrostShockLevel = [FrostShockRanks + 1]int{0, 20, 34, 46, 58}

func (shaman *Shaman) registerFrostShockSpell(shockTimer *core.Timer) {
	shaman.FrostShock = make([]*core.Spell, FrostShockRanks+1)

	for rank := 1; rank <= FrostShockRanks; rank++ {
		config := shaman.newFrostShockSpellConfig(rank, shockTimer)

		if config.RequiredLevel <= int(shaman.Level) {
			shaman.FrostShock[rank] = shaman.RegisterSpell(config)
		}
	}
}

func (shaman *Shaman) newFrostShockSpellConfig(rank int, shockTimer *core.Timer) core.SpellConfig {
	baseDamageLow := FrostShockBaseDamage[rank][0]
	baseDamageHigh := FrostShockBaseDamage[rank][1]
	level := FrostShockLevel[rank]

	row := spellData.FrostShock.ByRank(int32(rank))
	spell := shaman.newShockSpellConfig(core.ActionID{SpellID: row.SpellID}, row, shockTimer)

	spell.SpellCode = SpellCode_ShamanFrostShock
	spell.ClassSpellMask = SpellMaskFrostShock
	spell.RequiredLevel = level
	spell.Rank = rank

	spell.ApplyEffects = func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
		baseDamage := sim.Roll(baseDamageLow, baseDamageHigh)
		spell.CalcAndDealDamage(sim, target, baseDamage, spell.OutcomeMagicHitAndCrit)
	}

	return spell
}
