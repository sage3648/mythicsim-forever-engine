package paladin

import (
	"github.com/wowsims/classic/sim/core"
	"github.com/wowsims/classic/sim/core/stats"
)

var ItemSetLieutenantCommandersRedoubt = core.NewItemSet(core.ItemSet{
	Name: "Lieutenant Commander's Redoubt",
	Bonuses: map[int32]core.ApplyEffect{
		// Increases damage and healing done by magical spells and effects by up to 23.
		2: func(agent core.Agent) {
			c := agent.GetCharacter()
			c.AddStat(stats.SpellPower, 23)
		},
		// Reduces the cooldown of your Hammer of Justice by 10 sec.
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

