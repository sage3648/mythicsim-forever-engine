// Power Infusion: the generated aura buffs.PowerInfusionsAura builds, which the priest casts on
// itself (talents_discipline.go applyPowerInfusion) and any class receives from priests in the
// raid (buffs/drivers.go drivePowerInfusions, an external cooldown). Each copy is its own aura,
// labelled for the player or the external caster; both bid in the same exclusive categories.
package main

import (
	"fmt"

	"github.com/wowsims/forever/sim/core"
	"github.com/wowsims/forever/sim/core/buffs"
	"github.com/wowsims/forever/sim/core/dbcenums"
	"github.com/wowsims/forever/sim/core/proto"
	"github.com/wowsims/forever/sim/core/simsignals"
	"github.com/wowsims/forever/sim/core/spelldata"
)

const powerInfusionSpell = 10060

// The label buffs.PowerInfusionsAura gives the external caster's copy.
const externalPowerInfusionLabel = "Power Infusions (External)"

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

// core/buffs.go registerExternalConsecutiveCDApproximation through buffs/drivers.go
// drivePowerInfusions: priests in the raid cast Power Infusion on the player on cooldown, taking
// turns. The cast is a simple spell on the player's own timer with no cost, metrics or log, which
// the major cooldown manager uses as a DPS cooldown; each source has a timer of its own that the
// cast sets to the buff's cooldown, and the player's timer waits for the next source. The aura is
// the generated external copy. The runtime keeps the pending source across fights, as Go's closure
// does, and rolls the healing multiplier back by division, which is exact only when it is 1.
func externalPowerInfusionEffects(character *core.Character, request *proto.RaidSimRequest, unrepresented *[]string) []map[string]any {
	aura := character.GetAura(externalPowerInfusionLabel)
	if aura == nil {
		return nil
	}
	note := func(condition bool, message string) {
		if condition {
			*unrepresented = append(*unrepresented, message)
		}
	}
	sources := request.Raid.Parties[0].Players[0].Buffs.PowerInfusions
	var cast *core.Spell
	for _, spell := range character.Spellbook {
		if spell.RelatedSelfBuff == aura && spell.ActionID == aura.ActionID {
			cast = spell
		}
	}
	if cast == nil || cast.CD.Timer == nil || sources <= 0 {
		note(true, "the external Power Infusion has no cooldown spell")
		return nil
	}
	note(cast.CD.Duration != aura.Duration, "the external Power Infusion's cooldown is not its aura's duration")
	note(character.PseudoStats.HealingDealtMultiplier != 1, fmt.Sprintf("the external Power Infusion on a healing dealt multiplier of %v", character.PseudoStats.HealingDealtMultiplier))
	effects := []map[string]any{{
		"kind": "external_cooldown", "spell_id": cast.ActionID.SpellID, "spell_tag": cast.ActionID.Tag,
		"aura": aura.Label, "aura_tag": aura.Tag, "sources": sources,
		"cooldown_ns": nanos(buffs.PowerInfusionsCooldown()), "duration_ns": nanos(aura.Duration),
	}}
	if effect := powerInfusionEffect(character, request, aura.ActionID, note); effect != nil {
		effects = append(effects, effect)
	}
	return effects
}
