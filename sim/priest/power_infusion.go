package priest

import (
	"github.com/wowsims/classic/sim/core"
)

// Power Infusion is the thirty-one point Discipline talent and the reason the Smite build
// goes that deep, so the priest casts it on itself. The proto still carries a target option
// for the healing specs, which the sim has no way to act on yet.
// TODO: let the option pick a raid member once buffing another player is modelled.
func (priest *Priest) registerPowerInfusionCD() {
	if !priest.Talents.PowerInfusion {
		return
	}

	// Spell ID, cost and cooldown from the client table (see shadow_word_pain.go)
	row := spellData.PowerInfusion.ByRank(1)
	actionID := core.ActionID{SpellID: row.SpellID, Tag: priest.Index}
	powerInfusionAura := core.PowerInfusionAura(&priest.Unit, actionID.Tag)

	piSpell := priest.RegisterSpell(core.SpellConfig{
		ActionID: actionID,
		Flags:    SpellFlagPriest | core.SpellFlagHelpful | core.SpellFlagAPL,

		// 20% of base mana and a 3 min cooldown in the Forever beta client, as in Classic.
		ManaCost: core.ManaCostOptions{
			BaseCost: row.PowerCostPct / 100,
		},
		Cast: core.CastConfig{
			CD: core.Cooldown{
				Timer:    priest.NewTimer(),
				Duration: row.Cooldown,
			},
		},

		ApplyEffects: func(sim *core.Simulation, _ *core.Unit, _ *core.Spell) {
			powerInfusionAura.Activate(sim)
		},
	})

	priest.AddMajorCooldown(core.MajorCooldown{
		Spell:    piSpell,
		Priority: core.CooldownPriorityDefault,
		Type:     core.CooldownTypeDPS,
	})
}
