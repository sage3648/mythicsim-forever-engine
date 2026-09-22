package rogue

import (
	"time"

	"github.com/wowsims/classic/sim/common/shared"
	"github.com/wowsims/classic/sim/core"
)

// Cost, school, defense type, flat damage and coefficient come from the client table; the id stays
// ours (see sinister_strike.go).
func (rogue *Rogue) registerAmbushSpell() {
	spellID := map[int32]int32{
		25: 8676,
		40: 8725,
		50: 11268,
		60: 11269,
	}[rogue.Level]

	row := spellData.Ambush.BySpellID(spellID)
	flatDamageBonus := shared.SpellDataMin(row.Direct)

	damageMultiplier := 2.5 * []float64{1, 1.05, 1.1}[rogue.Talents.Opportunity]

	rogue.Ambush = rogue.RegisterSpell(core.SpellConfig{
		SpellCode:      SpellCode_RogueAmbush,
		ClassSpellMask: SpellMaskAmbush,
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
			// Cutthroat lets the Stealth requirement slide for a short while after a Backstab.
			return rogue.IsStealthed() || (rogue.CutthroatAura != nil && rogue.CutthroatAura.IsActive())
		},

		BonusCritRating:  15 * core.CritRatingPerCritChance * float64(rogue.Talents.ImprovedAmbush),
		DamageMultiplier: damageMultiplier,
		ThreatMultiplier: 1,
		BonusCoefficient: row.Direct.BonusCoefficient(),

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			rogue.BreakStealth(sim)
			if rogue.CutthroatAura != nil {
				rogue.CutthroatAura.Deactivate(sim)
			}
			baseDamage := (flatDamageBonus + spell.Unit.MHNormalizedWeaponDamage(sim, spell.MeleeAttackPower(target)))

			result := spell.CalcAndDealDamage(sim, target, baseDamage, spell.OutcomeMeleeSpecialNoBlockDodgeParry)

			if result.Landed() {
				rogue.AddComboPoints(sim, 1, target, spell.ComboPointMetrics())
			} else {
				spell.IssueRefund(sim)
			}
		},
	})
}
