package druid

import (
	"time"

	"github.com/wowsims/classic/sim/core"
)

// Rank 5 at every level. Its cost and flat bonus come from the client table (see wrath.go); the id stays ours, since
// reading it through the table files the never-registered ranks 1-4 in sim/spell_sources_test.go.
func (druid *Druid) registerClawSpell() {
	row := spellData.Claw.ByRank(5)
	flatDamageBonus, _ := row.Direct.Range()

	druid.Claw = druid.RegisterSpell(Cat, core.SpellConfig{
		SpellCode:      SpellCode_DruidClaw,
		ClassSpellMask: SpellMaskClaw,
		ActionID:       core.ActionID{SpellID: 9850},
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskMeleeMHSpecial,
		Flags:          core.SpellFlagMeleeMetrics | core.SpellFlagAPL | SpellFlagBuilder,

		EnergyCost: core.EnergyCostOptions{
			Cost:   float64(row.Cost) - 1*float64(druid.Talents.Ferocity),
			Refund: 0.8,
		},
		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD: time.Second,
			},
			IgnoreHaste: true,
		},

		DamageMultiplierAdditive: 1 + 0.05*float64(druid.Talents.SavageFury),
		// Beta client 1.60.1.69893: Claw now deals 110% weapon damage plus its bonus, where Classic's was 100%.
		DamageMultiplier: 1.1,
		ThreatMultiplier: 1,
		BonusCoefficient: 1,

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			baseDamage := flatDamageBonus + spell.Unit.MHWeaponDamage(sim, spell.MeleeAttackPower(target))

			result := spell.CalcAndDealDamage(sim, target, baseDamage, spell.OutcomeMeleeSpecialHitAndCrit)

			if result.Landed() {
				druid.AddComboPoints(sim, 1, target, spell.ComboPointMetrics())
			} else {
				spell.IssueRefund(sim)
			}
		},
	})
}
