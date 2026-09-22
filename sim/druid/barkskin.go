package druid

import (
	"github.com/wowsims/classic/sim/core"
	"github.com/wowsims/classic/sim/core/stats"
)

func (druid *Druid) registerBarkskinCD() {
	if !druid.InForm(Bear) {
		return
	}

	// Beta client 1.60.1.69893: 20% less Physical damage taken for 15 sec, no cost and no cast time penalty. The id,
	// duration and cooldown come from the client table (see wrath.go).
	row := spellData.Barkskin.ByRank(1)
	actionId := core.ActionID{SpellID: row.SpellID}

	druid.BarkskinAura = druid.RegisterAura(core.Aura{
		Label:    "Barkskin",
		ActionID: actionId,
		Duration: row.Duration,
	}).AttachMultiplicativePseudoStatBuff(&druid.PseudoStats.SchoolDamageTakenMultiplier[stats.SchoolIndexPhysical], 0.8)

	druid.Barkskin = druid.RegisterSpell(Any, core.SpellConfig{
		ActionID: actionId,
		Flags:    core.SpellFlagAPL,
		Cast: core.CastConfig{
			CD: core.Cooldown{
				Timer:    druid.NewTimer(),
				Duration: row.Cooldown,
			},
		},
		ApplyEffects: func(sim *core.Simulation, _ *core.Unit, _ *core.Spell) {
			druid.BarkskinAura.Activate(sim)
			druid.AutoAttacks.StopMeleeUntil(sim, sim.CurrentTime, false)
		},
	})

	druid.AddMajorCooldown(core.MajorCooldown{
		Spell: druid.Barkskin.Spell,
		Type:  core.CooldownTypeSurvival,
	})
}
