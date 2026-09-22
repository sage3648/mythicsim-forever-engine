package warlock

import (
	"github.com/wowsims/classic/sim/common/shared"
	"github.com/wowsims/classic/sim/core"
)

func (warlock *Warlock) registerWrackSpell() {
	if !warlock.Talents.Wrack {
		return
	}

	// Beta client 1.60.1 (spell 1316697): 36 per tick with a 0.143 coefficient, 6 sec channel,
	// 200 mana, all read from the client table. The action id stays 11704, which the APLs name.
	row := spellData.Wrack.ByRank(1)
	periodic := row.Periodic.(shared.SpellDataPeriodic)

	warlock.Wrack = warlock.RegisterSpell(core.SpellConfig{
		SpellCode:      SpellCode_WarlockWrack,
		ClassSpellMask: SpellMaskWrack,
		ActionID:       core.ActionID{SpellID: 11704},
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskSpellDamage,
		Flags:          core.SpellFlagAPL | core.SpellFlagChanneled | core.SpellFlagResetAttackSwing | WarlockFlagAffliction,

		RequiredLevel: 60,

		ManaCost: core.ManaCostOptions{
			FlatCost: float64(row.Cost),
		},
		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD: core.GCDDefault,
			},
		},

		DamageMultiplier: 1,
		ThreatMultiplier: 1,

		Dot: core.DotConfig{
			Aura: core.Aura{
				Label: "Wrack-" + warlock.Label,
			},
			NumberOfTicks:    periodic.NumberOfTicks,
			TickLength:       periodic.TickLength,
			BonusCoefficient: roundCoef(periodic.Coef),

			OnSnapshot: func(sim *core.Simulation, target *core.Unit, dot *core.Dot, isRollover bool) {
				dot.Snapshot(target, periodic.Tick, isRollover)
			},
			OnTick: func(sim *core.Simulation, target *core.Unit, dot *core.Dot) {
				result := dot.CalcSnapshotDamage(sim, target, dot.OutcomeTick)
				result.Damage *= warlock.improvedDrainsMultiplier(sim, target)
				dot.Spell.DealPeriodicDamage(sim, result)
			},
		},

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			result := spell.CalcOutcome(sim, target, spell.OutcomeMagicHitNoHitCounter)
			if result.Landed() {
				dot := spell.Dot(target)
				dot.Apply(sim)
			}
			spell.DealOutcome(sim, result)
		},
	})

	// Other shadow dots on the target tick for 10% more while Wrack is on it
	for _, target := range warlock.Env.Encounter.TargetUnits {
		target.AddDynamicDamageTakenModifier(func(sim *core.Simulation, spell *core.Spell, result *core.SpellResult) {
			if spell.Unit != &warlock.Unit || spell == warlock.Wrack {
				return
			}

			if spell.SpellSchool.Matches(core.SpellSchoolShadow) && len(spell.Dots()) > 0 && warlock.Wrack.Dot(result.Target).IsActive() {
				result.Damage *= 1.1
			}
		})
	}
}
