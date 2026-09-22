package warrior

import (
	"github.com/wowsims/classic/sim/common/shared"
	"github.com/wowsims/classic/sim/core"
)

func (warrior *Warrior) registerBloodrageCD() {
	// Forever beta client 1.60.1.69893: id, cooldown, duration, Rage (the client counts it in tenths),
	// tick count and tick length come from the client table.
	row := spellData.Bloodrage.ByRank(1)
	auraRow := spellData.BloodrageTriggered.ByRank(1)
	periodic := auraRow.Energize.(shared.SpellDataPeriodic)

	actionID := core.ActionID{SpellID: row.SpellID}
	rageMetrics := warrior.NewRageMetrics(actionID)

	// Improved Bloodrage scales all of the Rage the ability makes now, not just the instant hit.
	rageMultiplier := 1 + 0.25*float64(warrior.Talents.ImprovedBloodrage)
	instantRage := shared.SpellDataMin(row.Energize) / 10 * rageMultiplier
	ragePerSec := periodic.Tick / 10 * rageMultiplier

	warrior.BloodrageAura = warrior.RegisterAura(core.Aura{
		Label:    "Bloodrage",
		ActionID: actionID,
		Duration: auraRow.Duration,
	})

	warrior.Bloodrage = warrior.RegisterSpell(AnyStance, core.SpellConfig{
		ActionID: actionID,
		Cast: core.CastConfig{
			CD: core.Cooldown{
				Timer:    warrior.NewTimer(),
				Duration: row.Cooldown,
			},
		},

		ApplyEffects: func(sim *core.Simulation, _ *core.Unit, _ *core.Spell) {
			warrior.BloodrageAura.Activate(sim)
			warrior.AddRage(sim, instantRage, rageMetrics)

			core.StartPeriodicAction(sim, core.PeriodicActionOptions{
				NumTicks: int(periodic.NumberOfTicks),
				Period:   periodic.TickLength,
				OnAction: func(sim *core.Simulation) {
					warrior.AddRage(sim, ragePerSec, rageMetrics)
				},
			})
		},
	})

	warrior.AddMajorCooldown(core.MajorCooldown{
		Spell: warrior.Bloodrage.Spell,
		Type:  core.CooldownTypeDPS,
	})
}
