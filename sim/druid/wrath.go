package druid

import (
	"math"
	"time"

	"github.com/wowsims/classic/sim/core"
)

// The client tables (spell_data_auto_gen.go) hold each rank's id, cost, cast time, cooldown, coefficients, school,
// defense type, missile speed and dot schedule, and the druid's spells read those from there. They hold the damage
// as one truncated centre value and no level, so damage ranges and levels stay ours; spell_damage_test.go checks
// every range kept here still contains the table's value.
//
// The client stores .429 as a float32; the table widens it. Rounding back to the stated value keeps the sim's
// numbers where they were (sim/mage/frostbolt.go). Every coefficient read from the table goes through this.
func roundCoef(coef float64) float64 {
	return math.Round(coef*1e6) / 1e6
}

const WrathRanks = 8

// Beta client 1.60.1.69893: a quarter of Classic's damage at every rank from 3 up, cheaper, and no downranking penalty on
// ranks 1-2. Ranges are the client's base plus its per level growth up to the rank's max level.
var WrathBaseDamage = [WrathRanks + 1][]float64{{0}, {10, 13}, {16, 19}, {21, 25}, {26, 31}, {31, 36}, {37, 42}, {46, 51}, {62, 69}}
var WrathLevel = [WrathRanks + 1]int{0, 1, 6, 14, 22, 30, 38, 46, 54}

func (druid *Druid) registerWrathSpell() {
	druid.Wrath = make([]*DruidSpell, WrathRanks+1)

	for rank := 1; rank <= WrathRanks; rank++ {
		config := druid.newWrathSpellConfig(rank)

		if config.RequiredLevel <= int(druid.Level) {
			druid.Wrath[rank] = druid.RegisterSpell(Humanoid|Moonkin, config)
		}
	}
}

func (druid *Druid) newWrathSpellConfig(rank int) core.SpellConfig {
	row := spellData.Wrath.ByRank(int32(rank))
	baseDamageLow := WrathBaseDamage[rank][0]
	baseDamageHigh := WrathBaseDamage[rank][1]
	level := WrathLevel[rank]

	return core.SpellConfig{
		ActionID:       core.ActionID{SpellID: row.SpellID},
		SpellCode:      SpellCode_DruidWrath,
		ClassSpellMask: SpellMaskWrath,
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskSpellDamage,
		Flags:          core.SpellFlagAPL | core.SpellFlagResetAttackSwing,

		RequiredLevel: level,
		Rank:          rank,
		MissileSpeed:  row.MissileSpeed,

		ManaCost: core.ManaCostOptions{
			FlatCost: float64(row.Cost),
			// Improved Wrath's beta client curves are 10-50% of the cost and 0.1-0.5 sec of the cast.
			Multiplier: 100 - 10*druid.Talents.ImprovedWrath,
		},
		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD:      core.GCDDefault,
				CastTime: row.CastTime - time.Millisecond*100*time.Duration(druid.Talents.ImprovedWrath),
			},
		},

		DamageMultiplier: 1, // + core.Ternary(druid.Ranged().ID == IdolOfWrath, .02, 0),
		ThreatMultiplier: 1,
		BonusCoefficient: roundCoef(row.Direct.BonusCoefficient()),

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			baseDamage := sim.Roll(baseDamageLow, baseDamageHigh)
			result := spell.CalcDamage(sim, target, baseDamage, spell.OutcomeMagicHitAndCrit)

			// NG procs when the cast finishes
			if result.DidCrit() {
				druid.procNaturesGrace(sim)
			}

			spell.WaitTravelTime(sim, func(sim *core.Simulation) {
				spell.DealDamage(sim, result)
			})
		},
	}
}
