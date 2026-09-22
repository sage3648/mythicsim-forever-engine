package mage

import (
	"math"
	"time"

	"github.com/wowsims/classic/sim/core"
)

const FrostboltRanks = 11

// Spell ID, cost, cast time, coefficient and missile speed come from the client table
// (spell_data_auto_gen.go, vendored from wowsims/forever). The damage does not: the client rolls each
// rank over a spread (SpellEffect.Variance, 7-12%), and the vendored generator drops it (its own TODO in
// tools/database/spelldata.go) and truncates the level-scaled centre, so the table's Value is one
// number where the game rolls a range. Until it carries the spread we keep ours: beta client
// 1.60.1.69893, base plus EffectRealPointsPerLevel to the rank's max level, capped at 60.
// spell_damage_test.go checks every range here still contains the table's value.
//
// Beta client 1.60.1.69893: every rank from 3 up hits for less, and the low ranks lost their
// downranking penalty.
var FrostboltBaseDamage = [FrostboltRanks + 1][]float64{{0, 0}, {20, 22}, {33, 38}, {46, 53}, {61, 68}, {97, 105}, {134, 147}, {181, 197}, {243, 264}, {305, 332}, {382, 413}, {457, 493}}
var FrostboltLevel = [FrostboltRanks + 1]int{0, 4, 8, 14, 20, 26, 32, 38, 44, 50, 56, 60}

// The client stores .407 as a float32; the table widens it to 0.40700000524520874. Rounding back to
// the stated value keeps the sim's numbers where they were. Every coefficient read from the table
// goes through this.
func roundCoef(coef float64) float64 {
	return math.Round(coef*1e6) / 1e6
}

func (mage *Mage) registerFrostboltSpell() {
	mage.Frostbolt = make([]*core.Spell, FrostboltRanks+1)

	maxRank := core.TernaryInt(core.IncludeAQ, FrostboltRanks, FrostboltRanks-1)
	for rank := 1; rank <= maxRank; rank++ {
		config := mage.getFrostboltConfig(rank)

		if config.RequiredLevel <= int(mage.Level) {
			mage.Frostbolt[rank] = mage.GetOrRegisterSpell(config)
		}
	}
}

func (mage *Mage) getFrostboltConfig(rank int) core.SpellConfig {
	row := spellData.Frostbolt.ByRank(int32(rank))
	baseDamageLow := FrostboltBaseDamage[rank][0]
	baseDamageHigh := FrostboltBaseDamage[rank][1]
	level := FrostboltLevel[rank]

	return core.SpellConfig{
		ActionID:       core.ActionID{SpellID: row.SpellID},
		SpellCode:      SpellCode_MageFrostbolt,
		ClassSpellMask: SpellMaskFrostbolt,
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskSpellDamage,
		Flags:          SpellFlagMage | SpellFlagChillSpell | core.SpellFlagBinary | core.SpellFlagAPL,
		MissileSpeed:   row.MissileSpeed,

		RequiredLevel: level,
		Rank:          rank,

		ManaCost: core.ManaCostOptions{
			FlatCost: float64(row.Cost),
		},
		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD:      core.GCDDefault,
				CastTime: row.CastTime - time.Millisecond*100*time.Duration(mage.Talents.ImprovedFrostbolt),
			},
		},

		DamageMultiplier: 1,
		ThreatMultiplier: 1,
		BonusCoefficient: roundCoef(row.Direct.BonusCoefficient()),

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			baseDamage := sim.Roll(baseDamageLow, baseDamageHigh)
			result := spell.CalcDamage(sim, target, baseDamage, spell.OutcomeMagicHitAndCrit)

			spell.WaitTravelTime(sim, func(sim *core.Simulation) {
				if result.Landed() {
					spell.DealDamage(sim, result)
				}
			})
		},
	}
}
