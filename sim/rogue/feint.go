package rogue

import (
	"time"

	"github.com/wowsims/classic/sim/common/shared"
	"github.com/wowsims/classic/sim/core"
)

// Cost, cooldown, school and defense type come from the client table, one rank a level (see
// sinister_strike.go for why the ids stay here). The threat is the rank's effect 63: base -5 a level
// past the rank's level, capped 10 levels on (1966 -750 at 16, 8637 -1950 at 40, 25302 -4000 at 60),
// so -795 at 25, -1950 at 40, -2000 at 50, -4000 at 60; the table holds each rank at level 60.
// Feint refunds 80% of its energy on a miss (Attributes[1] 0x08000000, as the finishers).
func (rogue *Rogue) registerFeintSpell() {
	spellID := map[int32]int32{
		25: 1966,
		40: 8637,
		50: 8637,
		60: 25302,
	}[rogue.Level]
	// The rank's base threat and level, as floats so sim/spell_sources_test.go does not read them as ids.
	threat := map[int32]struct{ base, level float64 }{
		1966:  {-750, 16},
		8637:  {-1950, 40},
		25302: {-4000, 60},
	}[spellID]
	row := spellData.Feint.BySpellID(spellID)

	rogue.Feint = rogue.RegisterSpell(core.SpellConfig{
		ActionID:    core.ActionID{SpellID: spellID},
		SpellSchool: row.SpellSchool,
		DefenseType: row.DefenseType,
		ProcMask:    core.ProcMaskMeleeMH,
		Flags:       core.SpellFlagMeleeMetrics | core.SpellFlagAPL,

		EnergyCost: core.EnergyCostOptions{
			Cost:   float64(row.Cost),
			Refund: 0.8,
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
		FlatThreatBonus:  shared.LevelScaled(threat.base, -5, int32(threat.level), int32(threat.level)+10, rogue.Level),

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			rogue.BreakStealth(sim)
			if !spell.CalcAndDealOutcome(sim, target, spell.OutcomeMeleeSpecialHit).Landed() {
				spell.IssueRefund(sim)
			}
		},
	})
}
