// Shared effects of stats that change during a fight and of the weapon, consumable and
// raid procs every melee build can carry. Each formula mirrors the cited Go file at the
// pinned revision.
package main

import (
	"fmt"
	"math"
	"reflect"
	"sort"
	"time"
	"unsafe"

	"github.com/wowsims/forever/sim/core"
	"github.com/wowsims/forever/sim/core/buffs"
	"github.com/wowsims/forever/sim/core/dbcenums"
	"github.com/wowsims/forever/sim/core/proto"
	"github.com/wowsims/forever/sim/core/simsignals"
	"github.com/wowsims/forever/sim/core/spelldata"
	"github.com/wowsims/forever/sim/core/stats"
)

// The combination key of Go Character.SpiritManaRegenPerSecond, which is not a stat.
const spiritRegenKey = "SpiritManaRegenPerSecond"

// The stats the Rust runtime reads during a fight.
var dynamicReadStats = []stats.Stat{stats.SpellDamage, stats.AttackPower, stats.RangedAttackPower,
	stats.SpellCritPercent, stats.PhysicalCritPercent, stats.MP5}

// Stats the Rust runtime reads during a fight that a combination carries only when one changes
// them: maximum mana, healing power and health, Spirit, which Life Tap and Dark Sacrifice read,
// the school spell damage stats spell.go SpellSchoolBonusDamage reads, the resistances a spell
// that hits the player rolls against and the physical damage spell.go BonusDamage adds to a
// physical spell.
var optionalReadStats = []stats.Stat{stats.Mana, stats.HealingPower, stats.Health, stats.Spirit,
	stats.ArcaneDamage, stats.FireDamage, stats.FrostDamage, stats.HolyDamage, stats.NatureDamage,
	stats.ShadowDamage, stats.ArcaneResistance, stats.FireResistance, stats.FrostResistance,
	stats.NatureResistance, stats.ShadowResistance, stats.PhysicalDamage}

// Auras of races, items and raid buffs whose gain and expiry change stats through
// AddStatsDynamic. A class adds its own through classExport.statAuras.
var commonStatAuraLabels = []string{"Blood Fury", "Elune's Light", "Holy Strength (MH)", "Holy Strength (OH)",
	"Windfury Totem (External)", "Battle Shout (External)", "Headmaster's Charge", "Crusader's Wrath",
	"Diamond Flask", "Mana Tide Totem (External)"}

// The stat auras of the character, in the order a combination's bits number them.
func characterStatAuras(character *core.Character, class classExport, agent core.Agent) []string {
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
	candidates = append(candidates, spellDataStatProcAuras(character)...)
	candidates = append(candidates, lionHornProcAura(character)...)
	// shared.NewSimpleStatActive: an item's on-use buff is a temporary stats aura.
	for _, spell := range character.Spellbook {
		if simpleStatActive(character, spell) != nil {
			candidates = append(candidates, spell.RelatedSelfBuff.Label)
		}
	}
	candidates = append(candidates, weaponAuraProcAuras...)
	for _, label := range candidates {
		if character.GetAura(label) != nil {
			labels = append(labels, label)
		}
	}
	return labels
}

// A function that reads more of a stat aura combination's reset simulation, which
// statAurasEffect has already set up. `exact` is false when its own setup left a different
// simulation than setStatAuras does, so the reader must set one up itself.
type comboReader func(mask int, simulation *core.Simulation, player *core.Character, exact bool)

// unit.go AddStatsDynamic recomputes every stat from the active flat bonuses, so stats are a
// function of which stat auras are active. Each combination is read from a separate reset
// simulation with exactly those auras active: combination i has aura j active when bit j is
// set, and an aura active after the reset, as a druid's starting form, is deactivated when
// its bit is clear. Maximum mana and healing power are read too when some combination changes
// them. The reader, when there is one, reads the same simulations for the target's swing.
func statAurasEffect(request *proto.RaidSimRequest, labels []string, reader comboReader) map[string]any {
	if len(labels) == 0 {
		return nil
	}
	if len(labels) > 10 {
		fail(fmt.Errorf("%d stat auras exceed the combination limit", len(labels)))
	}
	combos := []map[string]float64{}
	// The MP5 of the raw stats, which AddStatsDynamic adds each bonus to and takes it from again.
	rawMP5 := []float64{}
	changedSpiritRegen := false
	changed := map[string]bool{}
	var base map[string]float64
	for mask := 0; mask < 1<<len(labels); mask++ {
		simulation := core.NewSim(request, simsignals.CreateSignals())
		simulation.Reset()
		player := simulation.Raid.Parties[0].Players[0].GetCharacter()
		// An aura up from the reset, such as the default stance, is down where its bit is clear.
		for bit, label := range labels {
			if aura := player.GetAura(label); mask&(1<<bit) == 0 && aura.IsActive() {
				aura.Deactivate(simulation)
			}
		}
		exact := true
		for bit, label := range labels {
			aura := player.GetAura(label)
			if want := mask&(1<<bit) != 0; want && !aura.IsActive() {
				aura.Activate(simulation)
			} else if !want && aura.IsActive() {
				// An activation switched this aura on again: setStatAuras would leave it up.
				exact = false
				aura.Deactivate(simulation)
			}
		}
		values := statValues(player.GetStats())
		rawMP5 = append(rawMP5, player.GetStatsWithoutDeps()[stats.MP5])
		if mask == 0 {
			base = values
		}
		combo := map[string]float64{}
		for _, stat := range dynamicReadStats {
			combo[stat.StatName()] = values[stat.StatName()]
		}
		for _, stat := range optionalReadStats {
			combo[stat.StatName()] = values[stat.StatName()]
		}
		// Spirit regeneration follows Intellect and Spirit, which mana.go UpdateManaRegenRates
		// rereads when a stat aura changes either.
		combo[spiritRegenKey] = player.SpiritManaRegenPerSecond()
		if mask > 0 && combo[spiritRegenKey] != combos[0][spiritRegenKey] {
			changedSpiritRegen = true
		}
		combos = append(combos, combo)
		for name, value := range values {
			if value != base[name] {
				changed[name] = true
			}
		}
		if reader != nil {
			reader(mask, simulation, player, exact)
		}
	}
	names := []string{}
	for name := range changed {
		names = append(names, name)
	}
	sort.Strings(names)
	// The optional stats stay out of the combinations unless one changes them, and so does
	// spirit regeneration.
	for _, stat := range optionalReadStats {
		if !changed[stat.StatName()] {
			for _, combo := range combos {
				delete(combo, stat.StatName())
			}
		}
	}
	if !changedSpiritRegen {
		for _, combo := range combos {
			delete(combo, spiritRegenKey)
		}
	}
	effect := map[string]any{"kind": "stat_auras", "auras": labels, "combos": combos, "changed": names}
	if raw := rawMP5Bonuses(labels, rawMP5); len(raw) != 0 {
		effect["raw_mp5"] = raw
	}
	return effect
}

// unit.go AddStatsDynamic adds each bonus to the raw stats and takes it away again, so a bonus
// that is not a whole number leaves the sum a few ulps off its start once the aura expires (77
// plus Mana Tide Totem's 483.33 and minus it again is 76.99999999999994), where the combination
// reads the start. The raw MP5 bonus of such an aura lets the runtime add and take it as Go does.
// A bonus that is a multiple of 2^-10 (a whole number, or the 31.25 of Mana Spring Totem) is added
// and taken away exactly. Only Mana Tide Totem's is not, and a reset simulation checks that its
// aura adds exactly its value; any other inexact MP5 bonus is unrepresented.
func rawMP5Bonuses(labels []string, raw []float64) map[string]float64 {
	bonuses := map[string]float64{}
	for bit, label := range labels {
		delta := raw[1<<bit] - raw[0]
		if delta == 0 || (delta*1024 == math.Trunc(delta*1024) && (raw[0]+delta)-delta == raw[0]) {
			continue
		}
		if label == externalManaTideLabel && raw[0]+buffs.ManaTideTotemsValue(0) == raw[1<<bit] {
			bonuses[label] = buffs.ManaTideTotemsValue(0)
			continue
		}
		fail(fmt.Errorf("%s adds %v MP5, which is not added and taken away exactly", label, delta))
	}
	return bonuses
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

// Item procs common/shared/shared_utils.go applySpellDataDamageProc builds from client rows: the
// item's trigger aura, its trigger row and the damage row the proc casts.
var spellDataDamageProcs = []struct {
	label   string
	trigger int32
	damage  int32
}{{"Storm Gauntlets", 16615, 16614}, {"Orb of Fire", 16982, 13441}, {"Premier High Warlord's Shield Wall", 13959, 16782},
	{"Premier Grand Marshal's Aegis", 13959, 16782}}

// The proc mask of a "when struck in combat" trigger: melee and ranged hits taken.
const struckProcMask = core.ProcMaskMeleeMHAuto | core.ProcMaskMeleeOHAuto | core.ProcMaskMeleeMHSpecial |
	core.ProcMaskMeleeOHSpecial | core.ProcMaskRangedAuto | core.ProcMaskRangedSpecial

// applySpellDataDamageProc: a listener resolved from the trigger row that casts the damage spell at
// once on the unit hit, or, for a hit taken, on the attacker (procDamageTarget). Only a single
// target magic hit is described: dealt where it lands, or struck by a melee or ranged hit, which
// the target's swings are and the Goblin Sapper Charge's hit on the player is not.
func spellDataDamageProcEffects(character *core.Character, unrepresented *[]string) []map[string]any {
	effects := []map[string]any{}
	melee := false
	for _, spell := range character.Spellbook {
		melee = melee || spell.ProcMask.Matches(core.ProcMaskMeleeSpecial)
	}
	for _, proc := range spellDataDamageProcs {
		if character.GetAura(proc.label) == nil {
			continue
		}
		trigger := spelldata.MustFind(proc.trigger)
		damage := spelldata.MustFind(proc.damage)
		listener := spelldata.ProcTrigger(character, trigger, nil, spelldata.ItemProcChance(trigger))
		struck := listener.Callback == core.CallbackOnSpellHitTaken
		// Without a melee special, meleeItemListeners describes a dealt listener as inert.
		if !struck && !melee {
			continue
		}
		spell := -1
		for i, registered := range character.Spellbook {
			if registered.ActionID == (core.ActionID{SpellID: damage.ID}) {
				spell = i
			}
		}
		effect := damage.DamageEffect()
		// shared_utils.go damageDefenseType: a stated defense type, else the school's.
		defense := damage.DefenseTypeCore()
		if defense == core.DefenseTypeNone {
			defense = core.DefenseTypeMagic
			if damage.SpellSchool().Matches(core.SpellSchoolPhysical) {
				defense = core.DefenseTypeMelee
			}
		}
		// AttachProcTriggerCallback reads an unset chance as certain.
		chance := listener.ProcChance
		if chance == 0 {
			chance = 1
		}
		names := callbackNames(listener.Callback)
		if spell < 0 || effect == nil || effect == spelldata.NilEffect || damage.PeriodicDamageEffect() != spelldata.NilEffect || effect.HitsAnArea() ||
			effect.ChainTargets > 1 || damage.AppliesAnAuraToAnEnemy() || damage.Speed != 0 || defense != core.DefenseTypeMagic ||
			trigger.RPPM != 0 || listener.DPM != nil || listener.ExtraCondition != nil || len(names) != 1 ||
			(names[0] != "on_spell_hit_dealt" && !struck) || (listener.Outcome != core.OutcomeEmpty && listener.Outcome != core.OutcomeLanded) ||
			(struck && (listener.ProcMask != struckProcMask || listener.CanProcFromProcs || listener.IsWeaponProc ||
				listener.ClassSpellMask != 0 || listener.SpellFlags != core.SpellFlagNone || listener.ProcMaskExclude != core.ProcMaskUnknown)) {
			*unrepresented = append(*unrepresented, fmt.Sprintf("%s's proc is not a single target magic hit", proc.label))
			continue
		}
		// A struck proc hears the attacker's hits, which the Rust runtime tells by their masks.
		triggerSpells := []int{}
		if !struck {
			triggerSpells = procTriggerSpells(character, listener)
		}
		exported := map[string]any{
			"kind": "spell_data_damage_proc", "trigger_aura": proc.label, "trigger_spells": triggerSpells,
			"landed_only": listener.Outcome == core.OutcomeLanded, "require_damage": listener.RequireDamageDealt,
			"proc_chance": chance, "spell": spell, "average": effect.Average(core.CharacterLevel),
			"variance": effect.Variance, "can_crit": !damage.CannotCrit(),
		}
		if struck {
			exported["struck"] = true
		}
		effects = append(effects, exported)
	}
	return effects
}

// Set bonuses common/forever/item_sets_classic.go setStatProc builds: a proc trigger on the
// set bonus aura, rolling its proc manager on the hits it hears, that activates a temporary
// stats aura a batch window later. The trigger's name keys the roll.
var setStatProcs = []struct {
	setAura  string
	name     string
	aura     string
	auraID   int32
	stats    stats.Stats
	procMask core.ProcMask
}{{"Lightforge Armor 5P", "Item - Crusader's Wrath Proc - Lightforge Armor", "Crusader's Wrath", 27499,
	stats.Stats{stats.SpellDamage: 65, stats.HealingPower: 65}, core.ProcMaskMeleeWhiteHit}}

func setStatProcEffects(simulation *core.Simulation, character *core.Character, unrepresented *[]string) []map[string]any {
	effects := []map[string]any{}
	for _, proc := range setStatProcs {
		setAura := character.GetAura(proc.setAura)
		if setAura == nil {
			continue
		}
		if setAura.Dpm == nil || character.GetAura(proc.aura) == nil {
			*unrepresented = append(*unrepresented, fmt.Sprintf("%s has no proc manager", proc.setAura))
			continue
		}
		id := core.ActionID{SpellID: proc.auraID}
		effects = append(effects, map[string]any{
			"kind": "stat_proc", "trigger_aura": proc.setAura, "rng_label": proc.name, "aura": proc.aura,
			// AttachProcTriggerCallback: the trigger's mask and eligibility, then its manager.
			"chances": dpmChances(character, setAura.Dpm, simulation, func(spell *core.Spell) bool {
				return spell.ProcMask.Matches(proc.procMask) && !spell.Flags.Matches(core.SpellFlagProc)
			}),
			"gain_log":   fmt.Sprintf("Gained %s from %s.", proc.stats.FlatString(), id),
			"expire_log": fmt.Sprintf("Lost %s from fading %s.", proc.stats.FlatString(), id),
		})
	}
	return effects
}

// common/forever/enchants.go Fiery Weapon (803) and Lifestealing (1898), and Fiery Blaze (36) from
// enchants_auto_gen.go: a weapon proc on landed hits, at a rate of the enchanted hand, that casts
// the row's damage spell at once on the unit hit (SpellDataProcDamageSpell). Only a magic hit on one
// target is described, which an area hit is against the encounter's only target.
var weaponEnchantDamageProcs = []struct {
	label  string
	damage int32
}{{"Enchant Weapon - Fiery Weapon", 13897}, {"Enchant Weapon - Lifestealing", 20004}, {"Enchant: Fiery Blaze", 6297}}

// shared_utils.go calcMultiTargetDamage against a single target: a chain or a capped or split area
// hit casts one roll on it, and an uncapped area hit one roll times the encounter's AoE cap
// multiplier, which is exactly one for a single target.
func singleTargetMultiHit(character *core.Character, row *spelldata.Spell, effect *spelldata.Effect) bool {
	if !effect.HitsAnArea() && effect.ChainTargets <= 1 {
		return true
	}
	return character.Env.ActiveTargetCount() == 1 && character.Env.Encounter.AOECapMultiplier() == 1
}

func weaponEnchantDamageProcEffects(simulation *core.Simulation, character *core.Character, unrepresented *[]string) []map[string]any {
	effects := []map[string]any{}
	for _, proc := range weaponEnchantDamageProcs {
		aura := character.GetAura(proc.label)
		if aura == nil {
			continue
		}
		damage := spelldata.MustFind(proc.damage)
		spell := -1
		for i, registered := range character.Spellbook {
			if registered.ActionID == (core.ActionID{SpellID: damage.ID}) {
				spell = i
			}
		}
		effect := damage.DamageEffect()
		if aura.Dpm == nil || aura.Icd != nil || spell < 0 || effect == spelldata.NilEffect || damage.PeriodicDamageEffect() != spelldata.NilEffect ||
			!singleTargetMultiHit(character, damage, effect) || damage.AppliesAnAuraToAnEnemy() || damage.Speed != 0 ||
			character.Spellbook[spell].DefenseType != core.DefenseTypeMagic {
			*unrepresented = append(*unrepresented, fmt.Sprintf("%s's proc is not a single target magic hit", proc.label))
			continue
		}
		chances := dpmChances(character, aura.Dpm, simulation, func(spell *core.Spell) bool {
			return !spell.Flags.Matches(core.SpellFlagSuppressWeaponProcs)
		})
		triggers := []int{}
		for _, chance := range chances {
			triggers = append(triggers, chance.Spell)
		}
		effects = append(effects, map[string]any{
			"kind": "spell_data_damage_proc", "trigger_aura": aura.Label, "trigger_spells": triggers,
			"landed_only": true, "require_damage": false, "proc_chance": 1.0, "chances": chances,
			"spell": spell, "average": effect.Average(character.Level), "variance": effect.Variance,
			"can_crit": !damage.CannotCrit(),
		})
	}
	return effects
}

// Items common/shared/shared_utils.go NewProcDamageEffect builds by hand: a listener on landed hits
// at a legacy proc manager's rate that casts a hit of a Go literal range at once on the unit hit.
// common/classic/items_trinkets.go Heart of Wyrmthalak is a magic hit on melee and ranged hits.
// common/classic/items_store_gaps.go Iceblade Hacker and Warblade of Caer Darrow are Frost hits of
// the melee defense type on the landed melee hits that dealt damage, at a fixed chance of 1 on the
// hand holding the weapon: damageOutcome gives them the melee special hit table with a crit, where
// a magic hit rolls the magic one.
var procDamageItems = []struct {
	label                string
	spellID              int32
	minDamage, maxDamage float64
	defense              core.DefenseType
	requireDamage        bool
	procMask             core.ProcMask
}{{"Heart of Wyrmthalak", 27655, 112, 168, core.DefenseTypeMagic, false, core.ProcMaskMeleeOrRanged},
	{"Iceblade Hacker", 1298414, 40.70000076293945, 40.70000076293945, core.DefenseTypeMelee, true, core.ProcMaskMelee},
	{"Warblade of Caer Darrow", 1298499, 27.719999313354492, 27.719999313354492, core.DefenseTypeMelee, true, core.ProcMaskMelee}}

func procDamageItemEffects(simulation *core.Simulation, character *core.Character, unrepresented *[]string) []map[string]any {
	effects := []map[string]any{}
	for _, item := range procDamageItems {
		aura := character.GetAura(item.label)
		if aura == nil {
			continue
		}
		spell := -1
		for i, registered := range character.Spellbook {
			if registered.ActionID == (core.ActionID{SpellID: item.spellID}) {
				spell = i
			}
		}
		hit := "magic"
		if item.defense == core.DefenseTypeMelee {
			hit = "melee"
		}
		if aura.Dpm == nil || aura.Icd != nil || spell < 0 || character.Spellbook[spell].DefenseType != item.defense ||
			aura.OnSpellHitDealt == nil || aura.OnSpellHitTaken != nil || aura.OnPeriodicDamageDealt != nil {
			*unrepresented = append(*unrepresented, fmt.Sprintf("%s's proc is not a single target %s hit", item.label, hit))
			continue
		}
		listener := core.ProcTrigger{ProcMask: item.procMask, Outcome: core.OutcomeLanded}
		effect := map[string]any{
			"kind": "spell_data_damage_proc", "trigger_aura": item.label, "trigger_spells": procTriggerSpells(character, listener),
			"landed_only": true, "require_damage": item.requireDamage, "proc_chance": 1.0, "spell": spell,
			"average": 0.0, "variance": 0.0, "roll": []float64{item.minDamage, item.maxDamage}, "can_crit": true,
			"chances": dpmChances(character, aura.Dpm, simulation, func(spell *core.Spell) bool {
				return spell.ProcMask.Matches(item.procMask) && !spell.Flags.Matches(core.SpellFlagProc)
			}),
		}
		if item.defense == core.DefenseTypeMelee {
			effect["outcome"] = "melee_special_hit_and_crit"
		}
		effects = append(effects, effect)
	}
	return effects
}

// Enchants common/shared/shared_utils.go NewSpellDataHealProc builds from client rows: a listener
// resolved from the trigger row that casts the heal row on the wearer at once
// (applySpellDataSelfProc). Only a direct heal, a share of maximum health or a rolled amount, is
// described. common/forever/enchants_auto_gen.go Recovery.
var spellDataHealProcs = []struct {
	label         string
	trigger, heal int32
}{{"Enchant Weapon - Recovery", 1248761, 1248759}}

func spellDataHealProcEffects(character *core.Character, unrepresented *[]string) []map[string]any {
	effects := []map[string]any{}
	for _, proc := range spellDataHealProcs {
		aura := character.GetAura(proc.label)
		if aura == nil {
			continue
		}
		trigger := spelldata.MustFind(proc.trigger)
		heal := spelldata.MustFind(proc.heal)
		listener := spelldata.ProcTrigger(character, trigger, nil, spelldata.ItemProcChance(trigger))
		spell := -1
		for i, registered := range character.Spellbook {
			if registered.ActionID == (core.ActionID{SpellID: heal.ID}) {
				spell = i
			}
		}
		effect := heal.ProcHealEffect()
		chance := listener.ProcChance
		if chance == 0 {
			chance = 1
		}
		names := callbackNames(listener.Callback)
		icd := aura.Icd != nil && aura.Icd.Duration != listener.ICD
		if spell < 0 || effect == nil || effect == spelldata.NilEffect || effect.Aura == dbcenums.A_PERIODIC_HEAL ||
			(effect.Type != dbcenums.E_HEAL_PCT && effect.Type != dbcenums.E_HEAL) || listener.DPM != nil || trigger.RPPM != 0 ||
			listener.ExtraCondition != nil || len(names) != 1 || names[0] != "on_spell_hit_dealt" || icd ||
			(listener.ICD != 0) != (aura.Icd != nil) || character.Spellbook[spell].BonusCoefficient != 0 {
			*unrepresented = append(*unrepresented, fmt.Sprintf("%s's proc is not a direct heal on the wearer", proc.label))
			continue
		}
		exported := map[string]any{
			"kind": "spell_data_heal_proc", "trigger_aura": proc.label, "trigger_spells": procTriggerSpells(character, listener),
			"outcome": outcomeNames(listener.Outcome), "require_damage": listener.RequireDamageDealt, "proc_chance": chance,
			"spell": spell, "can_crit": !heal.CannotCrit(),
			"healing_dealt_multiplier":       character.PseudoStats.HealingDealtMultiplier,
			"healing_taken_multiplier":       character.PseudoStats.HealingTakenMultiplier,
			"table_healing_dealt_multiplier": character.AttackTables[character.UnitIndex].HealingDealtMultiplier,
			"bonus_healing_taken":            character.PseudoStats.BonusHealingTaken,
		}
		if effect.Type == dbcenums.E_HEAL_PCT {
			exported["max_health_share"] = effect.Percent()
		} else {
			exported["average"], exported["variance"] = effect.Average(character.Level), effect.Variance
		}
		effects = append(effects, exported)
	}
	return effects
}

func meleeProcEffects(simulation *core.Simulation, character *core.Character, unrepresented *[]string) []map[string]any {
	effects := spellDataDamageProcEffects(character, unrepresented)
	effects = append(effects, spellDataHealProcEffects(character, unrepresented)...)
	effects = append(effects, weaponEnchantDamageProcEffects(simulation, character, unrepresented)...)
	effects = append(effects, procDamageItemEffects(simulation, character, unrepresented)...)
	effects = append(effects, weaponDamageProcEffects(simulation, character, unrepresented)...)
	effects = append(effects, lobotomizerEffects(simulation, character, unrepresented)...)
	effects = append(effects, ebonHiltEffects(simulation, character, unrepresented)...)
	effects = append(effects, setStatProcEffects(simulation, character, unrepresented)...)
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
	effects = append(effects, flurryAxeEffects(simulation, character, unrepresented)...)
	effects = append(effects, weaponAuraProcEffects(simulation, character, unrepresented)...)
	// common/classic/items_weapons.go Dragon's Call: a weapon proc on landed hits, at one proc a
	// minute of the weapon's speed, whose handler summons the Emerald Dragon Whelp for 15 seconds
	// a spell batch window later (emerald_dragon_whelp.go); its rotation spits half the time.
	if aura := character.GetAura("Emerald Dragon Whelp Proc"); aura != nil {
		var whelp *core.Pet
		for _, pet := range character.Pets {
			if summonedPet(pet) && pet.Name == "Emerald Dragon Whelp" {
				whelp = pet
			}
		}
		if aura.Dpm == nil || whelp == nil {
			*unrepresented = append(*unrepresented, "Dragon's Call has no proc manager or whelp")
		} else {
			effects = append(effects, map[string]any{
				"kind": "emerald_dragon_whelp", "trigger_aura": aura.Label, "pet": whelp.Label,
				"chances": dpmChances(character, aura.Dpm, simulation, func(spell *core.Spell) bool {
					return !spell.Flags.Matches(core.SpellFlagSuppressWeaponProcs)
				}),
				"delay_ns": nanos(core.SpellBatchWindow), "duration_ns": nanos(15 * time.Second),
				"acid_spit_spell_id": int32(9591), "acid_spit_min": 374.0, "acid_spit_max": 503.0,
				"spit_chance": 0.5,
			})
		}
	}
	// common/classic/items_weapons.go Sulfuras, Hand of Ragnaros: a weapon proc at one proc a
	// minute of the weapon's speed that casts its Fireball at once, rolled 273 to 333 on the magic
	// hit table, whose landing applies a burn of 15 every 2 seconds; and Immolation, 5 Fire that
	// always lands on every melee attacker that lands a hit. Go literals.
	if aura := character.GetAura("Sulfuras, Hand of Ragnaros Proc"); aura != nil {
		immolation := character.GetAura("Immolation (Hand of Ragnaros)")
		fireball, burn := -1, -1
		for i, spell := range character.Spellbook {
			switch spell.ActionID {
			case core.ActionID{SpellID: 21162}:
				fireball = i
			case core.ActionID{SpellID: 21142}:
				burn = i
			}
		}
		if aura.Dpm == nil || immolation == nil || fireball < 0 || burn < 0 {
			*unrepresented = append(*unrepresented, "Sulfuras, Hand of Ragnaros is incomplete")
		} else {
			effects = append(effects, map[string]any{
				"kind": "sulfuras_hand_of_ragnaros", "trigger_aura": aura.Label,
				"chances": dpmChances(character, aura.Dpm, simulation, func(spell *core.Spell) bool {
					return !spell.Flags.Matches(core.SpellFlagSuppressWeaponProcs)
				}),
				"fireball_spell": fireball, "roll_min": 273.0, "roll_max": 333.0, "dot_base": 15.0,
				"immolation_aura": immolation.Label, "immolation_spell": burn, "immolation_damage": 5.0,
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
		// buffs/air_totem.go: the party holds one air totem and Windfury Totem outbids a party Grace
		// of Air, which the reset then leaves displaced or blocked, as the aura export records. A
		// Grace of Air still up after the reset, as a twisting shaman's, would contest the slot.
		if other := character.GetAura("Grace of Air Totem (External)"); other != nil && other.IsActive() {
			*unrepresented = append(*unrepresented, fmt.Sprintf("Windfury Totem shares the air totem slot with %s", other.Label))
		}
		grant := spelldata.ProcTrigger(character, spelldata.MustFind(10612), nil)
		spend := spelldata.ProcTrigger(character, spelldata.MustFind(10610), nil, spelldata.Chance(1))
		grantSpells := procTriggerSpells(character, grant)
		// Without melee autos Go registers no extra attack, and nothing the character casts can
		// trigger the totem then, as for a caster.
		if procAura == nil || trigger == nil || (extra < 0 && len(grantSpells) > 0) {
			*unrepresented = append(*unrepresented, "Windfury Totem is incomplete")
		} else {
			if grant.DPM != nil || len(callbackNames(grant.Callback)) != 1 || len(callbackNames(spend.Callback)) != 1 ||
				callbackNames(grant.Callback)[0] != "on_spell_hit_dealt" || callbackNames(spend.Callback)[0] != "on_spell_hit_dealt" {
				*unrepresented = append(*unrepresented, "Windfury Totem's triggers listen to other callbacks")
			}
			effect := map[string]any{
				"kind": "windfury_totem", "totem_aura": totem.Label, "period_ns": nanos(5 * time.Second),
				"trigger_aura": trigger.Label, "trigger_spells": grantSpells,
				"trigger_outcome": outcomeNames(grant.Outcome), "trigger_proc_chance": grant.ProcChance,
				"proc_aura": procAura.Label, "spend_spells": procTriggerSpells(character, spend),
				"spend_outcome":          outcomeNames(spend.Outcome),
				"trigger_require_damage": grant.RequireDamageDealt, "spend_require_damage": spend.RequireDamageDealt,
			}
			if extra >= 0 {
				effect["extra_attack_spell"] = extra
			}
			effects = append(effects, effect)
		}
	}
	return effects
}
