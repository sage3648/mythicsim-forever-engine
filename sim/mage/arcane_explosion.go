package mage

import (
	"github.com/wowsims/classic/sim/core"
)

const ArcaneExplosionRanks = 6

// Spell ID, cost, school and coefficient come from the client table (see
// frostbolt.go for why the damage does not).
//
// Beta client 1.60.1.69893.
var ArcaneExplosionBaseDamage = [ArcaneExplosionRanks + 1][]float64{{0}, {32, 36}, {55, 61}, {94, 103}, {133, 146}, {180, 197}, {238, 259}}
var ArcaneExplosionLevel = [ArcaneExplosionRanks + 1]int{0, 14, 22, 30, 38, 46, 54}

func (mage *Mage) registerArcaneExplosionSpell() {
	mage.ArcaneExplosion = make([]*core.Spell, ArcaneExplosionRanks+1)

	for rank := 1; rank <= ArcaneExplosionRanks; rank++ {
		config := mage.newArcaneExplosionSpellConfig(rank)

		if config.RequiredLevel <= int(mage.Level) {
			mage.ArcaneExplosion[rank] = mage.GetOrRegisterSpell(config)
		}
	}
}

func (mage *Mage) newArcaneExplosionSpellConfig(rank int) core.SpellConfig {
	row := spellData.ArcaneExplosion.ByRank(int32(rank))
	baseDamageLow := ArcaneExplosionBaseDamage[rank][0]
	baseDamageHigh := ArcaneExplosionBaseDamage[rank][1]
	level := ArcaneExplosionLevel[rank]

	return core.SpellConfig{
		ActionID:       core.ActionID{SpellID: row.SpellID},
		SpellCode:      SpellCode_MageArcaneExplosion,
		ClassSpellMask: SpellMaskArcaneExplosion,
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskSpellDamage,
		Flags:          SpellFlagMage | core.SpellFlagAPL,

		RequiredLevel: level,
		Rank:          rank,

		ManaCost: core.ManaCostOptions{
			FlatCost: float64(row.Cost),
		},
		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD: core.GCDDefault,
			},
		},

		DamageMultiplier: 1,
		ThreatMultiplier: 1,
		BonusCoefficient: roundCoef(row.Direct.BonusCoefficient()),

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			for _, aoeTarget := range sim.Encounter.TargetUnits {
				damage := sim.Roll(baseDamageLow, baseDamageHigh)
				spell.CalcAndDealDamage(sim, aoeTarget, damage, spell.OutcomeMagicCrit)
			}
		},
	}
}
