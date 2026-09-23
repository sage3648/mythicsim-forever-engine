package priest

import (
	"github.com/wowsims/classic/sim/core"
	"github.com/wowsims/classic/sim/core/stats"
)

var ItemSetChampionsRaiment = core.NewItemSet(core.ItemSet{
	Name: "Champion's Raiment",
	Bonuses: map[int32]core.ApplyEffect{
		// Increases healing done by up to 44 and damage done by up to 15 for all magical spells and
		// effects (467550).
		2: func(agent core.Agent) {
			c := agent.GetCharacter()
			c.AddStats(stats.Stats{
				stats.HealingPower: 44,
				stats.SpellDamage:  15,
			})
		},
		// Increases the duration of your Psychic Scream spell by 1 sec.
		4: func(agent core.Agent) {
			// Nothing to do
		},
		// +20 Stamina (14467, was 15).
		6: func(agent core.Agent) {
			c := agent.GetCharacter()
			c.AddStat(stats.Stamina, 20)
		},
	},
})

var ItemSetChampionsInvestiture = core.NewItemSet(core.ItemSet{
	Name: "Champion's Investiture",
	Bonuses: map[int32]core.ApplyEffect{
		// Increases damage and healing done by magical spells and effects by up to 23.
		2: func(agent core.Agent) {
			c := agent.GetCharacter()
			c.AddStat(stats.SpellPower, 23)
		},
		// Increases the duration of your Psychic Scream spell by 1 sec.
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

