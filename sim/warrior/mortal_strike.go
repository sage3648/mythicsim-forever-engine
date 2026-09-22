package warrior

import (
	"github.com/wowsims/classic/sim/common/shared"
	"github.com/wowsims/classic/sim/core"
)

func (warrior *Warrior) registerMortalStrikeSpell(cdTimer *core.Timer) {
	if !warrior.Talents.MortalStrike {
		return
	}

	// Rank 4 (21553) in the beta client, as in Classic. The talent tooltip's 85 is rank 1's (12294).
	// Cost, cooldown, school, defense type, flat damage and coefficient come from the client table; the
	// id stays ours (see registerHeroicStrikeSpell).
	spellID := int32(21553)
	row := spellData.MortalStrike.BySpellID(spellID)
	bonusDamage := shared.SpellDataMin(row.Direct)

	warrior.MortalStrike = warrior.RegisterSpell(AnyStance, core.SpellConfig{
		SpellCode:      SpellCode_WarriorMortalStrike,
		ClassSpellMask: SpellMaskMortalStrike,
		ActionID:       core.ActionID{SpellID: spellID},
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskMeleeMHSpecial,
		Flags:          core.SpellFlagMeleeMetrics | core.SpellFlagAPL | SpellFlagOffensive,

		RageCost: core.RageCostOptions{
			Cost:   float64(row.Cost),
			Refund: 0.8,
		},
		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD: core.GCDDefault,
			},
			IgnoreHaste: true,
			CD: core.Cooldown{
				Timer:    cdTimer,
				Duration: row.Cooldown,
			},
		},

		CritDamageBonus: warrior.impale(),

		DamageMultiplier: 1,
		ThreatMultiplier: 1,
		BonusCoefficient: row.Direct.BonusCoefficient(),

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			baseDamage := bonusDamage + spell.Unit.MHNormalizedWeaponDamage(sim, spell.MeleeAttackPower(target))

			result := spell.CalcAndDealDamage(sim, target, baseDamage, spell.OutcomeMeleeWeaponSpecialHitAndCrit)

			if !result.Landed() {
				spell.IssueRefund(sim)
			}
		},
	})
}
