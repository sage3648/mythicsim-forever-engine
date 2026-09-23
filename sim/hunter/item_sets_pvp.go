package hunter

import (
	"github.com/wowsims/classic/sim/core"
	"github.com/wowsims/classic/sim/core/stats"
)

///////////////////////////////////////////////////////////////////////////
//                            Classic Phase 2
///////////////////////////////////////////////////////////////////////////

// https://www.wowhead.com/classic/item-set=361/champions-pursuit
var ItemSetChampionsPursuit = core.NewItemSet(core.ItemSet{
	Name: "Champion's Pursuit",
	Bonuses: map[int32]core.ApplyEffect{
		// +20 Agility (14384), where Classic gave 1% parry.
		2: func(agent core.Agent) {
			c := agent.GetCharacter()
			c.AddStat(stats.Agility, 20)
		},
		// Reduces the cooldown of your Concussive Shot by 1 sec.
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

// https://www.wowhead.com/classic/item-set=543/champions-pursuance
var ItemSetChampionsPursuance = core.NewItemSet(core.ItemSet{
	Name: "Champion's Pursuance",
	Bonuses: map[int32]core.ApplyEffect{
		// +20 Agility.
		2: func(agent core.Agent) {
			c := agent.GetCharacter()
			c.AddStat(stats.Agility, 20)
		},
		// Reduces the cooldown of your Concussive Shot by 1 sec.
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

// https://www.wowhead.com/classic/item-set=550/lieutenant-commanders-pursuance
var ItemSetLieutenantCommandersPursuance = core.NewItemSet(core.ItemSet{
	Name: "Lieutenant Commander's Pursuance",
	Bonuses: map[int32]core.ApplyEffect{
		// +20 Agility.
		2: func(agent core.Agent) {
			c := agent.GetCharacter()
			c.AddStat(stats.Agility, 20)
		},
		// Reduces the cooldown of your Concussive Shot by 1 sec.
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

///////////////////////////////////////////////////////////////////////////
//                            Classic Phase 3
///////////////////////////////////////////////////////////////////////////

