package druid

import (
	"github.com/wowsims/classic/sim/common/shared"
	"github.com/wowsims/classic/sim/core"
	"github.com/wowsims/classic/sim/core/stats"
)

// Generates 20 Rage over 10 sec, but reduces base armor by 27% while it lasts. Forever adds 10 Rage up front
// (beta client 1.60.1.69893, effect 1 of 5229). The id, duration, cooldown and rage tick schedule come from the client
// table (see wrath.go); its 20 a tick is in tenths of Rage, so the 2 stays ours.
func (druid *Druid) registerEnrageSpell() {
	row := spellData.Enrage.ByRank(1)
	energize := row.Energize.(shared.SpellDataPeriodic)
	actionID := core.ActionID{SpellID: row.SpellID}
	rageMetrics := druid.NewRageMetrics(actionID)

	armorMultiplier := 1 - 0.27

	druid.EnrageAura = druid.RegisterAura(core.Aura{
		Label:    "Enrage",
		ActionID: actionID,
		Duration: row.Duration,
		OnGain: func(aura *core.Aura, sim *core.Simulation) {
			druid.ApplyDynamicEquipScaling(sim, stats.Armor, armorMultiplier)
		},
		OnExpire: func(aura *core.Aura, sim *core.Simulation) {
			druid.RemoveDynamicEquipScaling(sim, stats.Armor, armorMultiplier)
		},
	})

	druid.Enrage = druid.RegisterSpell(Bear, core.SpellConfig{
		ActionID: actionID,
		Flags:    core.SpellFlagAPL,

		Cast: core.CastConfig{
			CD: core.Cooldown{
				Timer:    druid.NewTimer(),
				Duration: row.Cooldown,
			},
			IgnoreHaste: true,
		},

		ApplyEffects: func(sim *core.Simulation, _ *core.Unit, _ *core.Spell) {
			if druid.Env.IsForever() {
				druid.AddRage(sim, 10, rageMetrics)
			}

			core.StartPeriodicAction(sim, core.PeriodicActionOptions{
				NumTicks: int(energize.NumberOfTicks),
				Period:   energize.TickLength,
				OnAction: func(sim *core.Simulation) {
					if druid.EnrageAura.IsActive() {
						druid.AddRage(sim, 2, rageMetrics)
					}
				},
			})

			druid.EnrageAura.Activate(sim)
		},
	})

	druid.AddMajorCooldown(core.MajorCooldown{
		Spell: druid.Enrage.Spell,
		Type:  core.CooldownTypeDPS,
	})
}
