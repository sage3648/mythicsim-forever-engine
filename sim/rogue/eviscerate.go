package rogue

import (
	"time"

	"github.com/wowsims/classic/sim/core"
)

// Our damage by spell id: the low end with no combo points, the step per combo point, and the width of
// the roll. The client table holds only the centre of the range with no combo points
// (spell_damage_test.go checks it), and the per combo point step sits on a dummy effect that reads 0.
var eviscerateDamage = map[int32]struct{ flat, perCombo, variance float64 }{
	6762:  {10, 31, 20},
	8624:  {22, 71, 44},
	11299: {34, 110, 68},
	11300: {48, 151, 96},
	31016: {54, 170, 108},
}

func (rogue *Rogue) registerEviscerate() {
	spellID := map[int32]int32{
		25: 6762,
		40: 8624,
		50: 11299,
		60: core.TernaryInt32(core.IncludeAQ, 31016, 11300),
	}[rogue.Level]

	// Cost, school, defense type and coefficient come from the client table; the id stays ours (see
	// sinister_strike.go), and so does the damage (see eviscerateDamage).
	row := spellData.Eviscerate.BySpellID(spellID)
	damage := eviscerateDamage[spellID]
	flatDamage, comboDamageBonus, damageVariance := damage.flat, damage.perCombo, damage.variance

	rogue.Eviscerate = rogue.RegisterSpell(core.SpellConfig{
		SpellCode:      SpellCode_RogueEviscerate,
		ClassSpellMask: SpellMaskEviscerate,
		ActionID:       core.ActionID{SpellID: spellID},
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskMeleeMHSpecial,
		Flags:          rogue.finisherFlags() | SpellFlagColdBlooded,
		MetricSplits:   6,

		EnergyCost: core.EnergyCostOptions{
			Cost:   float64(row.Cost) - core.TernaryFloat64(rogue.Talents.FlawlessExecution, 10, 0),
			Refund: 0,
		},
		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD: time.Second,
			},
			IgnoreHaste: true,
			ModifyCast: func(sim *core.Simulation, spell *core.Spell, cast *core.Cast) {
				spell.SetMetricsSplit(spell.Unit.ComboPoints())
			},
		},
		ExtraCastCondition: func(sim *core.Simulation, target *core.Unit) bool {
			return rogue.ComboPoints() > 0
		},

		// Ranks 2 and 3 were extrapolated from rank 1's 7% until the beta showed 13% and
		// 20%, so the talent is slightly weaker than a straight multiple.
		DamageMultiplier: 1 +
			[]float64{0, 0.07, 0.13, 0.20}[rogue.Talents.ImprovedEviscerate] +
			[]float64{0, 0.02, 0.04, 0.06}[rogue.Talents.Aggression],
		ThreatMultiplier: 1,
		BonusCoefficient: row.Direct.BonusCoefficient(),

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			rogue.BreakStealth(sim)

			comboPoints := rogue.ComboPoints()
			flatBaseDamage := flatDamage + comboDamageBonus*float64(comboPoints)

			baseDamage := sim.Roll(flatBaseDamage, flatBaseDamage+damageVariance) +
				0.03*float64(comboPoints)*spell.MeleeAttackPower(target)

			result := spell.CalcDamage(sim, target, baseDamage, spell.OutcomeMeleeSpecialHitAndCrit)

			if result.Landed() {
				rogue.SpendComboPoints(sim, spell)
			} else {
				spell.IssueRefund(sim)
			}

			spell.DealDamage(sim, result)
		},
	})
	rogue.Finishers = append(rogue.Finishers, rogue.Eviscerate)
}
