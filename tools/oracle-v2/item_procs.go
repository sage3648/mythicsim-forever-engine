// Item and enchant procs and on-use items the shared item code registers from client rows,
// beyond the weapon procs in melee_procs.go. Each mirrors the cited Go file at the pinned
// revision.
package main

import (
	"fmt"

	"github.com/wowsims/forever/sim/core"
	"github.com/wowsims/forever/sim/core/proto"
	"github.com/wowsims/forever/sim/core/simsignals"
	"github.com/wowsims/forever/sim/core/spelldata"
	"github.com/wowsims/forever/sim/core/stats"
)

// Procs common/forever/stat_bonus_procs_auto_gen.go and enchants_auto_gen.go register with
// shared.NewSpellDataProc: the trigger's name, its trigger and buff rows, and the item or enchant
// whose effect entry states the buff's stats; or with shared.NewSpellDataAuraProc, whose buff is
// the row's own parsed aura on the wearer.
var spellDataStatProcs = []struct {
	label   string
	trigger int32
	buff    int32
	item    int32
	enchant int32
	parsed  bool
}{
	{"Draconic Infused Emblem", 1318931, 1318930, 22268, 0, false},
	{"The Green Tower", 18097, 17154, 1204, 0, false},
	{"Wall of the Dead", 19409, 18828, 1979, 0, false},
	{"Darkmoon Card: Blue Dragon", 23688, 23684, 19288, 0, false},
	{"Wrath of Cenarius", 25906, 25907, 21190, 0, false},
	{"Painwalker Buckler", 1293701, 1293700, 274290, 0, false},
	{"Enchant Weapon - Grand Sorcerer", 1231163, 1231162, 0, 7942, false},
	{"Enchant 2H Weapon - Grand Arcanist", 1231152, 1231138, 0, 7941, false},
	{"Enchant Weapon - Insight", 1248758, 1299796, 0, 8216, true},
}

// shared_utils.go applySpellDataProc names the buff it builds after the trigger.
func spellDataStatProcAura(label string) string {
	return label + " Proc"
}

// The stat auras of the spell data procs the character wears, which statAurasEffect combines. A
// proc of the hits the wearer takes joins them only when the wearer tanks the target, since nothing
// else hits it.
func spellDataStatProcAuras(character *core.Character) []string {
	labels := []string{}
	for _, proc := range spellDataStatProcs {
		if character.GetAura(proc.label) == nil {
			continue
		}
		trigger := spelldata.MustFind(proc.trigger)
		listener := spelldata.ProcTrigger(character, trigger, nil, spelldata.ItemProcChance(trigger))
		if listener.Callback == core.CallbackOnSpellHitTaken && !tanksTheTarget(character) {
			continue
		}
		labels = append(labels, spellDataStatProcAura(proc.label))
	}
	return labels
}

// Whether the target swings at the wearer.
func tanksTheTarget(character *core.Character) bool {
	return character.Env.Encounter.ActiveTargetUnits[0].CurrentTarget == &character.Unit
}

// shared_utils.go applySpellDataProc: the listener its trigger row decodes to (spellDataProcListener)
// rolls the stated chance under the trigger's name, behind the trigger aura's cooldown, and a spell
// batch window later activates a temporary stats aura of the effect entry's stats
// (spellDataProcAura, NewTemporaryStatsAuraWrapped). spell_data_aura.go registerSpellDataAuraProc
// activates the row's parsed aura instead, which logs nothing; only one on the wearer alone is
// described. Only a proc without a rate, stacks or charges, hearing landed or any hits, casts and
// heals, is described.
func spellDataStatProcEffects(character *core.Character, unrepresented *[]string) []map[string]any {
	effects := []map[string]any{}
	for _, proc := range spellDataStatProcs {
		triggerAura := character.GetAura(proc.label)
		if triggerAura == nil {
			continue
		}
		procAura := character.GetAura(spellDataStatProcAura(proc.label))
		trigger := spelldata.MustFind(proc.trigger)
		buff := spelldata.MustFind(proc.buff)
		var entries []*proto.ItemEffect
		if proc.item != 0 {
			entries = core.GetItemByID(proc.item).ItemEffects
		} else {
			entries = core.GetEnchantByEffectID(proc.enchant).EnchantEffects
		}
		var entry *proto.ItemEffect
		for _, candidate := range entries {
			if candidate.BuffId == proc.buff {
				entry = candidate
			}
		}
		if proc.parsed {
			// The parsed aura needs no effect entry; one on a pet or an enemy is not described.
			entry = &proto.ItemEffect{}
			if len(spelldata.EffectsOn(buff, spelldata.AuraOnPet)) > 0 || len(spelldata.EffectsOn(buff, spelldata.AuraOnEnemy)) > 0 {
				entry = nil
			}
		}
		listener := spelldata.ProcTrigger(character, trigger, nil, spelldata.ItemProcChance(trigger))
		names := callbackNames(listener.Callback)
		struck := hearsTheTargetsSwings(character, listener)
		heard := len(names) > 0
		for _, name := range names {
			heard = heard && (name == "on_spell_hit_dealt" || name == "on_heal_dealt" || name == "on_cast_complete")
		}
		heard = heard || struck
		if struck && !tanksTheTarget(character) {
			effects = append(effects, map[string]any{
				"kind": "inert_listener", "unit": "player", "aura": triggerAura.Label, "reason": "hears only melee hits the player takes",
			})
			continue
		}
		if procAura == nil || entry == nil || entry.GetStackingAura() != nil || triggerAura.Dpm != nil || trigger.RPPM != 0 ||
			entry.GetProc().GetPpm() != 0 || max(buff.MaxStack, trigger.MaxStack) > 0 || buff.ProcCharges > 0 || !heard ||
			listener.ExtraCondition != nil || listener.IsWeaponProc ||
			(listener.Outcome != core.OutcomeEmpty && listener.Outcome != core.OutcomeLanded) {
			*unrepresented = append(*unrepresented, fmt.Sprintf("%s is not a chance stat proc on hits, heals or casts", proc.label))
			continue
		}
		chance := listener.ProcChance
		if chance == 0 {
			chance = 1
		}
		effect := map[string]any{
			"kind": "spell_data_stat_proc", "trigger_aura": triggerAura.Label, "aura": procAura.Label,
			"trigger_spells": procTriggerSpellsOrNone(character, listener, struck), "callbacks": names,
			"landed_only": listener.Outcome == core.OutcomeLanded, "require_damage": listener.RequireDamageDealt,
			"proc_chance": chance,
		}
		if struck {
			effect["struck"] = true
		}
		if !proc.parsed {
			bonus := stats.FromProtoMap(entry.GetScalingOptions()[int32(0)].GetStats())
			effect["gain_log"] = fmt.Sprintf("Gained %s from %s.", bonus.FlatString(), procAura.ActionID)
			effect["expire_log"] = fmt.Sprintf("Lost %s from fading %s.", bonus.FlatString(), procAura.ActionID)
		}
		effects = append(effects, effect)
	}
	return effects
}

// common/classic/items_trinkets.go Burst of Knowledge (11832): its use activates an aura whose
// spell mod lowers the mana cost of spells by a flat amount. The changes are read from a separate
// reset simulation with the aura active, by spellbook position.
func spellCostAuraOnUseEffect(request *proto.RaidSimRequest, character *core.Character, spell *core.Spell, label string) map[string]any {
	if character.GetAura(label) == nil {
		return nil
	}
	simulation := core.NewSim(request, simsignals.CreateSignals())
	simulation.Reset()
	player := simulation.Raid.Parties[0].Players[0].GetCharacter()
	before := map[int]int32{}
	for i, registered := range player.Spellbook {
		if registered.Cost != nil {
			before[i] = registered.Cost.FlatModifier
		}
	}
	player.GetAura(label).Activate(simulation)
	changes := []map[string]any{}
	for i, registered := range player.Spellbook {
		if registered.Cost != nil && registered.Cost.FlatModifier != before[i] {
			changes = append(changes, map[string]any{"spell": i, "flat": registered.Cost.FlatModifier - before[i]})
		}
	}
	return map[string]any{"kind": "spell_cost_aura_on_use", "item_id": spell.ActionID.ItemID, "aura": label, "cost_changes": changes}
}

// The spells a listener hears, none for a listener of the hits the wearer takes: the target's swings
// carry no spellbook position.
func procTriggerSpellsOrNone(character *core.Character, listener core.ProcTrigger, struck bool) []int {
	if struck {
		return []int{}
	}
	return procTriggerSpells(character, listener)
}
