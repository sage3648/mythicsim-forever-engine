package paladin

import (
	"time"

	"github.com/wowsims/classic/sim/core"
)

// Libram IDs
const (
	SanctifiedOrb  = 20512
	LibramOfHope   = 22401
	LibramOfFervor = 23203
)

func init() {
	// Sanctified Orb
	// Use: Restores 340 mana (24865, client 1.60.1.69977), doubled in Wasteland and Haunted areas,
	// which no encounter is. 5 min cooldown, shared with no other trinket. Classic's gave 3% crit
	// for 25 sec.
	core.NewItemEffect(SanctifiedOrb, func(agent core.Agent) {
		character := agent.GetCharacter()
		actionID := core.ActionID{ItemID: SanctifiedOrb}
		manaMetrics := character.NewManaMetrics(actionID)
		const manaGain = 340.0

		spell := character.RegisterSpell(core.SpellConfig{
			ActionID:    actionID,
			SpellSchool: core.SpellSchoolHoly,
			ProcMask:    core.ProcMaskEmpty,
			Flags:       core.SpellFlagNoOnCastComplete | core.SpellFlagHelpful,

			Cast: core.CastConfig{
				CD: core.Cooldown{
					Timer:    character.NewTimer(),
					Duration: time.Minute * 5,
				},
			},

			ApplyEffects: func(sim *core.Simulation, _ *core.Unit, _ *core.Spell) {
				character.AddMana(sim, manaGain, manaMetrics)
			},
		})

		character.AddMajorCooldown(core.MajorCooldown{
			Spell: spell,
			Type:  core.CooldownTypeMana,
			ShouldActivate: func(_ *core.Simulation, character *core.Character) bool {
				return character.MaxMana()-character.CurrentMana() >= manaGain
			},
		})
	})
}
