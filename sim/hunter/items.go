package hunter

import (
	"time"

	"github.com/wowsims/classic/sim/core"
)

const (
	KnightLieutenantsChainGauntlets = 16403
	BloodGuardsChainGauntlets       = 16530
	MarshalsChainGrips              = 16463
	GeneralsChainGloves             = 16571
	RenatakisCharmofBeasts          = 19953
	KnightLieutenantsChainVices     = 23279
	BloodGuardsChainVices           = 22862
)

// Devilsaur Eye (19991) and Devilsaur Tooth (19992) have no effect here: client 1.60.1.69977 reworks
// them into a root on Beasts (24352) and a cheaper Revive Pet (24353), neither of which the sim models.
func init() {
	// Equip: Reduces the mana cost of your Arcane Shot by 15.
	core.NewItemEffect(KnightLieutenantsChainGauntlets, func(agent core.Agent) {
		hunter := agent.(HunterAgent).GetHunter()
		core.MakePermanent(hunter.RegisterAura(core.Aura{
			Label: "Arcane Shot Mana Reduction",
			OnInit: func(aura *core.Aura, sim *core.Simulation) {
				if hunter.ArcaneShot != nil {
					hunter.ArcaneShot.Cost.FlatModifier -= 15.0
				}
			},
		}))
	})
	// Equip: Reduces the mana cost of your Arcane Shot by 15.
	core.NewItemEffect(BloodGuardsChainGauntlets, func(agent core.Agent) {
		hunter := agent.(HunterAgent).GetHunter()
		core.MakePermanent(hunter.RegisterAura(core.Aura{
			Label: "Arcane Shot Mana Reduction",
			OnInit: func(aura *core.Aura, sim *core.Simulation) {
				if hunter.ArcaneShot != nil {
					hunter.ArcaneShot.Cost.FlatModifier -= 15.0
				}
			},
		}))
	})

	// Equip: Increases the damage done by your Multi-Shot by 4%
	core.NewItemEffect(MarshalsChainGrips, func(agent core.Agent) {
		hunter := agent.(HunterAgent).GetHunter()
		core.MakePermanent(hunter.RegisterAura(core.Aura{
			Label: "Multi-Shot Damage Increase",
			OnInit: func(aura *core.Aura, sim *core.Simulation) {
				hunter.MultiShot.BaseDamageMultiplierAdditive += 0.04
			},
		}))
	})
	// Equip: Increases the damage done by your Multi-Shot by 4%
	core.NewItemEffect(GeneralsChainGloves, func(agent core.Agent) {
		hunter := agent.(HunterAgent).GetHunter()
		core.MakePermanent(hunter.RegisterAura(core.Aura{
			Label: "Multi-Shot Damage Increase",
			OnInit: func(aura *core.Aura, sim *core.Simulation) {
				hunter.MultiShot.BaseDamageMultiplierAdditive += 0.04
			},
		}))
	})
	// Equip: Increases the damage done by your Multi-Shot by 4%
	core.NewItemEffect(KnightLieutenantsChainVices, func(agent core.Agent) {
		hunter := agent.(HunterAgent).GetHunter()
		core.MakePermanent(hunter.RegisterAura(core.Aura{
			Label: "Multi-Shot Damage Increase",
			OnInit: func(aura *core.Aura, sim *core.Simulation) {
				hunter.MultiShot.BaseDamageMultiplierAdditive += 0.04
			},
		}))
	})
	// Equip: Increases the damage done by your Multi-Shot by 4%
	core.NewItemEffect(BloodGuardsChainVices, func(agent core.Agent) {
		hunter := agent.(HunterAgent).GetHunter()
		core.MakePermanent(hunter.RegisterAura(core.Aura{
			Label: "Multi-Shot Damage Increase",
			OnInit: func(aura *core.Aura, sim *core.Simulation) {
				hunter.MultiShot.BaseDamageMultiplierAdditive += 0.04
			},
		}))
	})
	// Use: Instantly clears the cooldowns of Aimed Shot, Multishot, Volley, and Arcane Shot. (cooldown 3 min)
	core.NewItemEffect(RenatakisCharmofBeasts, func(agent core.Agent) {
		hunter := agent.(HunterAgent).GetHunter()

		spell := hunter.RegisterSpell(core.SpellConfig{
			ActionID: core.ActionID{ItemID: RenatakisCharmofBeasts},
			ProcMask: core.ProcMaskEmpty,
			Flags:    core.SpellFlagNoOnCastComplete | core.SpellFlagOffensiveEquipment,

			Cast: core.CastConfig{
				CD: core.Cooldown{
					Timer:    hunter.NewTimer(),
					Duration: time.Second * 180,
				},
				SharedCD: core.Cooldown{
					Timer:    hunter.GetOffensiveTrinketCD(),
					Duration: time.Second * 10,
				},
			},

			ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
				hunter.AimedShot.CD.Reset()
				hunter.MultiShot.CD.Reset()
				// Volley has no cooldown in Forever, so there is nothing of it to clear.
				hunter.ArcaneShot.CD.Reset()
			},
		})

		hunter.AddMajorCooldown(core.MajorCooldown{
			Type:  core.CooldownTypeDPS,
			Spell: spell,
		})
	})
}
