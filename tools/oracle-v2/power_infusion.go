// Power Infusion: the generated aura buffs.PowerInfusionsAura builds, which the priest casts on
// itself (talents_discipline.go applyPowerInfusion) and any class receives from priests in the
// raid (buffs/drivers.go drivePowerInfusions, an external cooldown that external_cooldowns.go
// describes). Each copy is its own aura, labelled for the player or the external caster; both bid
// in the same exclusive categories.
package main

import (
	"github.com/wowsims/forever/sim/core"
	"github.com/wowsims/forever/sim/core/dbcenums"
	"github.com/wowsims/forever/sim/core/proto"
	"github.com/wowsims/forever/sim/core/simsignals"
	"github.com/wowsims/forever/sim/core/spelldata"
)

const powerInfusionSpell = 10060

// The aura with the id: it multiplies the damage of the schools its mask names and healing dealt.
// The multipliers come from client data; a separate reset simulation checks they are all the aura
// changes, and that no aura outside Power Infusion shares its categories. Both copies of the aura
// sit in each category, so a category holds one effect per copy.
func powerInfusionEffect(character *core.Character, request *proto.RaidSimRequest, id core.ActionID, note func(bool, string)) map[string]any {
	row := spelldata.MustFind(powerInfusionSpell)
	damage := 1 + row.Effect(dbcenums.A_MOD_DAMAGE_PERCENT_DONE, 126).Percent()
	healing := 1 + row.Effect(dbcenums.A_MOD_HEALING_DONE_PERCENT, 126).Percent()
	aura := character.GetAuraByID(id)
	if aura == nil {
		note(true, "Power Infusion has no aura")
		return nil
	}
	for _, ee := range aura.ExclusiveEffects {
		members := privateField(ee.Category, "effects")
		for i := 0; i < members.Len(); i++ {
			member := members.Index(i).Elem().FieldByName("Aura").Elem().FieldByName("ActionID").FieldByName("SpellID").Int()
			note(member != powerInfusionSpell, "Power Infusion shares its category")
		}
	}
	simulation := core.NewSim(request, simsignals.CreateSignals())
	simulation.Reset()
	player := simulation.Raid.Parties[0].Players[0].GetCharacter()
	before := player.PseudoStats
	beforeStats := player.GetStats()
	player.GetAuraByID(id).Activate(simulation)
	after := player.PseudoStats
	schools := []int{}
	for school := range before.SchoolDamageDealtMultiplier {
		if after.SchoolDamageDealtMultiplier[school] != before.SchoolDamageDealtMultiplier[school] {
			schools = append(schools, school)
			note(after.SchoolDamageDealtMultiplier[school] != before.SchoolDamageDealtMultiplier[school]*damage,
				"Power Infusion's school damage is not its client multiplier")
		}
	}
	note(after.HealingDealtMultiplier != before.HealingDealtMultiplier*healing,
		"Power Infusion's healing is not its client multiplier")
	after.SchoolDamageDealtMultiplier = before.SchoolDamageDealtMultiplier
	after.HealingDealtMultiplier = before.HealingDealtMultiplier
	note(after != before || player.GetStats() != beforeStats, "Power Infusion changes more than school damage and healing")
	return map[string]any{
		"kind": "power_infusion", "spell_id": powerInfusionSpell, "aura": aura.Label,
		"damage_multiplier": damage, "schools": schools, "healing_multiplier": healing,
	}
}
