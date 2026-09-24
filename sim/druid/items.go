package druid

import (
	"github.com/wowsims/classic/sim/core"
)

// Item IDs
const (
	WolfsheadHelm   = 8345
	IdolOfFerocity  = 22397
	IdolOfTheMoon   = 23197
	IdolOfBrutality = 23198

	// 28855's rage cost cut on Maul, Swipe and Mangle.
	IdolOfBrutalityRageReduction = 2
)

func init() {
	core.AddEffectsToTest = false

	// https://www.wowhead.com/forever/item=22397/idol-of-ferocity
	// Equip: Reduces the energy cost of Claw and Rake by 2. Client 1.60.1.69977's 27851 takes 2 off
	// their shared class mask bit; Classic's took 3.
	core.NewItemEffect(IdolOfFerocity, func(agent core.Agent) {
		druid := agent.(DruidAgent).GetDruid()

		druid.OnSpellRegistered(func(spell *core.Spell) {
			if spell.SpellCode == SpellCode_DruidRake || spell.SpellCode == SpellCode_DruidClaw {
				spell.Cost.FlatModifier -= 2
			}
		})
	})

	// https://www.wowhead.com/classic/item=23197/idol-of-the-moon
	// Equip: Increases the damage of your Moonfire spell by up to 33.
	core.NewItemEffect(IdolOfTheMoon, func(agent core.Agent) {
		druid := agent.(DruidAgent).GetDruid()
		druid.OnSpellRegistered(func(spell *core.Spell) {
			if spell.SpellCode == SpellCode_DruidMoonfire {
				spell.BonusDamage += 33
			}
		})
	})

	// https://www.wowhead.com/forever/item=23198/idol-of-brutality
	// Equip: Reduces the rage cost of Maul and Swipe by 2. Client 1.60.1.69977's 28855 takes 20 tenths of
	// rage (2 Rage) off Maul, Swipe and Mangle; Classic's took 3 off Maul and Swipe.
	core.NewItemEffect(IdolOfBrutality, func(agent core.Agent) {
		// Implemented in maul.go, swipe.go and mangle.go (Bear Mangle, the Forever one).
	})

	core.AddEffectsToTest = true
}
