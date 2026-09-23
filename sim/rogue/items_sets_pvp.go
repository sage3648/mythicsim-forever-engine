package rogue

import (
	"github.com/wowsims/classic/sim/core"
	"github.com/wowsims/classic/sim/core/stats"
)

///////////////////////////////////////////////////////////////////////////
//                          Classic Phase 2
///////////////////////////////////////////////////////////////////////////

var ItemSetLieutenantCommandersGuard = core.NewItemSet(core.ItemSet{
	Name: "Lieutenant Commander's Guard",
	Bonuses: map[int32]core.ApplyEffect{
		// +40 Attack Power.
		2: func(agent core.Agent) {
			c := agent.GetCharacter()
			c.AddStats(stats.Stats{
				stats.AttackPower:       40,
				stats.RangedAttackPower: 40,
			})
		},
		// Reduces the cooldown of your Gouge ability by 1 sec.
		4: func(agent core.Agent) {
			// Nothing to do
		},
		// +20 Stamina.
		6: func(agent core.Agent) {
			c := agent.GetCharacter()
			c.AddStat(stats.Stamina, 20)
		},
	},
})

