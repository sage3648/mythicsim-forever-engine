package warrior

import (
	"github.com/wowsims/classic/sim/core"
)

func (warrior *Warrior) registerSunderArmorSpell() {
	warrior.SunderArmorAuras = warrior.NewEnemyAuraArray(core.SunderArmorAura)

	spellID := int32(11597)
	// Cost, school and threat come from the client table; the id stays ours (see
	// registerHeroicStrikeSpell). The table's Melee defense type is not applied, as before.
	row := spellData.SunderArmor.BySpellID(spellID)

	// Forever gives Sunder Armor an explicit threat effect, 1013 at rank 5, where Classic's
	// 2.25 x 2 x level (261) was server side.
	threat := row.FlatThreatBonus

	var canApplySunder bool

	warrior.SunderArmor = warrior.RegisterSpell(AnyStance, core.SpellConfig{
		ActionID:    core.ActionID{SpellID: spellID},
		SpellSchool: row.SpellSchool,
		ProcMask:    core.ProcMaskMeleeMHSpecial,
		Flags:       core.SpellFlagMeleeMetrics | core.SpellFlagAPL | SpellFlagOffensive,

		RageCost: core.RageCostOptions{
			Cost:   float64(row.Cost) - float64(warrior.Talents.ImprovedSunderArmor),
			Refund: 0.8,
		},
		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD: core.GCDDefault,
			},
			IgnoreHaste: true,
		},
		ExtraCastCondition: func(sim *core.Simulation, target *core.Unit) bool {
			sa := warrior.SunderArmorAuras.Get(target)
			if sa.IsActive() {
				canApplySunder = true
			} else if sa.ExclusiveEffects[0].Category.AnyActive() {
				canApplySunder = false
			} else {
				canApplySunder = true
			}
			return canApplySunder
		},

		ThreatMultiplier: 1,
		FlatThreatBonus:  threat,

		RelatedAuras: []core.AuraArray{warrior.SunderArmorAuras},

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			result := spell.CalcAndDealOutcome(sim, target, spell.OutcomeMeleeWeaponSpecialNoCrit) // Cannot be blocked
			if !result.Landed() {
				spell.IssueRefund(sim)
				return
			}

			if canApplySunder {
				sa := warrior.SunderArmorAuras.Get(target)
				sa.Activate(sim)
				sa.AddStack(sim)
			}
		},
	})
}
