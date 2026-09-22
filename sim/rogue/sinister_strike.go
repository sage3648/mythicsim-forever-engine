package rogue

import (
	"time"

	"github.com/wowsims/classic/sim/common/shared"
	"github.com/wowsims/classic/sim/core"
)

// Forever beta client 1.60.1.69893: cost, school, defense type, flat damage and coefficient come from
// the client table. The id stays ours: one rank a level is registered, and reading it from the table
// would make spell_sources_test.go file the other ranks as registered.
func (rogue *Rogue) registerSinisterStrikeSpell() {
	spellID := map[int32]int32{
		25: 1759,
		40: 8621,
		50: 11293,
		60: 11294,
	}[rogue.Level]

	row := spellData.SinisterStrike.BySpellID(spellID)
	flatDamageBonus := shared.SpellDataMin(row.Direct)

	rogue.SinisterStrike = rogue.RegisterSpell(core.SpellConfig{
		SpellCode:      SpellCode_RogueSinisterStrike,
		ClassSpellMask: SpellMaskSinisterStrike,
		ActionID:       core.ActionID{SpellID: spellID},
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskMeleeMHSpecial,
		Flags:          rogue.builderFlags(),

		EnergyCost: core.EnergyCostOptions{
			Cost:   float64(row.Cost) - []float64{0, 3, 5}[rogue.Talents.ImprovedSinisterStrike],
			Refund: 0.8,
		},
		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD: time.Second,
			},
			IgnoreHaste: true,
		},

		CritDamageBonus: rogue.lethality(),

		DamageMultiplier: []float64{1, 1.02, 1.04, 1.06}[rogue.Talents.Aggression],
		ThreatMultiplier: 1,
		BonusCoefficient: row.Direct.BonusCoefficient(),

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			rogue.BreakStealth(sim)

			baseDamage := (flatDamageBonus + spell.Unit.MHNormalizedWeaponDamage(sim, spell.MeleeAttackPower(target))) * rogue.quietusMultiplier(sim)
			result := spell.CalcAndDealDamage(sim, target, baseDamage, spell.OutcomeMeleeWeaponSpecialHitAndCrit)

			if result.Landed() {
				rogue.AddComboPoints(sim, 1, target, spell.ComboPointMetrics())
			} else {
				spell.IssueRefund(sim)
			}
		},
	})
}
