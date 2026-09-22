package warrior

import (
	"github.com/wowsims/classic/sim/common/shared"
	"github.com/wowsims/classic/sim/core"
)

func (warrior *Warrior) registerHamstringSpell() {
	// Cost, school, defense type, flat damage and coefficient come from the client table; the id and
	// threat stay ours (see registerHeroicStrikeSpell).
	spellID := int32(7373)
	row := spellData.Hamstring.BySpellID(spellID)
	damage := shared.SpellDataMin(row.Direct)
	spell_level := 54.0

	warrior.Hamstring = warrior.RegisterSpell(BattleStance|BerserkerStance, core.SpellConfig{
		ActionID:    core.ActionID{SpellID: spellID},
		SpellSchool: row.SpellSchool,
		DefenseType: row.DefenseType,
		ProcMask:    core.ProcMaskMeleeMHSpecial,
		Flags:       core.SpellFlagMeleeMetrics | core.SpellFlagAPL | core.SpellFlagBinary | SpellFlagOffensive,

		RageCost: core.RageCostOptions{
			Cost:   float64(row.Cost),
			Refund: 0.8,
		},
		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD: core.GCDDefault,
			},
		},

		CritDamageBonus: warrior.impale(),

		DamageMultiplier: 1,
		ThreatMultiplier: 1.25,
		FlatThreatBonus:  1.25 * 2 * float64(spell_level),
		BonusCoefficient: row.Direct.BonusCoefficient(),

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			result := spell.CalcAndDealDamage(sim, target, damage, spell.OutcomeMeleeWeaponSpecialHitAndCrit)

			if !result.Landed() {
				spell.IssueRefund(sim)
			}
		},
	})
}
