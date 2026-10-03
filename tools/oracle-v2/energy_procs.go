// Item procs that restore energy. Each formula mirrors the cited Go file at the pinned
// revision.
package main

import (
	"github.com/wowsims/forever/sim/core"
)

// common/forever/item_sets_classic.go Shadowcraft Armor (5): a set bonus proc trigger on landed
// white hits at one proc a minute of each hand's speed, whose handler waits a spell batch
// window and restores 20 energy to a character with an energy bar.
func energyProcEffects(simulation *core.Simulation, character *core.Character, unrepresented *[]string) []map[string]any {
	effects := []map[string]any{}
	if aura := character.GetAura("Shadowcraft Armor 5P"); aura != nil {
		if aura.Dpm == nil {
			*unrepresented = append(*unrepresented, "Shadowcraft Armor's energize has no proc manager")
			return effects
		}
		effects = append(effects, map[string]any{
			"kind": "energize_proc", "trigger_aura": aura.Label, "rng_label": "Rogue Armor Energize",
			"chances": dpmChances(character, aura.Dpm, simulation, func(spell *core.Spell) bool {
				return !spell.Flags.Matches(core.SpellFlagProc) && spell.ProcMask.Matches(core.ProcMaskMeleeWhiteHit)
			}),
			"energy": 20.0, "metrics_action_id": actionID(core.ActionID{SpellID: 27787}),
			"delay_ns": nanos(core.SpellBatchWindow),
		})
	}
	return effects
}
