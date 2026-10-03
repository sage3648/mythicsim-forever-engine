// Shared effects of stats that change during a fight and of the weapon, consumable and
// raid procs every melee build can carry. Each formula mirrors the cited Go file at the
// pinned revision.
package main

import (
	"fmt"
	"reflect"
	"sort"
	"time"
	"unsafe"

	"github.com/wowsims/forever/sim/core"
	"github.com/wowsims/forever/sim/core/proto"
	"github.com/wowsims/forever/sim/core/simsignals"
	"github.com/wowsims/forever/sim/core/spelldata"
	"github.com/wowsims/forever/sim/core/stats"
)

// The stats the Rust runtime reads during a fight.
var dynamicReadStats = []stats.Stat{stats.SpellDamage, stats.AttackPower, stats.RangedAttackPower,
	stats.SpellCritPercent, stats.PhysicalCritPercent, stats.MP5}

// Auras of races, items and raid buffs whose gain and expiry change stats through
// AddStatsDynamic. A class adds its own through classExport.statAuras.
var commonStatAuraLabels = []string{"Blood Fury", "Elune's Light", "Holy Strength (MH)", "Holy Strength (OH)",
	"Windfury Totem (External)", "Battle Shout (External)"}

// unit.go AddStatsDynamic recomputes every stat from the active flat bonuses, so stats are a
// function of which stat auras are active. Each combination is read from a separate reset
// simulation with those auras active: combination i has aura j active when bit j is set.
func statAurasEffect(request *proto.RaidSimRequest, character *core.Character, class classExport, agent core.Agent) map[string]any {
	labels := []string{}
	candidates := append([]string{}, commonStatAuraLabels...)
	// consumes.go: a potion's stat buff is a temporary stats aura named for the potion.
	for _, spell := range character.Spellbook {
		if consumable := core.GetConsumableByID(spell.ActionID.ItemID); spell.ActionID.ItemID != 0 &&
			spell.Flags.Matches(core.SpellFlagPotion) && consumable.BuffDuration > 0 {
			candidates = append(candidates, consumable.Name)
		}
	}
	if class.statAuras != nil {
		candidates = append(candidates, class.statAuras(agent, character)...)
	}
	for _, label := range candidates {
		if character.GetAura(label) != nil {
			labels = append(labels, label)
		}
	}
	if len(labels) == 0 {
		return nil
	}
	if len(labels) > 10 {
		fail(fmt.Errorf("%d stat auras exceed the combination limit", len(labels)))
	}
	combos := []map[string]float64{}
	changed := map[string]bool{}
	var base map[string]float64
	for mask := 0; mask < 1<<len(labels); mask++ {
		simulation := core.NewSim(request, simsignals.CreateSignals())
		simulation.Reset()
		player := simulation.Raid.Parties[0].Players[0].GetCharacter()
		for bit, label := range labels {
			if mask&(1<<bit) != 0 {
				player.GetAura(label).Activate(simulation)
			}
		}
		values := statValues(player.GetStats())
		if mask == 0 {
			base = values
		}
		combo := map[string]float64{}
		for _, stat := range dynamicReadStats {
			combo[stat.StatName()] = values[stat.StatName()]
		}
		combos = append(combos, combo)
		for name, value := range values {
			if value != base[name] {
				changed[name] = true
			}
		}
	}
	names := []string{}
	for name := range changed {
		names = append(names, name)
	}
	sort.Strings(names)
	return map[string]any{"kind": "stat_auras", "auras": labels, "combos": combos, "changed": names}
}

type spellChance struct {
	Spell  int     `json:"spell"`
	Chance float64 `json:"chance"`
}

// The chance a dynamic proc manager rolls for each spell it hears: procs.go Proc takes the
// first mask entry the spell's proc mask matches. Every entry in scope is a static chance.
func dpmChances(character *core.Character, dpm *core.DynamicProcManager, simulation *core.Simulation, eligible func(*core.Spell) bool) []spellChance {
	masks := privateField(dpm, "procMasks")
	procs := privateField(dpm, "procChances")
	// Unexported fields refuse Interface(); read the slice through its address instead.
	procs = reflect.NewAt(procs.Type(), unsafe.Pointer(procs.UnsafeAddr())).Elem()
	chances := []spellChance{}
	for i, spell := range character.Spellbook {
		if !eligible(spell) {
			continue
		}
		for entry := 0; entry < masks.Len(); entry++ {
			if core.ProcMask(masks.Index(entry).Uint()).Matches(spell.ProcMask) {
				proc := procs.Index(entry).Interface().(core.DynamicProc)
				chances = append(chances, spellChance{Spell: i, Chance: proc.Chance(simulation)})
				break
			}
		}
	}
	return chances
}

func meleeProcEffects(simulation *core.Simulation, character *core.Character, unrepresented *[]string) []map[string]any {
	effects := []map[string]any{}
	// common/classic/enchants.go Crusader (1900): a weapon proc on landed hits, at one proc a
	// minute of each hand's speed, that activates that hand's Holy Strength and heals.
	if aura := character.GetAura("Enchant Weapon - Crusader"); aura != nil {
		if aura.Dpm == nil {
			*unrepresented = append(*unrepresented, "Crusader has no proc manager")
		} else {
			buffs := stats.Stats{stats.Strength: 100}
			id := core.ActionID{SpellID: 20007}
			effects = append(effects, map[string]any{
				"kind": "crusader", "trigger_aura": aura.Label, "mh_aura": "Holy Strength (MH)", "oh_aura": "Holy Strength (OH)",
				"chances": dpmChances(character, aura.Dpm, simulation, func(spell *core.Spell) bool {
					return !spell.Flags.Matches(core.SpellFlagSuppressWeaponProcs)
				}),
				"heal_min": 75.0, "heal_max": 125.0, "heal_metrics_action_id": actionID(id),
				"mh_gain_log":   fmt.Sprintf("Gained %s from %s.", buffs.FlatString(), id.WithTag(1)),
				"mh_expire_log": fmt.Sprintf("Lost %s from fading %s.", buffs.FlatString(), id.WithTag(1)),
				"oh_gain_log":   fmt.Sprintf("Gained %s from %s.", buffs.FlatString(), id.WithTag(2)),
				"oh_expire_log": fmt.Sprintf("Lost %s from fading %s.", buffs.FlatString(), id.WithTag(2)),
			})
		}
	}
	// common/classic/items_weapons.go Ironfoe (11684) and common/forever/items_trinkets.go Hand of
	// Justice (11815): proc triggers on landed melee hits, Go literal chances, with the aura's
	// cooldown, whose handlers grant two and one extra main hand attacks at once.
	for _, item := range []struct {
		label   string
		chance  float64
		attacks int32
	}{{"Fury of Forgewright", 0.06, 2}, {"Hand of Justice", 0.01, 1}} {
		if aura := character.GetAura(item.label); aura != nil {
			effects = append(effects, map[string]any{
				"kind": "extra_attack_proc", "trigger_aura": aura.Label, "proc_chance": item.chance, "attacks": item.attacks,
			})
		}
	}
	// core/consumes.go registerDragonbreathChili: a 5% proc on landed melee hits, Go literals,
	// whose handler waits a spell batch window and casts a rolled Fire hit.
	if aura := character.GetAura("Dragonbreath Chili"); aura != nil {
		trigger := core.ProcTrigger{ProcMask: core.ProcMaskMelee}
		effects = append(effects, map[string]any{
			"kind": "dragonbreath_chili", "trigger_aura": aura.Label, "spell_id": int32(15851),
			"proc_chance": 0.05, "trigger_spells": procTriggerSpells(character, trigger),
			"roll_min": 57.0, "roll_max": 73.0, "delay_ns": nanos(core.SpellBatchWindow),
		})
	}
	// buffs/drivers.go driveWindfuryTotem: the totem aura, refreshed every 5 seconds, holds a
	// trigger that can grant charges of attack power and cast an extra main hand attack; the
	// charges are spent by landed autos. Triggers resolve from client rows as Go resolves them.
	if totem := character.GetAura("Windfury Totem"); totem != nil {
		procAura := character.GetAura("Windfury Totem (External)")
		trigger := character.GetAura("Windfury Totem Trigger")
		var extra = -1
		for i, spell := range character.Spellbook {
			if spell.ActionID == (core.ActionID{OtherID: proto.OtherAction_OtherActionAttack, Tag: 25584}) {
				extra = i
			}
		}
		// buffs/air_totem.go: the party holds one air totem; another bidder would contest the slot.
		if other := character.GetAura("Grace of Air Totem (External)"); other != nil {
			*unrepresented = append(*unrepresented, fmt.Sprintf("Windfury Totem shares the air totem slot with %s", other.Label))
		}
		if procAura == nil || trigger == nil || extra < 0 {
			*unrepresented = append(*unrepresented, "Windfury Totem is incomplete")
		} else {
			grant := spelldata.ProcTrigger(character, spelldata.MustFind(10612), nil)
			spend := spelldata.ProcTrigger(character, spelldata.MustFind(10610), nil, spelldata.Chance(1))
			if grant.DPM != nil || len(callbackNames(grant.Callback)) != 1 || len(callbackNames(spend.Callback)) != 1 ||
				callbackNames(grant.Callback)[0] != "on_spell_hit_dealt" || callbackNames(spend.Callback)[0] != "on_spell_hit_dealt" {
				*unrepresented = append(*unrepresented, "Windfury Totem's triggers listen to other callbacks")
			}
			effects = append(effects, map[string]any{
				"kind": "windfury_totem", "totem_aura": totem.Label, "period_ns": nanos(5 * time.Second),
				"trigger_aura": trigger.Label, "trigger_spells": procTriggerSpells(character, grant),
				"trigger_outcome": outcomeNames(grant.Outcome), "trigger_proc_chance": grant.ProcChance,
				"proc_aura": procAura.Label, "spend_spells": procTriggerSpells(character, spend),
				"spend_outcome": outcomeNames(spend.Outcome), "extra_attack_spell": extra,
			})
		}
	}
	return effects
}
