// Shared gear procs of common/forever: Battlegear of Valor's Warrior's Resolve and the Puncture
// Armor weapon procs. Each formula mirrors the cited Go file at the pinned revision.
package main

import (
	"fmt"

	"github.com/wowsims/forever/sim/core"
	"github.com/wowsims/forever/sim/core/proto"
	"github.com/wowsims/forever/sim/core/simsignals"
	"github.com/wowsims/forever/sim/core/stats"
)

func gearProcEffects(request *proto.RaidSimRequest, simulation *core.Simulation, character *core.Character, unrepresented *[]string) []map[string]any {
	effects := []map[string]any{}
	// item_sets_classic.go Battlegear of Valor 5 piece, Warrior's Resolve: landed melee hits roll a
	// legacy one proc a minute manager under the trigger's name; a batch window later the handler
	// heals Roll(88, 133) and, with a rage bar, gives 10 rage, both Go literals.
	if setAura := character.GetAura("Battlegear of Valor 5P"); setAura != nil {
		if setAura.Dpm == nil {
			*unrepresented = append(*unrepresented, "Battlegear of Valor 5P has no proc manager")
		} else {
			rage := 0.0
			if character.HasRageBar() {
				rage = 10
			}
			effects = append(effects, map[string]any{
				"kind": "health_rage_proc", "trigger_aura": setAura.Label, "rng_label": "Warrior's Resolve",
				"chances": dpmChances(character, setAura.Dpm, simulation, func(spell *core.Spell) bool {
					return spell.ProcMask.Matches(core.ProcMaskMelee) && !spell.Flags.Matches(core.SpellFlagProc)
				}),
				"heal_min": 88.0, "heal_max": 133.0, "rage": rage,
				"metrics_action_id": actionID(core.ActionID{SpellID: 450589}),
			})
		}
	}
	// classic items_trinkets.go Darkmoon Card: Heroism: landed melee hits roll a legacy two procs a
	// minute manager under the trigger's name; a batch window later the handler heals Roll(120, 180),
	// a Go literal, and gives no rage.
	if aura := character.GetAura("Darkmoon Card: Heroism"); aura != nil {
		if aura.Dpm == nil || aura.Icd != nil {
			*unrepresented = append(*unrepresented, "Darkmoon Card: Heroism has no proc manager")
		} else {
			effects = append(effects, map[string]any{
				"kind": "health_rage_proc", "trigger_aura": aura.Label, "rng_label": aura.Label,
				"chances": dpmChances(character, aura.Dpm, simulation, func(spell *core.Spell) bool {
					return spell.ProcMask.Matches(core.ProcMaskMelee) && !spell.Flags.Matches(core.SpellFlagProc)
				}),
				"heal_min": 120.0, "heal_max": 180.0, "rage": 0.0,
				"metrics_action_id": actionID(core.ActionID{SpellID: 23682}),
			})
		}
	}
	// items_weapons.go Bashguuder and Rivenspike: a weapon proc at two procs a minute of the
	// weapon's speed on landed hits; a batch window later the handler activates the target's
	// Puncture Armor and adds a stack, and each stack change moves the target's armor through
	// AddStatDynamic. The armor at each stack count is read from a separate reset simulation.
	// stat_bonus_procs_auto_gen.go Annihilator is the same through shared.NewSpellDataDebuffProc:
	// the trigger carries the item's name, the row's Armor Shatter takes 165 armor a stack, and the
	// handler applies it at once (TriggerImmediately) to the unit hit.
	for _, weapon := range []struct {
		trigger, debuff string
		immediate       bool
	}{{"Bashguuder Proc", "Puncture Armor", false}, {"Rivenspike Proc", "Puncture Armor", false},
		{"Annihilator", "Armor Shatter 16928", true}} {
		trigger := character.GetAura(weapon.trigger)
		if trigger == nil {
			continue
		}
		if trigger.Dpm == nil {
			*unrepresented = append(*unrepresented, fmt.Sprintf("%s has no proc manager", trigger.Label))
			continue
		}
		scratch := core.NewSim(request, simsignals.CreateSignals())
		scratch.Reset()
		target := &scratch.Encounter.AllTargets[0].Unit
		debuff := target.GetAura(weapon.debuff)
		if debuff == nil {
			*unrepresented = append(*unrepresented, fmt.Sprintf("%s has no %s aura", trigger.Label, weapon.debuff))
			continue
		}
		armor := []float64{0}
		base := target.GetStat(stats.Armor)
		debuff.Activate(scratch)
		for stacks := int32(1); stacks <= debuff.MaxStacks; stacks++ {
			debuff.SetStacks(scratch, stacks)
			armor = append(armor, target.GetStat(stats.Armor)-base)
		}
		effect := map[string]any{
			"kind": "armor_debuff_proc", "trigger_aura": trigger.Label, "rng_label": trigger.Label,
			"chances": dpmChances(character, trigger.Dpm, simulation, func(spell *core.Spell) bool {
				return !spell.Flags.Matches(core.SpellFlagSuppressWeaponProcs)
			}),
			"aura": debuff.Label, "armor_by_stacks": armor,
		}
		if weapon.immediate {
			effect["immediate"] = true
		}
		effects = append(effects, effect)
	}
	return effects
}
