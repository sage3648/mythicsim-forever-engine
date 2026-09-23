package shaman

import (
	"time"

	"github.com/wowsims/classic/sim/core"
)

// The shared buff values in sim/core/buffs.go are already the beta client's, 89 Agility (25360) and 53 Strength
// (25362), and Forever has no Enhancing Totems, so the shaman's own totems give them unscaled.

// The beta client puts every totem on a 1 sec global cooldown and gives the ones that last 1 or 2 min in Classic
// 5 min.
const totemGCD = time.Second
const totemDuration = time.Minute * 5

func (shaman *Shaman) newTotemSpellConfig(flatCost float64, spellID int32) core.SpellConfig {
	return core.SpellConfig{
		ActionID: core.ActionID{SpellID: spellID},
		Flags:    SpellFlagShaman | SpellFlagTotem | core.SpellFlagAPL,

		ManaCost: core.ManaCostOptions{
			FlatCost:   flatCost,
			Multiplier: shaman.totemManaMultiplier(),
		},

		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD: totemGCD,
			},
		},
	}
}
