package hunter

import (
	"github.com/wowsims/classic/sim/common/shared"
	"github.com/wowsims/classic/sim/core"
)

// The bleed lasts 21 sec and carries 40% of the Mongoose Bite that applied it. The tick count and
// length, school and defense type come from the bleed's row of the client table (1310536, 7 ticks 3 sec
// apart); its id does not, the bleed is reported under Mongoose Bite's.
func (hunter *Hunter) registerLaceratingStrikesSpell() {
	if !hunter.Talents.LaceratingStrikes {
		return
	}

	row := spellData.LaceratingStrikesTriggered.ByRank(1)
	periodic := row.Periodic.(shared.SpellDataPeriodic)

	hunter.LaceratingStrikes = hunter.RegisterSpell(core.SpellConfig{
		SpellCode:      SpellCode_HunterLaceratingStrikes,
		ClassSpellMask: SpellMaskLaceratingStrikes,
		ActionID:       hunter.MongooseBite.WithTag(1),
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskEmpty,
		Flags:          core.SpellFlagMeleeMetrics | core.SpellFlagNoOnCastComplete | core.SpellFlagPureDot,

		DamageMultiplier: 1,
		ThreatMultiplier: 1,

		Dot: core.DotConfig{
			Aura: core.Aura{
				Label: "Lacerating Strikes" + hunter.Label,
			},
			NumberOfTicks: periodic.NumberOfTicks,
			TickLength:    periodic.TickLength,

			OnTick: func(sim *core.Simulation, target *core.Unit, dot *core.Dot) {
				dot.CalcAndDealPeriodicSnapshotDamage(sim, target, dot.OutcomeTick)
			},
		},

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			spell.Dot(target).Apply(sim)
		},
	})
}

func (hunter *Hunter) procLaceratingStrikes(sim *core.Simulation, result *core.SpellResult) {
	dot := hunter.LaceratingStrikes.Dot(result.Target)
	dot.SnapshotBaseDamage = result.Damage * 0.4 / float64(dot.NumberOfTicks)
	dot.SnapshotAttackerMultiplier = 1

	hunter.LaceratingStrikes.Cast(sim, result.Target)
}
