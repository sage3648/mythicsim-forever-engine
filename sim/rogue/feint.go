package rogue

import (
	"time"

	"github.com/wowsims/classic/sim/core"
)

// Cost, cooldown, school and defense type come from the client table. The id stays ours: only rank 1
// is registered (see sinister_strike.go). The table's threat (-800 at rank 1) is not applied, as before.
func (rogue *Rogue) registerFeintSpell() {
	row := spellData.Feint.ByRank(1)

	rogue.Feint = rogue.RegisterSpell(core.SpellConfig{
		ActionID:    core.ActionID{SpellID: 1966},
		SpellSchool: row.SpellSchool,
		DefenseType: row.DefenseType,
		ProcMask:    core.ProcMaskMeleeMH,
		Flags:       core.SpellFlagMeleeMetrics | core.SpellFlagAPL,

		EnergyCost: core.EnergyCostOptions{
			Cost: float64(row.Cost),
		},
		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD: time.Second,
			},
			CD: core.Cooldown{
				Timer:    rogue.NewTimer(),
				Duration: row.Cooldown,
			},
			IgnoreHaste: true,
		},

		ThreatMultiplier: 1,

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			rogue.BreakStealth(sim)
			spell.CalcAndDealOutcome(sim, target, spell.OutcomeMeleeSpecialHit)
		},
	})
}
