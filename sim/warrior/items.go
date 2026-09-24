package warrior

import (
	"time"

	"github.com/wowsims/classic/sim/core"
	"github.com/wowsims/classic/sim/core/stats"
)

const (
	KnightLieutenantsPlateGauntlets = 16406
	MarshalsPlateGauntlets          = 16484
	GeneralsPlateGauntlets          = 16548
	RageOfMugamba                   = 19577
	GrileksCharmOfMight             = 19951
	DiamondFlask                    = 20130
)

func init() {
	core.AddEffectsToTest = false

	// Diamond Flask
	// Forever's use is a 5 sec channel (363881) that heals and, if it runs to the end, grants 20
	// Strength for 60 sec (1318070), client 1.60.1.69977; Classic's gave 75 Strength outright
	// (24427). 6 min cooldown, 1 min on its own consumable category rather than the burst trinket
	// one. The heal is left out, and five seconds of channel belong before the pull, so it is left
	// to the APL rather than auto-used.
	core.NewItemEffect(DiamondFlask, func(agent core.Agent) {
		character := agent.GetCharacter()
		strengthAura := character.NewTemporaryStatsAura("Diamond Flask", core.ActionID{SpellID: 1318070}, stats.Stats{stats.Strength: 20}, time.Minute)

		spell := character.RegisterSpell(core.SpellConfig{
			ActionID: core.ActionID{ItemID: DiamondFlask},
			ProcMask: core.ProcMaskEmpty,
			Flags:    core.SpellFlagNoOnCastComplete | core.SpellFlagChanneled | core.SpellFlagHelpful,

			Cast: core.CastConfig{
				CD: core.Cooldown{
					Timer:    character.NewTimer(),
					Duration: time.Minute * 6,
				},
			},

			Hot: core.DotConfig{
				SelfOnly: true,
				Aura: core.Aura{
					Label: "CHUG! CHUG! CHUG! CHUG!",
				},
				NumberOfTicks: 5,
				TickLength:    time.Second,
				OnTick: func(sim *core.Simulation, _ *core.Unit, dot *core.Dot) {
					if dot.MaxTicksRemaining() == 0 {
						strengthAura.Activate(sim)
					}
				},
			},

			ApplyEffects: func(sim *core.Simulation, _ *core.Unit, spell *core.Spell) {
				spell.SelfHot().Apply(sim)
			},
		})

		character.AddMajorCooldown(core.MajorCooldown{
			Spell: spell,
			Type:  core.CooldownTypeDPS,
			ShouldActivate: func(_ *core.Simulation, _ *core.Character) bool {
				return false // Five seconds of channel belong before the pull; left to the APL.
			},
		})
	})

	core.NewItemEffect(GeneralsPlateGauntlets, func(agent core.Agent) {
		warrior := agent.(WarriorAgent).GetWarrior()

		warrior.RegisterAura(core.Aura{
			Label: "Hamstring Rage Reduction",
			OnInit: func(aura *core.Aura, sim *core.Simulation) {
				warrior.Hamstring.Cost.FlatModifier -= 3
			},
		})
	})

	core.NewItemEffect(GrileksCharmOfMight, func(agent core.Agent) {
		warrior := agent.(WarriorAgent).GetWarrior()
		actionID := core.ActionID{ItemID: GrileksCharmOfMight}
		rageMetrics := warrior.NewRageMetrics(actionID)

		spell := warrior.Character.RegisterSpell(core.SpellConfig{
			ActionID:    actionID,
			SpellSchool: core.SpellSchoolPhysical,
			ProcMask:    core.ProcMaskEmpty,
			Flags:       core.SpellFlagNoOnCastComplete | core.SpellFlagOffensiveEquipment,

			Cast: core.CastConfig{
				CD: core.Cooldown{
					Timer:    warrior.NewTimer(),
					Duration: time.Minute * 3,
				},
			},

			ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
				warrior.AddRage(sim, 30, rageMetrics)
			},
		})

		warrior.AddMajorCooldown(core.MajorCooldown{
			Type:  core.CooldownTypeDPS,
			Spell: spell,
		})
	})

	core.NewItemEffect(RageOfMugamba, func(agent core.Agent) {
		warrior := agent.(WarriorAgent).GetWarrior()

		warrior.RegisterAura(core.Aura{
			Label: "Reduces the cost of your Hamstring ability by 2 rage points.",
			OnInit: func(aura *core.Aura, sim *core.Simulation) {
				warrior.Hamstring.Cost.FlatModifier -= 2
			},
		})
	})

	// Knight-Lieutenant's Plate Gauntlets: Hamstring costs 3 less rage (22778, client 1.60.1.69977),
	// as on Marshal's and General's.
	core.NewItemEffect(KnightLieutenantsPlateGauntlets, func(agent core.Agent) {
		warrior := agent.(WarriorAgent).GetWarrior()

		warrior.RegisterAura(core.Aura{
			Label: "Hamstring Rage Reduction",
			OnInit: func(aura *core.Aura, sim *core.Simulation) {
				warrior.Hamstring.Cost.FlatModifier -= 3
			},
		})
	})

	core.NewItemEffect(MarshalsPlateGauntlets, func(agent core.Agent) {
		warrior := agent.(WarriorAgent).GetWarrior()

		warrior.RegisterAura(core.Aura{
			Label: "Hamstring Rage Reduction",
			OnInit: func(aura *core.Aura, sim *core.Simulation) {
				warrior.Hamstring.Cost.FlatModifier -= 3
			},
		})
	})

	core.AddEffectsToTest = true
}
