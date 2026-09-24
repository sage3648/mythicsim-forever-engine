package mage

import (
	"time"

	"github.com/wowsims/classic/sim/core"
	"github.com/wowsims/classic/sim/core/stats"
)

const (
	FireRuby              = 20036
	HazzarahsCharmOfMagic = 19959
	JewelOfKajaro         = 19601
)

func init() {
	core.AddEffectsToTest = false

	core.NewItemEffect(FireRuby, func(agent core.Agent) {
		character := agent.GetCharacter()

		actionID := core.ActionID{ItemID: FireRuby}
		manaMetrics := character.NewManaMetrics(actionID)

		damageAura := character.GetOrRegisterAura(core.Aura{
			Label:    "Chaos Fire",
			ActionID: core.ActionID{SpellID: 24389},
			Duration: time.Minute * 1,
			OnGain: func(aura *core.Aura, sim *core.Simulation) {
				character.AddStatDynamic(sim, stats.FirePower, 100)
			},
			OnExpire: func(aura *core.Aura, sim *core.Simulation) {
				character.AddStatDynamic(sim, stats.FirePower, -100)
			},
			OnCastComplete: func(aura *core.Aura, sim *core.Simulation, spell *core.Spell) {
				if spell.SpellSchool.Matches(core.SpellSchoolFire) {
					aura.Deactivate(sim)
				}
			},
		})

		spell := character.RegisterSpell(core.SpellConfig{
			ActionID:    actionID,
			SpellSchool: core.SpellSchoolPhysical,
			Flags:       core.SpellFlagNoOnCastComplete | core.SpellFlagOffensiveEquipment,

			Cast: core.CastConfig{
				CD: core.Cooldown{
					Timer:    character.NewTimer(),
					Duration: time.Minute * 3,
				},
			},

			ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
				character.AddMana(sim, sim.Roll(1, 500), manaMetrics)
				damageAura.Activate(sim)
			},
		})

		character.AddMajorCooldown(core.MajorCooldown{
			Type:  core.CooldownTypeDPS,
			Spell: spell,
		})
	})

	// https://www.wowhead.com/forever/item=19959/hazzarahs-charm-of-magic
	// Increases the critical hit chance of your Arcane spells by 5%, and increases the critical hit damage of your Arcane spells by 50% for 20 sec.
	// (3 Min Cooldown)
	//
	// Client 1.60.1.69977: both of Arcane Potency's (24544) effects carry class mask 2359296, which
	// names Arcane Explosion and Arcane Missiles only, whatever the tooltip says; Classic's took every
	// Arcane spell, Arcane Blast included.
	core.NewItemEffect(HazzarahsCharmOfMagic, func(agent core.Agent) {
		mage := agent.(MageAgent).GetMage()

		duration := time.Second * 20
		classMask := SpellMaskArcaneExplosion | SpellMaskArcaneMissiles | SpellMaskArcaneMissilesTick

		critMod := mage.AddDynamicMod(core.SpellModConfig{
			Kind:       core.SpellMod_BonusCrit_Percent,
			ClassMask:  classMask,
			FloatValue: 5,
		})
		critDamageMod := mage.AddDynamicMod(core.SpellModConfig{
			Kind:       core.SpellMod_CritMultiplier_Flat,
			ClassMask:  classMask,
			FloatValue: 0.50,
		})

		aura := mage.RegisterAura(core.Aura{
			ActionID: core.ActionID{SpellID: 24544},
			Label:    "Arcane Potency",
			Duration: duration,
			OnGain: func(aura *core.Aura, sim *core.Simulation) {
				critMod.Activate()
				critDamageMod.Activate()
			},
			OnExpire: func(aura *core.Aura, sim *core.Simulation) {
				critMod.Deactivate()
				critDamageMod.Deactivate()
			},
		})

		spell := mage.RegisterSpell(core.SpellConfig{
			ActionID:    core.ActionID{ItemID: HazzarahsCharmOfMagic},
			SpellSchool: core.SpellSchoolArcane,
			Flags:       core.SpellFlagNoOnCastComplete | core.SpellFlagOffensiveEquipment,
			Cast: core.CastConfig{
				CD: core.Cooldown{
					Timer:    mage.NewTimer(),
					Duration: time.Minute * 3,
				},
				SharedCD: core.Cooldown{
					Timer:    mage.GetOffensiveTrinketCD(),
					Duration: duration,
				},
			},
			ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
				aura.Activate(sim)
			},
		})

		mage.AddMajorCooldown(core.MajorCooldown{
			Spell:    spell,
			Priority: core.CooldownPriorityBloodlust,
			Type:     core.CooldownTypeDPS,
		})
	})

	// https://www.wowhead.com/classic/item=19601/jewel-of-kajaro
	// Equip: Reduces the cooldown of Counterspell by 2 sec.
	core.NewItemEffect(JewelOfKajaro, func(agent core.Agent) {
		mage := agent.(MageAgent).GetMage()

		mage.RegisterAura(core.Aura{
			Label: "Improved Counterspell",
			OnInit: func(aura *core.Aura, sim *core.Simulation) {
				mage.Counterspell.CD.Duration -= time.Second * 2
			},
		})
	})

	core.AddEffectsToTest = true
}
