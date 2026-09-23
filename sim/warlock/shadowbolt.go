package warlock

import (
	"math"

	"github.com/wowsims/classic/sim/core"
)

const ShadowBoltRanks = 10

// Spell ID, cost, cast time, cooldown, coefficient, school and dot ticks of the warlock's spells come
// from the client table (spell_data_auto_gen.go, vendored from wowsims/forever). Damage ranges do not:
// the client rolls a spread the vendored generator drops, keeping only the truncated centre (see
// sim/mage/frostbolt.go). spell_damage_test.go checks every range kept here still contains it.
//
// Beta client 1.60.1: every rank's damage moved and the low ranks lost their downranking penalty.
// Damage is each rank's value at the level it stops scaling at (capped at 60), as the Classic table was.
var ShadowBoltBaseDamage = [ShadowBoltRanks + 1][]float64{{0}, {12, 16}, {25, 31}, {41, 48}, {57, 64}, {79, 89}, {101, 113}, {140, 156}, {188, 210}, {237, 265}, {253, 283}}

// The client stores .486 as a float32; the table widens it. Rounding back to the stated value keeps
// the sim's numbers where they were (sim/mage/frostbolt.go). Every coefficient read from the table
// goes through this.
func roundCoef(coef float64) float64 {
	return math.Round(coef*1e6) / 1e6
}

func (warlock *Warlock) getShadowBoltBaseConfig(rank int) core.SpellConfig {
	row := spellData.ShadowBolt.ByRank(int32(rank))
	baseDamage := ShadowBoltBaseDamage[rank]
	level := [ShadowBoltRanks + 1]int{0, 1, 6, 12, 20, 28, 36, 44, 52, 60, 60}[rank]

	return core.SpellConfig{
		SpellCode:      SpellCode_WarlockShadowBolt,
		ClassSpellMask: SpellMaskShadowBolt,
		ActionID:       core.ActionID{SpellID: row.SpellID},
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		MissileSpeed:   row.MissileSpeed,
		ProcMask:       core.ProcMaskSpellDamage,
		Flags:          core.SpellFlagAPL | core.SpellFlagResetAttackSwing | WarlockFlagDestruction,
		RequiredLevel:  level,
		Rank:           rank,

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
			result := spell.CalcDamage(sim, target, sim.Roll(baseDamage[0], baseDamage[1]), spell.OutcomeMagicHitAndCrit)
			spell.WaitTravelTime(sim, func(sim *core.Simulation) {
				spell.DealDamage(sim, result)
			})
		},
	}
}

func (warlock *Warlock) registerShadowBoltSpell() {
	warlock.ShadowBolt = make([]*core.Spell, 0)

	maxRank := core.TernaryInt(core.IncludeAQ, ShadowBoltRanks, ShadowBoltRanks-1)
	for rank := 1; rank <= maxRank; rank++ {
		config := warlock.getShadowBoltBaseConfig(rank)

		if config.RequiredLevel <= int(warlock.Level) {
			warlock.ShadowBolt = append(warlock.ShadowBolt, warlock.GetOrRegisterSpell(config))
		}
	}
}
