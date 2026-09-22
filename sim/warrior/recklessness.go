package warrior

import (
	"github.com/wowsims/classic/sim/core"
	"github.com/wowsims/classic/sim/core/stats"
)

func (warrior *Warrior) RegisterRecklessnessCD() {
	if warrior.Level < 50 {
		return
	}

	// Forever beta client 1.60.1.69893: id, cooldown and duration come from the client table.
	row := spellData.Recklessness.ByRank(1)
	actionID := core.ActionID{SpellID: row.SpellID}

	reckAura := warrior.RegisterAura(core.Aura{
		Label:    "Recklessness",
		ActionID: actionID,
		Duration: row.Duration,
		OnGain: func(aura *core.Aura, sim *core.Simulation) {
			warrior.PseudoStats.DamageTakenMultiplier *= 1.2
			warrior.AddStatDynamic(sim, stats.MeleeCrit, 100*core.CritRatingPerCritChance)
		},
		OnExpire: func(aura *core.Aura, sim *core.Simulation) {
			warrior.PseudoStats.DamageTakenMultiplier /= 1.2
			warrior.AddStatDynamic(sim, stats.MeleeCrit, -100*core.CritRatingPerCritChance)

		},
	})

	Recklessness := warrior.RegisterSpell(BerserkerStance, core.SpellConfig{
		ActionID: actionID,
		Cast: core.CastConfig{
			IgnoreHaste: true,
			DefaultCast: core.Cast{
				GCD: core.GCDDefault,
			},
			CD: core.Cooldown{
				Timer:    warrior.NewTimer(),
				Duration: row.Cooldown,
			},
		},

		ApplyEffects: func(sim *core.Simulation, _ *core.Unit, spell *core.Spell) {
			reckAura.Activate(sim)
		},
	})

	warrior.AddMajorCooldown(core.MajorCooldown{
		Spell: Recklessness.Spell,
		Type:  core.CooldownTypeDPS,
	})
}
