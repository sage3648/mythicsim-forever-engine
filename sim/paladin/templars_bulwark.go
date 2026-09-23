package paladin

import (
	"time"

	"github.com/wowsims/classic/sim/core"
)

// Templar's Bulwark is new in Forever and borrows Sacred Shield's spell id, the paladin absorb
// the tooltip describes; the beta client's own is 1311015, which confirms 110 mana, the 5 min
// cooldown, 8 sec and an absorb of 100% of maximum health.
// The 5 minute cooldown the sim assumed is confirmed: the BlizzCon "Paladin Class Change"
// talent slide reads "110 Mana, Instant, 5 min cooldown", and the same tooltip appears in
// Joardee's VOD. It applies Forbearance for 1 min, which is what sim/paladin/forbearance.go
// already grants.
func (paladin *Paladin) registerTemplarsBulwark() {
	if !paladin.Talents.TemplarsBulwark {
		return
	}

	actionID := core.ActionID{SpellID: 53601}

	// Cost, cooldown and duration come from the client table (1311015); the id stays ours.
	row := spellData.TemplarsBulwark.ByRank(1)

	// The sim has no absorb model. A shield worth the paladin's whole health pool is far more
	// than a tank takes in 8 seconds, so it is modelled as damage taken dropping to nothing.
	const damageTaken = 0.01

	bulwarkAura := paladin.RegisterAura(core.Aura{
		Label:    "Templar's Bulwark",
		ActionID: actionID,
		Duration: row.Duration,
		OnGain: func(aura *core.Aura, sim *core.Simulation) {
			paladin.PseudoStats.DamageTakenMultiplier *= damageTaken
		},
		OnExpire: func(aura *core.Aura, sim *core.Simulation) {
			paladin.PseudoStats.DamageTakenMultiplier /= damageTaken
		},
	})

	// Sacred Duty: 30 sec a rank, confirmed by the beta client's talent data.
	cooldown := row.Cooldown - time.Second*30*time.Duration(paladin.Talents.SacredDuty)

	bulwark := paladin.RegisterSpell(core.SpellConfig{
		ActionID: actionID,
		Flags:    core.SpellFlagAPL | SpellFlag_Forbearance,

		ManaCost: core.ManaCostOptions{
			FlatCost:   float64(row.Cost),
			Multiplier: paladin.benediction(),
		},

		// Off the global cooldown: the client row (1311015) has no start recovery time.
		Cast: core.CastConfig{
			IgnoreHaste: true,
			CD: core.Cooldown{
				Timer:    paladin.NewTimer(),
				Duration: cooldown,
			},
		},

		ApplyEffects: func(sim *core.Simulation, _ *core.Unit, _ *core.Spell) {
			bulwarkAura.Activate(sim)
		},
	})

	paladin.AddMajorCooldown(core.MajorCooldown{
		Spell: bulwark,
		Type:  core.CooldownTypeSurvival,
	})
}
