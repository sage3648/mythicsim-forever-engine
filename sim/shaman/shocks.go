package shaman

import (
	"time"

	"github.com/wowsims/classic/sim/common/shared"
	"github.com/wowsims/classic/sim/core"
)

// Shared logic for all shocks. Cost, the 6 sec cooldown, school, defense type and the direct hit's coefficient
// come from the client table row; the caller builds the action id from it, where spell_sources_test.go can
// read it.
func (shaman *Shaman) newShockSpellConfig(actionID core.ActionID, row shared.SpellData, shockTimer *core.Timer) core.SpellConfig {
	cdDuration := row.Cooldown - time.Millisecond*200*time.Duration(shaman.Talents.Reverberation)

	return core.SpellConfig{
		ActionID:    actionID,
		SpellSchool: row.SpellSchool,
		DefenseType: row.DefenseType,
		ProcMask:    core.ProcMaskSpellDamage,
		Flags:       SpellFlagShaman | core.SpellFlagAPL,

		ManaCost: core.ManaCostOptions{
			FlatCost:   float64(row.Cost),
			Multiplier: 100 - 2*shaman.Talents.Convection - shaman.shamanisticFocusReduction(),
		},
		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD: core.GCDDefault,
			},
			CD: core.Cooldown{
				Timer:    shaman.NewTimer(),
				Duration: cdDuration,
			},
			SharedCD: core.Cooldown{
				Timer:    shockTimer,
				Duration: cdDuration,
			},
		},

		DamageMultiplier: 1,
		ThreatMultiplier: 1,
		BonusCoefficient: roundCoef(row.Direct.BonusCoefficient()),
	}
}

func (shaman *Shaman) registerShocks() {
	shockTimer := shaman.NewTimer()
	shaman.registerEarthShockSpell(shockTimer)
	shaman.registerFlameShockSpell(shockTimer)
	shaman.registerFrostShockSpell(shockTimer)
}
