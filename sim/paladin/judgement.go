package paladin

import (
	"time"

	"github.com/wowsims/classic/sim/core"
)

func (paladin *Paladin) registerJudgement() {
	// Judgement functions as a dummy spell in vanilla.
	// It rolls on the spell hit table and can only miss or hit.
	// Individual seals have their own effects that this spell triggers,
	// that are handled in the implementations of the seal auras.
	// It is still a cast the paladin makes, and Sanctified Judgement and Swift Judgement both
	// listen for it through OnCastComplete, so it must not carry SpellFlagNoOnCastComplete.
	// Id, cost, cooldown and school come from the client table.
	row := spellData.Judgement.ByRank(1)
	paladin.judgement = paladin.RegisterSpell(core.SpellConfig{
		ClassSpellMask: SpellMaskJudgement,
		ActionID:       core.ActionID{SpellID: row.SpellID},
		SpellSchool:    row.SpellSchool,
		ProcMask:       core.ProcMaskEmpty,
		Flags:          core.SpellFlagMeleeMetrics | core.SpellFlagAPL | core.SpellFlagPassiveSpell | core.SpellFlagCastTimeNoGCD,

		ManaCost: core.ManaCostOptions{
			BaseCost:   row.PowerCostPct / 100,
			Multiplier: paladin.benediction(),
		},

		Cast: core.CastConfig{
			IgnoreHaste: true,
			CD: core.Cooldown{
				Timer:    paladin.NewTimer(),
				Duration: row.Cooldown - time.Second*time.Duration(paladin.Talents.ImprovedJudgement),
			},
		},
		ExtraCastCondition: func(_ *core.Simulation, _ *core.Unit) bool {
			return paladin.currentSeal.IsActive()
		},
		ApplyEffects: func(sim *core.Simulation, target *core.Unit, _ *core.Spell) {
			paladin.castSpecificJudgement(sim, target, paladin.currentJudgement)
		},
	})
}

// Helper Function For casting Judgement
//
// Forever's Judgement leaves the Seal up: 20271 reads "Unleash the energy of a Seal spell upon an
// enemy. Does not consume the Seal.", where Classic's is the same sentence without the second half.
// The seal aura still runs out on its own 30 sec.
func (paladin *Paladin) castSpecificJudgement(sim *core.Simulation, target *core.Unit, judgementSpell *core.Spell) {
	judgementSpell.Cast(sim, target)
}
