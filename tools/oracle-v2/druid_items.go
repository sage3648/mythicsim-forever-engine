// Druid item set export: Feralheart Raiment's four piece Nature's Bounty, Wildheart Raiment's
// five piece one and Symbols of Unending Life's three piece finisher refund, as item_sets.go
// attaches them.
package main

import (
	"github.com/wowsims/forever/sim/core"
	"github.com/wowsims/forever/sim/druid"
)

func druidItemEffects(character *core.Character) []map[string]any {
	effects := []map[string]any{}
	if aura := character.GetAura("Feralheart Raiment 4P"); aura != nil {
		mana := core.ProcTrigger{ProcMask: core.ProcMaskSpellDamage | core.ProcMaskSpellHealing}
		energy := core.ProcTrigger{ProcMask: core.ProcMaskMeleeWhiteHit}
		effects = append(effects, map[string]any{
			"kind": "natures_bounty", "aura": aura.Label, "proc_chance": 0.02,
			"mana_label": "Nature's Bounty (Mana)", "mana": 200.0, "mana_spells": procTriggerSpells(character, mana),
			"energy_label": "Nature's Bounty (Energy)", "energy": 20.0, "energy_spells": procTriggerSpells(character, energy),
			"rage_label": "Nature's Bounty (Rage)", "rage": 10.0,
			"metrics_action_id": actionID(core.ActionID{SpellID: 450608}),
		})
	}
	// Wildheart Raiment's Nature's Bounty hears only melee hits the player takes. The Goblin
	// Sapper Charge's hit on the player is a spell, and a tank's are rejected with the target's
	// swings.
	if aura := character.GetAura("Wildheart Raiment 5P"); aura != nil {
		effects = append(effects, map[string]any{"kind": "inert_listener", "unit": "player", "aura": aura.Label,
			"reason": "acts only on melee hits the player takes"})
	}
	if aura := character.GetAura("Symbols of Unending Life 3P"); aura != nil {
		trigger := core.ProcTrigger{ClassSpellMask: druid.DruidSpellFerociousBite | druid.DruidSpellRip}
		effects = append(effects, map[string]any{
			"kind": "unending_life_refund", "aura": aura.Label, "energy": 30.0,
			"spells": procTriggerSpells(character, trigger), "label": "Symbols of Unending Life Finisher Bonus",
			"metrics_action_id": actionID(core.ActionID{SpellID: 26107}),
		})
	}
	return effects
}
