package rogue

import (
	"time"

	"github.com/wowsims/classic/sim/common/shared"
	"github.com/wowsims/classic/sim/core"
)

// Cost, school, defense type, flat damage and coefficient come from the client table; the id stays
// ours (see sinister_strike.go).
func (rogue *Rogue) registerBackstabSpell() {
	spellID := map[int32]int32{
		25: 2590,
		40: 8721,
		50: 11279,
		60: core.TernaryInt32(core.IncludeAQ, 25300, 11281),
	}[rogue.Level]

	row := spellData.Backstab.BySpellID(spellID)
	flatDamageBonus := shared.SpellDataMin(row.Direct)

	damageMultiplier := 1.5 *
		[]float64{1, 1.05, 1.1}[rogue.Talents.Opportunity] *
		[]float64{1, 1.02, 1.04, 1.06}[rogue.Talents.Aggression]

	// Puncturing Wounds gives 15% extra combo point chance and 10% crit per rank (beta client).
	extraComboPointChance := 0.15 * float64(rogue.Talents.PuncturingWounds)
	cpMetrics := rogue.NewComboPointMetrics(core.ActionID{SpellID: 13866})

	rogue.Backstab = rogue.RegisterSpell(core.SpellConfig{
		SpellCode:      SpellCode_RogueBackstab,
		ClassSpellMask: SpellMaskBackstab,
		ActionID:       core.ActionID{SpellID: spellID},
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskMeleeMHSpecial,
		Flags:          rogue.builderFlags(),

		EnergyCost: core.EnergyCostOptions{
			Cost:   float64(row.Cost),
			Refund: 0.8,
		},
		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD: time.Second,
			},
			IgnoreHaste: true,
		},
		ExtraCastCondition: func(sim *core.Simulation, target *core.Unit) bool {
			if !rogue.HasDagger(core.MainHand) {
				return false
			}
			return !rogue.PseudoStats.InFrontOfTarget
		},

		BonusCritRating: 10 * core.CritRatingPerCritChance * float64(rogue.Talents.PuncturingWounds),

		CritDamageBonus: rogue.lethality(),

		DamageMultiplier: damageMultiplier,
		ThreatMultiplier: 1,
		BonusCoefficient: row.Direct.BonusCoefficient(),

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			rogue.BreakStealth(sim)
			baseDamage := (flatDamageBonus + spell.Unit.MHNormalizedWeaponDamage(sim, spell.MeleeAttackPower(target)))
			result := spell.CalcAndDealDamage(sim, target, baseDamage, spell.OutcomeMeleeWeaponSpecialHitAndCrit)

			if result.Landed() {
				rogue.AddComboPoints(sim, 1, target, spell.ComboPointMetrics())
				if sim.Proc(extraComboPointChance, "Puncturing Wounds") {
					rogue.AddComboPoints(sim, 1, target, cpMetrics)
				}
			} else {
				spell.IssueRefund(sim)
			}
		},
	})
}
