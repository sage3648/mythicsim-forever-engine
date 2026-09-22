package warlock

import (
	"github.com/wowsims/classic/sim/core"
)

const DeathCoilRanks = 3

func (warlock *Warlock) getDeathCoilBaseConfig(rank int) core.SpellConfig {
	// Beta client 1.60.1 values: slightly less damage, slightly more mana. Spell ID, cost, cooldown,
	// school and missile speed come from the client table; its damage row is empty (the value sits on
	// a dummy effect), so the damage and coefficient stay ours.
	row := spellData.DeathCoil.ByRank(int32(rank))
	spellId := row.SpellID
	baseDamage := [DeathCoilRanks + 1]float64{0, 285, 375, 460}[rank]
	level := [DeathCoilRanks + 1]int{0, 42, 50, 58}[rank]
	spellCoeff := 0.214

	healingSpell := warlock.GetOrRegisterSpell(core.SpellConfig{
		ActionID:    core.ActionID{SpellID: spellId}.WithTag(1),
		SpellSchool: core.SpellSchoolPhysical,
		ProcMask:    core.ProcMaskSpellHealing,
		Flags:       core.SpellFlagPassiveSpell | core.SpellFlagHelpful,

		DamageMultiplier: 1,
		ThreatMultiplier: 0,
	})

	return core.SpellConfig{
		SpellCode:      SpellCode_WarlockDeathCoil,
		ClassSpellMask: SpellMaskDeathCoil,
		ActionID:       core.ActionID{SpellID: spellId},
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskSpellDamage,
		Flags:          core.SpellFlagAPL | core.SpellFlagResetAttackSwing | core.SpellFlagBinary | WarlockFlagAffliction,
		RequiredLevel:  level,
		Rank:           rank,
		MissileSpeed:   row.MissileSpeed,

		ManaCost: core.ManaCostOptions{
			FlatCost: float64(row.Cost),
		},
		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD: core.GCDDefault,
			},
			CD: core.Cooldown{
				Timer:    warlock.NewTimer(),
				Duration: row.Cooldown,
			},
		},

		DamageMultiplierAdditive: 1,
		DamageMultiplier:         1,
		ThreatMultiplier:         1,
		BonusCoefficient:         spellCoeff,

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			results := spell.CalcDamage(sim, target, baseDamage, spell.OutcomeMagicHitAndCrit)

			spell.WaitTravelTime(sim, func(s *core.Simulation) {
				spell.DealDamage(sim, results)
				if results.Landed() {
					healingSpell.CalcAndDealHealing(sim, healingSpell.Unit, results.Damage, healingSpell.OutcomeHealing)
				}
			})
		},
	}
}

func (warlock *Warlock) registerDeathCoilSpell() {
	warlock.DeathCoil = make([]*core.Spell, 0)
	for rank := 1; rank <= DeathCoilRanks; rank++ {
		config := warlock.getDeathCoilBaseConfig(rank)

		if config.RequiredLevel <= int(warlock.Level) {
			warlock.DeathCoil = append(warlock.DeathCoil, warlock.GetOrRegisterSpell(config))
		}
	}
}
