package warlock

import (
	"time"

	"github.com/wowsims/classic/sim/core"
)

const (
	HazzarahsCharmOfDestruction = 19957
)

func init() {
	// https://www.wowhead.com/forever/item=19957/hazzarahs-charm-of-destruction
	// Use: Increases the critical hit chance of your Destruction spells by 10% for 20 sec. (3 Min Cooldown)
	//
	// The buff, Massive Destruction (24543), is a crit modifier over a class mask. Of what the sim
	// casts it names Shadow Bolt, Immolate, Searing Pain, Soul Fire, Shadowburn, Conflagrate and Rain
	// of Fire, not Incinerate, which the Destruction flag took in.
	core.NewItemEffect(HazzarahsCharmOfDestruction, func(agent core.Agent) {
		warlock := agent.(WarlockAgent).GetWarlock()

		actionID := core.ActionID{ItemID: HazzarahsCharmOfDestruction}
		duration := time.Second * 20

		bonusCrit := warlock.AddDynamicMod(core.SpellModConfig{
			Kind: core.SpellMod_BonusCrit_Percent,
			ClassMask: SpellMaskShadowBolt | SpellMaskImmolate | SpellMaskSearingPain | SpellMaskSoulFire |
				SpellMaskShadowburn | SpellMaskConflagrate | SpellMaskRainOfFire,
			FloatValue: 10,
		})

		buffAura := warlock.RegisterAura(core.Aura{
			ActionID: actionID,
			Label:    "Massive Destruction",
			Duration: duration,
			OnGain: func(aura *core.Aura, sim *core.Simulation) {
				bonusCrit.Activate()
			},
			OnExpire: func(aura *core.Aura, sim *core.Simulation) {
				bonusCrit.Deactivate()
			},
		})

		spell := warlock.RegisterSpell(core.SpellConfig{
			ActionID:    actionID,
			SpellSchool: core.SpellSchoolFire,
			Flags:       core.SpellFlagNoOnCastComplete | core.SpellFlagOffensiveEquipment,
			Cast: core.CastConfig{
				CD: core.Cooldown{
					Timer:    warlock.NewTimer(),
					Duration: time.Minute * 3,
				},
				SharedCD: core.Cooldown{
					Timer:    warlock.GetOffensiveTrinketCD(),
					Duration: duration,
				},
			},
			ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
				buffAura.Activate(sim)
			},
		})

		warlock.AddMajorCooldown(core.MajorCooldown{
			Spell:    spell,
			Priority: core.CooldownPriorityBloodlust,
			Type:     core.CooldownTypeDPS,
		})
	})
}
