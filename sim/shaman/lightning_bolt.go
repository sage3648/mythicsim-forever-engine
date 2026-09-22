package shaman

import (
	"math"

	"github.com/wowsims/classic/sim/core"
)

const LightningBoltRanks = 10

// Forever beta client (build 1.60.1.69893): Forever halves the upper ranks, drops the cast to 2.5 sec and gives
// every rank from 3 up the full 0.714 coefficient. Spell ID, cost, cast time, coefficient, missile speed, school
// and defense type come from the client table (spellData). Damage stays here: the table holds one truncated value
// per rank, not the range. Ours is the client's range scaled to level 60 by its per-level points, capped at the
// rank's max level, low end rounded down and high end up; spell_damage_test.go checks it still contains the
// table's value.
var LightningBoltBaseDamage = [LightningBoltRanks + 1][]float64{{0}, {15, 17}, {29, 34}, {35, 41}, {49, 57}, {70, 81}, {108, 122}, {142, 160}, {157, 177}, {172, 194}, {190, 212}}
var LightningBoltLevel = [LightningBoltRanks + 1]int{0, 1, 8, 14, 20, 26, 32, 38, 44, 50, 56}

// The client stores .714 as a float32; the table widens it. Rounding back to the stated value keeps the sim's
// numbers where they were (sim/mage/frostbolt.go). Every coefficient read from the table goes through this.
func roundCoef(coef float64) float64 {
	return math.Round(coef*1e6) / 1e6
}

func (shaman *Shaman) registerLightningBoltSpell() {
	shaman.LightningBolt = make([]*core.Spell, LightningBoltRanks+1)
	shaman.LightningBoltOverload = make([]*core.Spell, LightningBoltRanks+1)

	for rank := 1; rank <= LightningBoltRanks; rank++ {
		config := shaman.newLightningBoltSpellConfig(rank)

		if config.RequiredLevel <= int(shaman.Level) {
			// The overload gets a config of its own so that the two casts share no state.
			shaman.LightningBoltOverload[rank] = shaman.registerOverloadSpell(shaman.newLightningBoltSpellConfig(rank))
			shaman.LightningBolt[rank] = shaman.RegisterSpell(config)
		}
	}
}

func (shaman *Shaman) newLightningBoltSpellConfig(rank int) core.SpellConfig {
	baseDamageLow := LightningBoltBaseDamage[rank][0]
	baseDamageHigh := LightningBoltBaseDamage[rank][1]
	level := LightningBoltLevel[rank]

	row := spellData.LightningBolt.ByRank(int32(rank))
	spell := shaman.newElectricSpellConfig(core.ActionID{SpellID: row.SpellID}, row)
	spell.SpellCode = SpellCode_ShamanLightningBolt
	spell.ClassSpellMask = SpellMaskLightningBolt
	spell.RequiredLevel = level
	spell.Rank = rank

	spell.ApplyEffects = func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
		baseDamage := sim.Roll(baseDamageLow, baseDamageHigh)
		result := spell.CalcDamage(sim, target, baseDamage, spell.OutcomeMagicHitAndCrit)

		spell.WaitTravelTime(sim, func(sim *core.Simulation) {
			spell.DealDamage(sim, result)
		})

		shaman.tryLightningOverload(sim, target, spell, shaman.LightningBoltOverload[rank])
	}

	return spell
}
