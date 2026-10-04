// Effects that make the player take damage, and the attack table such a hit rolls on. Each
// formula mirrors the cited Go file at the pinned revision.
package main

import (
	"fmt"

	"github.com/wowsims/forever/sim/core"
	"github.com/wowsims/forever/sim/core/proto"
	"github.com/wowsims/forever/sim/core/simsignals"
)

// Whether something can hit the player in scope: the Goblin Sapper Charge's half that goes
// off in the thrower's face, or the target's swings when the player tanks it.
func playerTakesDamage(character *core.Character, target *core.Unit) bool {
	return character.GetSpell(core.GoblinSapperActionID.WithTag(1)) != nil || target.CurrentTarget == &character.Unit
}

// The player's attack table against itself, which a spell that hits the player rolls on.
func selfAttackTable(character *core.Character, request *proto.RaidSimRequest, unrepresented *[]string) AttackTable {
	table := character.AttackTables[character.UnitIndex]
	if table.DamageDoneByCasterMultiplier != nil || len(table.DamageDoneByCasterExtraMultiplier) != 0 {
		*unrepresented = append(*unrepresented, "caster damage callbacks on the player are unsupported")
	}
	// Absorb shields register a damage taken modifier that acts only while their aura is up;
	// what activates the shields is a spell or listener the gate checks.
	if actingSelfDamageTakenModifiers(request) > 0 {
		*unrepresented = append(*unrepresented, "dynamic damage taken modifiers on the player are unsupported")
	}
	return AttackTable{BaseSpellMissChance: table.BaseSpellMissChance, SpellCritSuppression: table.SpellCritSuppression,
		BonusSpellCritPercent: table.BonusSpellCritPercent, CritMultiplier: table.CritMultiplier,
		DamageDealtMultiplier: table.DamageDealtMultiplier, DamageTakenMultiplier: table.DamageTakenMultiplier}
}

// How many of the player's dynamic damage taken modifiers change the sapper's hit on the
// player in a separate reset simulation.
func actingSelfDamageTakenModifiers(request *proto.RaidSimRequest) int {
	simulation := core.NewSim(request, simsignals.CreateSignals())
	simulation.Reset()
	player := simulation.Raid.Parties[0].Players[0].GetCharacter()
	spell := player.GetSpell(core.GoblinSapperActionID.WithTag(1))
	acting := 0
	for _, modifier := range player.DynamicDamageTakenModifiers {
		result := &core.SpellResult{Target: &player.Unit, Damage: 1000, Outcome: core.OutcomeHit}
		modifier(simulation, spell, result, false)
		if result.Damage != 1000 {
			acting++
		}
	}
	return acting
}

// consumes.go newBasicExplosiveSpellConfig for the Goblin Sapper Charge: a rolled Fire hit on
// every target, scaled by the AoE cap, dealt at once, then a second roll that hits the player.
func goblinSapperEffect(character *core.Character, request *proto.RaidSimRequest, unrepresented *[]string) map[string]any {
	self := character.GetSpell(core.GoblinSapperActionID.WithTag(1))
	if self == nil {
		*unrepresented = append(*unrepresented, fmt.Sprintf("Goblin Sapper Charge %s has no self damage spell", core.GoblinSapperActionID))
	}
	return map[string]any{
		"kind": "goblin_sapper", "item_id": core.GoblinSapperActionID.ItemID, "self_tag": int32(1),
		"min_damage": 450.0, "max_damage": 750.0,
		"aoe_cap_multiplier": character.Env.Encounter.AOECapMultiplier(),
		"self_attack_table":  selfAttackTable(character, request, unrepresented),
	}
}
