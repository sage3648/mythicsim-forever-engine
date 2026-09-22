package mage

import (
	"github.com/wowsims/classic/sim/core"
)

const ScorchRanks = 7

// Spell ID, cost, cast time and coefficient come from the client table (see
// frostbolt.go for why the damage does not).
//
// Beta client 1.60.1.69893, about 30% below Classic at every rank.
var ScorchBaseDamage = [ScorchRanks + 1][]float64{{0}, {38, 47}, {54, 64}, {67, 79}, {89, 106}, {111, 132}, {143, 169}, {166, 197}}
var ScorchLevel = [ScorchRanks + 1]int{0, 22, 28, 34, 40, 46, 52, 58}

func (mage *Mage) registerScorchSpell() {
	mage.Scorch = make([]*core.Spell, ScorchRanks+1)

	for rank := 1; rank <= ScorchRanks; rank++ {
		config := mage.getScorchConfig(rank)

		if config.RequiredLevel <= int(mage.Level) {
			mage.Scorch[rank] = mage.GetOrRegisterSpell(config)
		}
	}
}

func (mage *Mage) getScorchConfig(rank int) core.SpellConfig {
	row := spellData.Scorch.ByRank(int32(rank))
	baseDamageLow := ScorchBaseDamage[rank][0]
	baseDamageHigh := ScorchBaseDamage[rank][1]
	level := ScorchLevel[rank]

	debuffProcChance := []float64{0, .33, .66, 1}[mage.Talents.ImprovedScorch]

	return core.SpellConfig{
		ActionID:       core.ActionID{SpellID: row.SpellID},
		SpellCode:      SpellCode_MageScorch,
		ClassSpellMask: SpellMaskScorch,
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskSpellDamage,
		Flags:          core.SpellFlagAPL | SpellFlagMage,

		RequiredLevel: level,
		Rank:          rank,

		ManaCost: core.ManaCostOptions{
			FlatCost: float64(row.Cost),
		},
		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD:      core.GCDDefault,
				CastTime: row.CastTime,
			},
		},

		DamageMultiplier: 1,
		ThreatMultiplier: 1,
		BonusCoefficient: roundCoef(row.Direct.BonusCoefficient()),

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			baseDamage := sim.Roll(baseDamageLow, baseDamageHigh)
			spell.CalcAndDealDamage(sim, target, baseDamage, spell.OutcomeMagicHitAndCrit)

			if sim.RandomFloat("Improved Scorch") < debuffProcChance {
				mage.ImprovedScorchAura.Activate(sim)
				mage.ImprovedScorchAura.AddStack(sim)
			}
		},
	}
}
