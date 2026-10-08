// The buffs other players cast on the player on cooldown: core/buffs.go
// registerExternalConsecutiveCDApproximation through buffs/drivers.go, which wraps a generated
// aura in NewGeneratedExternalCD. Priests in the raid cast Power Infusion (drivePowerInfusions),
// druids cast Innervate (driveInnervates) and shamans drop Mana Tide Totem (driveManaTideTotems),
// each with a number of sources taking turns. The cast is a simple spell on the player's own timer
// with no cost, metrics or log, which the major cooldown manager uses; each source has a timer of
// its own that the cast sets to the buff's cooldown, and the player's timer waits for the next
// source. What differs between the buffs is when the manager may cast (ShouldActivate) and what
// the aura does: Power Infusion's multipliers (power_infusion.go), Innervate's spirit regen
// (AttachInnervateRegen) and Mana Tide Totem's mana per 5 seconds, a stat the stat auras carry.
// The runtime keeps the pending source across fights, as Go's closure does.
package main

import (
	"fmt"
	"time"

	"github.com/wowsims/forever/sim/core"
	"github.com/wowsims/forever/sim/core/buffs"
	"github.com/wowsims/forever/sim/core/proto"
	"github.com/wowsims/forever/sim/core/simsignals"
	"github.com/wowsims/forever/sim/core/stats"
)

// The label buffs.ManaTideTotemsAura gives the external caster's copy.
const externalManaTideLabel = "Mana Tide Totem (External)"

// buffs/drivers.go innervateSpiritRegenMultiplier and innervateRegenTag: Go literals.
const (
	innervateSpiritRegenMultiplier = 5.0
	innervateRegenTag              = -2
)

// buffs/drivers.go driveManaTideTotems: the totem waits until the party has mana to refill, which
// is 40 seconds in, or halfway through a fight shorter than that.
const manaTideInitialDelayCap = 40 * time.Second

// One generated external cooldown: the aura's label, how many sources the request assigns, the
// buff's cooldown, when the manager may cast it and the effects its aura adds.
type externalCooldown struct {
	label      string
	sources    int32
	cooldown   time.Duration
	activation func(character *core.Character) map[string]any
	aura       func(character *core.Character, request *proto.RaidSimRequest, aura *core.Aura, note func(bool, string)) []map[string]any
}

// The external cooldowns of a request in the order applyGeneratedBuffs drives them: Mana Tide
// Totem from the party buffs, then Innervate and Power Infusion from the individual buffs.
func externalCooldowns(character *core.Character, request *proto.RaidSimRequest) []externalCooldown {
	player := request.Raid.Parties[0].Players[0]
	// The party buffs the drivers read hold what the agents add, such as a talented shaman's totem.
	party := character.Env.Raid.Parties[0].GetPartyBuffs(request.Raid.Parties[0].Buffs)
	return []externalCooldown{
		{externalManaTideLabel, party.GetManaTideTotems(), buffs.ManaTideTotemsCooldown(),
			manaTideActivation, manaTideAuraEffects},
		{"Innervates (External)", player.Buffs.GetInnervates(), buffs.InnervatesCooldown(),
			innervateActivation, innervateAuraEffects},
		{"Power Infusions (External)", player.Buffs.GetPowerInfusions(), buffs.PowerInfusionsCooldown(),
			nil, powerInfusionAuraEffects},
	}
}

func externalCooldownEffects(character *core.Character, request *proto.RaidSimRequest, unrepresented *[]string) []map[string]any {
	note := func(condition bool, message string) {
		if condition {
			*unrepresented = append(*unrepresented, message)
		}
	}
	effects := []map[string]any{}
	for _, external := range externalCooldowns(character, request) {
		aura := character.GetAura(external.label)
		if aura == nil {
			continue
		}
		var cast *core.Spell
		for _, spell := range character.Spellbook {
			if spell.RelatedSelfBuff == aura && spell.ActionID == aura.ActionID {
				cast = spell
			}
		}
		if cast == nil || cast.CD.Timer == nil || external.sources <= 0 {
			note(true, fmt.Sprintf("%s has no cooldown spell", external.label))
			continue
		}
		note(cast.CD.Duration != aura.Duration, fmt.Sprintf("%s's cooldown is not its aura's duration", external.label))
		effect := map[string]any{
			"kind": "external_cooldown", "spell_id": cast.ActionID.SpellID, "spell_tag": cast.ActionID.Tag,
			"aura": aura.Label, "aura_tag": aura.Tag, "sources": external.sources,
			"cooldown_ns": nanos(external.cooldown), "duration_ns": nanos(aura.Duration),
		}
		if external.activation != nil {
			effect["activation"] = external.activation(character)
		}
		effects = append(effects, effect)
		effects = append(effects, external.aura(character, request, aura, note)...)
	}
	return effects
}

// driveInnervates: a druid innervates a character who is nearly out of mana, so that every other
// mana cooldown is spent first. A mage burns mana fast enough that waiting for a flat thousand
// left would waste most of the innervate. The threshold is read once the environment finalizes.
func innervateActivation(character *core.Character) map[string]any {
	threshold := 1000.0
	if character.Class == proto.Class_ClassMage {
		threshold = maxManaAtFinalize * 0.4
	}
	return map[string]any{"kind": "mana_at_most", "threshold": threshold}
}

// driveManaTideTotems: no sooner than halfway through the fight, or 40 seconds in. The delay is
// read from the base duration once the environment finalizes.
func manaTideActivation(character *core.Character) map[string]any {
	return map[string]any{"kind": "not_before", "time_ns": nanos(min(character.Env.BaseDuration/2, manaTideInitialDelayCap))}
}

// AttachInnervateRegen: full spirit regeneration at five times the rate while the aura is up, its
// mana credited to regeneration metrics of their own. A separate reset simulation checks the aura
// changes nothing else.
func innervateAuraEffects(character *core.Character, request *proto.RaidSimRequest, aura *core.Aura, note func(bool, string)) []map[string]any {
	simulation := core.NewSim(request, simsignals.CreateSignals())
	simulation.Reset()
	player := simulation.Raid.Parties[0].Players[0].GetCharacter()
	before := player.PseudoStats
	beforeStats := player.GetStats()
	player.GetAura(aura.Label).Activate(simulation)
	after := player.PseudoStats
	note(!after.ForceFullSpiritRegen || after.SpiritRegenMultiplier != before.SpiritRegenMultiplier*innervateSpiritRegenMultiplier,
		"Innervate does not force full spirit regeneration at five times the rate")
	after.ForceFullSpiritRegen = before.ForceFullSpiritRegen
	after.SpiritRegenMultiplier = before.SpiritRegenMultiplier
	note(after != before || player.GetStats() != beforeStats, "Innervate changes more than spirit regeneration")
	regen := aura.ActionID
	regen.Tag = innervateRegenTag
	return []map[string]any{{
		"kind": "innervate_regen", "aura": aura.Label,
		"spirit_regen_multiplier": innervateSpiritRegenMultiplier, "regen_metrics_action_id": actionID(regen),
	}}
}

// The aura is the mana per 5 seconds the totem's row states, which the stat auras carry: a separate
// reset simulation checks it changes that stat alone, in a category of its own.
func manaTideAuraEffects(character *core.Character, request *proto.RaidSimRequest, aura *core.Aura, note func(bool, string)) []map[string]any {
	for _, ee := range aura.ExclusiveEffects {
		note(privateField(ee.Category, "effects").Len() != 1, "Mana Tide Totem shares its category")
	}
	simulation := core.NewSim(request, simsignals.CreateSignals())
	simulation.Reset()
	player := simulation.Raid.Parties[0].Players[0].GetCharacter()
	before := player.PseudoStats
	beforeStats := player.GetStats()
	player.GetAura(aura.Label).Activate(simulation)
	after := player.GetStats()
	note(after[stats.MP5] == beforeStats[stats.MP5], "Mana Tide Totem does not add mana per 5 seconds")
	after[stats.MP5] = beforeStats[stats.MP5]
	note(after != beforeStats || player.PseudoStats != before, "Mana Tide Totem changes more than mana per 5 seconds")
	return nil
}

// drivePowerInfusions: the priest has nothing to hold Power Infusion for, so it goes out on
// cooldown. The aura is the generated external copy, whose multipliers power_infusion.go describes;
// the runtime rolls the healing multiplier back by division, which is exact only when it is 1.
func powerInfusionAuraEffects(character *core.Character, request *proto.RaidSimRequest, aura *core.Aura, note func(bool, string)) []map[string]any {
	note(character.PseudoStats.HealingDealtMultiplier != 1,
		fmt.Sprintf("the external Power Infusion on a healing dealt multiplier of %v", character.PseudoStats.HealingDealtMultiplier))
	if effect := powerInfusionEffect(character, request, aura.ActionID, note); effect != nil {
		return []map[string]any{effect}
	}
	return nil
}
