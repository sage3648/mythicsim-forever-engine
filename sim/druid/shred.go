package druid

import (
	"time"

	"github.com/wowsims/classic/sim/core"
)

func (druid *Druid) registerShredSpell() {
	// Beta client 1.60.1.69893: 155% weapon damage, down from Classic's 225%, with the same flat bonus per rank.
	damageMultiplier := 1.55

	// The cost and flat bonus come from the client table (see wrath.go). The ids stay ours: reading them through the
	// table files the never-registered rank 2 in sim/spell_sources_test.go.
	spellID := map[int32]int32{
		25: 5221,
		40: 8992,
		50: 9829,
		60: 9830,
	}[druid.Level]
	row := spellData.Shred.BySpellID(spellID)
	flatDamageBonus, _ := row.Direct.Range()

	druid.Shred = druid.RegisterSpell(Cat, core.SpellConfig{
		SpellCode:      SpellCode_DruidShred,
		ClassSpellMask: SpellMaskShred,
		ActionID:       core.ActionID{SpellID: spellID},
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskMeleeMHSpecial,
		Flags:          core.SpellFlagMeleeMetrics | core.SpellFlagAPL | SpellFlagBuilder,

		EnergyCost: core.EnergyCostOptions{
			Cost:   float64(row.Cost) - 6*float64(druid.Talents.ShreddingAttacks),
			Refund: 0.8,
		},
		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD: time.Second,
			},
			IgnoreHaste: true,
		},
		ExtraCastCondition: func(sim *core.Simulation, target *core.Unit) bool {
			return !druid.PseudoStats.InFrontOfTarget
		},

		DamageMultiplier: damageMultiplier,
		// Savage Fury names Shred under Forever, where Classic's did not.
		DamageMultiplierAdditive: 1 + 0.05*float64(druid.Talents.SavageFury),
		ThreatMultiplier:         1,
		BonusCoefficient:         1,

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			baseDamage := flatDamageBonus + spell.Unit.MHWeaponDamage(sim, spell.MeleeAttackPower(target))

			result := spell.CalcAndDealDamage(sim, target, baseDamage, spell.OutcomeMeleeSpecialHitAndCrit)

			if result.Landed() {
				druid.AddComboPoints(sim, 1, target, spell.ComboPointMetrics())
			} else {
				spell.IssueRefund(sim)
			}
		},
		ExpectedInitialDamage: func(sim *core.Simulation, target *core.Unit, spell *core.Spell, _ bool) *core.SpellResult {
			baseDamage := flatDamageBonus + spell.Unit.AutoAttacks.MH().CalculateAverageWeaponDamage(spell.MeleeAttackPower(target))

			baseres := spell.CalcDamage(sim, target, baseDamage, spell.OutcomeExpectedMagicAlwaysHit)

			attackTable := spell.Unit.AttackTables[target.UnitIndex][spell.CastType]
			critChance := spell.PhysicalCritChance(attackTable)
			critMod := critChance * (spell.CritMultiplier(attackTable) - 1)

			baseres.Damage *= 1 + critMod

			return baseres
		},
	})
}
