package warrior

import (
	"github.com/wowsims/classic/sim/core"
)

func (warrior *Warrior) registerWhirlwindSpell() {
	results := make([]*core.SpellResult, min(4, warrior.Env.GetNumTargets()))
	ohResults := make([]*core.SpellResult, len(results))

	// Raging Blows adds an off-hand swing to every target Whirlwind hits.
	var whirlwindOh *core.Spell
	if warrior.Talents.RagingBlows && warrior.AutoAttacks.IsDualWielding {
		whirlwindOh = warrior.registerWhirlwindOffHandSpell()
	}

	// Forever beta client 1.60.1.69893: id, cost, cooldown, school, defense type and coefficient come
	// from the client table.
	row := spellData.Whirlwind.ByRank(1)

	warrior.Whirlwind = warrior.RegisterSpell(BerserkerStance, core.SpellConfig{
		SpellCode:      SpellCode_WarriorWhirlwind,
		ClassSpellMask: SpellMaskWhirlwind,
		ActionID:       core.ActionID{SpellID: row.SpellID},
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskMeleeMHSpecial,
		Flags:          core.SpellFlagAPL | SpellFlagOffensive,

		RageCost: core.RageCostOptions{
			Cost: float64(row.Cost),
		},
		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD: core.GCDDefault,
			},
			IgnoreHaste: true,
			CD: core.Cooldown{
				Timer:    warrior.NewTimer(),
				Duration: row.Cooldown,
			},
		},
		CritDamageBonus: warrior.impale(),

		DamageMultiplier: 1,
		ThreatMultiplier: 1.25,
		BonusCoefficient: row.Direct.BonusCoefficient(),

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			mhTarget := target
			for idx := range results {
				baseDamage := spell.Unit.MHNormalizedWeaponDamage(sim, spell.MeleeAttackPower(target))
				results[idx] = spell.CalcDamage(sim, target, baseDamage, spell.OutcomeMeleeWeaponSpecialHitAndCrit)
				target = sim.Environment.NextTargetUnit(target)
			}

			if whirlwindOh != nil {
				target = mhTarget
				for idx := range ohResults {
					baseDamage := whirlwindOh.Unit.OHNormalizedWeaponDamage(sim, whirlwindOh.MeleeAttackPower(target))
					ohResults[idx] = whirlwindOh.CalcDamage(sim, target, baseDamage, whirlwindOh.OutcomeMeleeWeaponSpecialHitAndCrit)
					target = sim.Environment.NextTargetUnit(target)
				}
			}

			for _, result := range results {
				spell.DealDamage(sim, result)
			}

			if whirlwindOh != nil {
				for _, result := range ohResults {
					whirlwindOh.DealDamage(sim, result)
				}
			}
		},
	})
}

func (warrior *Warrior) registerWhirlwindOffHandSpell() *core.Spell {
	return warrior.RegisterSpell(BerserkerStance, core.SpellConfig{
		ActionID:    core.ActionID{SpellID: 1680}.WithTag(2),
		SpellSchool: core.SpellSchoolPhysical,
		DefenseType: core.DefenseTypeMelee,
		ProcMask:    core.ProcMaskMeleeOHSpecial,
		Flags:       core.SpellFlagMeleeMetrics | core.SpellFlagNoOnCastComplete | core.SpellFlagPassiveSpell,

		CritDamageBonus: warrior.impale(),

		DamageMultiplier: warrior.AutoAttacks.OHConfig().DamageMultiplier,
		ThreatMultiplier: 1.25,
		BonusCoefficient: 1,
	}).Spell
}
