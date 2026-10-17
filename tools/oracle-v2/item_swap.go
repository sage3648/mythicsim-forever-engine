// sim/core/item_swaps.go: a character that swaps items during the fight.
//
// The fight starts on the request's equipment (the item swap's Main set) and an APL itemSwap action
// changes it to the Swap1 set; the iteration's end and the next reset swap back. What a swap changes is
// exported twice: the callbacks Go runs for each swapped slot, in registration order, each as the
// state its target takes under either set (callbacks), and the prepared state of a simulation that
// swapped to Swap1 at the pull, as its difference from the reset's (swapped). The runtime runs the
// callbacks and takes the values the difference names for the weapon it holds.
package main

import (
	"encoding/json"
	"fmt"
	"math"
	"reflect"
	"runtime"
	"slices"
	"sort"
	"strings"
	"unsafe"

	"github.com/wowsims/forever/sim/core"
	"github.com/wowsims/forever/sim/core/proto"
	"github.com/wowsims/forever/sim/core/simsignals"
	"github.com/wowsims/forever/sim/core/stats"
)

// The labels sim/core/item_swaps.go enableItemSwap gives the set auras.
const (
	itemSwapMainLabel    = "Item Swap: Main"
	itemSwapSwappedLabel = "Item Swap: Swapped"
)

// A JSON value as encoding/json decodes it.
func genericJSON(value any) any {
	data, err := json.Marshal(value)
	fail(err)
	var out any
	fail(json.Unmarshal(data, &out))
	return out
}

type jsonChange struct {
	Path  string `json:"path"`
	Value any    `json:"value"`
}

// The changes from a to b as JSON pointers, sorted by path: an object key either side lacks, an
// array whose length differs and a value that differs are each one change to b's value (null for a
// key b lacks).
func jsonDiff(path string, a, b any, out *[]jsonChange) {
	switch av := a.(type) {
	case map[string]any:
		bv, ok := b.(map[string]any)
		if !ok {
			*out = append(*out, jsonChange{path, b})
			return
		}
		keys := map[string]bool{}
		for k := range av {
			keys[k] = true
		}
		for k := range bv {
			keys[k] = true
		}
		names := []string{}
		for k := range keys {
			names = append(names, k)
		}
		sort.Strings(names)
		for _, k := range names {
			child := path + "/" + strings.ReplaceAll(strings.ReplaceAll(k, "~", "~0"), "/", "~1")
			x, xok := av[k]
			y, yok := bv[k]
			if !xok || !yok {
				*out = append(*out, jsonChange{child, y})
				continue
			}
			jsonDiff(child, x, y, out)
		}
	case []any:
		bv, ok := b.([]any)
		if !ok || len(av) != len(bv) {
			*out = append(*out, jsonChange{path, b})
			return
		}
		for i := range av {
			jsonDiff(fmt.Sprintf("%s/%d", path, i), av[i], bv[i], out)
		}
	case float64:
		bv, ok := b.(float64)
		if !ok || math.Float64bits(av) != math.Float64bits(bv) {
			*out = append(*out, jsonChange{path, b})
		}
	default:
		if !reflect.DeepEqual(a, b) {
			*out = append(*out, jsonChange{path, b})
		}
	}
}

// The item swap's equipment: the Main set and the Swap1 set, each as the character wears it, with
// the temporary enchantments the imbues put on its weapons (ItemSwap.AddTempEnchant).
type swapSets struct {
	main, swapped core.Equipment
	slots         []proto.ItemSlot
}

func itemSwapSets(character *core.Character) swapSets {
	swap := &character.ItemSwap
	original := readPrivate(privateField(swap, "originalEquip")).Interface().(core.Equipment)
	swapEquip := readPrivate(privateField(swap, "swapEquip")).Interface().(core.Equipment)
	slots := readPrivate(privateField(swap, "slots")).Interface().([]proto.ItemSlot)
	sets := swapSets{main: original, swapped: original, slots: slots}
	for _, slot := range slots {
		sets.swapped[slot] = swapEquip[slot]
	}
	return sets
}

func (s swapSets) set(swapped bool) *core.Equipment {
	if swapped {
		return &s.swapped
	}
	return &s.main
}

func hasItemIn(equipment *core.Equipment, itemID int32, slots []proto.ItemSlot) bool {
	for _, slot := range slots {
		if equipment[slot].ID == itemID {
			return true
		}
	}
	return false
}

func hasEnchantIn(equipment *core.Equipment, effectID int32, slots []proto.ItemSlot) bool {
	for _, slot := range slots {
		if equipment[slot].Enchant.EffectID == effectID || equipment[slot].TempEnchant == effectID {
			return true
		}
	}
	return false
}

// A callback's captured variables, read from its closure: a Go func value points at the code
// pointer followed by the variables the literal captures. Each decoder below names the words its
// literal's closure holds at the pinned revision and checks what it reads.
type closure struct {
	words *[16]uintptr
}

func closureOf(callback reflect.Value) closure {
	pointer := *(*unsafe.Pointer)(unsafe.Pointer(callback.UnsafeAddr()))
	return closure{words: (*[16]uintptr)(pointer)}
}

func (c closure) word(i int) uintptr { return c.words[i] }

func (c closure) aura(character *core.Character, i int) *core.Aura {
	for _, aura := range character.GetAuras() {
		if uintptr(unsafe.Pointer(aura)) == c.words[i] {
			return aura
		}
	}
	fail(fmt.Errorf("item swap callback word %d is not one of the character's auras", i))
	return nil
}

func (c closure) slots(i int) []proto.ItemSlot {
	pointer, length := c.words[i], int(c.words[i+1])
	if length < 0 || length > int(core.NumItemSlots) {
		fail(fmt.Errorf("item swap callback word %d is not a slot list", i))
	}
	return unsafe.Slice((*proto.ItemSlot)(unsafe.Pointer(pointer)), length)
}

// The states an aura callback leaves its aura in: on, activated if inactive; activate, Activate,
// which refreshes an active aura; off, deactivated; keep, untouched.
func auraState(on bool, activate string, otherwise string) string {
	if on {
		return activate
	}
	return otherwise
}

// Callbacks that change nothing a swap exports as an action: the dynamic proc managers, whose
// chances the swapped state's effects carry, and AddStatProcBuff's IsSwapped, which only APL actions
// the gate refuses read.
var itemSwapRebuilds = map[string]bool{
	"github.com/wowsims/forever/sim/core.(*Character).NewLegacyPPMManager.func1":                                   true,
	"github.com/wowsims/forever/sim/core.(*Character).newDynamicProcManagerWithDynamicProcMask.func1":              true,
	"github.com/wowsims/forever/sim/core.(*Character).newDynamicProcManagerWithDynamicProcMaskDynamicProcChance.func1": true,
	"github.com/wowsims/forever/sim/core.(*Character).AddStatProcBuff.func1":                                       true,
	// druid.go Initialize: the paw the form fights with is rebuilt from the weapon in the hand,
	// which the swapped state's melee and form effects carry.
	"github.com/wowsims/forever/sim/druid.(*Druid).Initialize.func2": true,
}

// A dynamic spell modifier a callback switches: its kind, value and the player's spells it reaches.
func spellModOf(character *core.Character, mod *core.SpellMod, note func(bool, string)) map[string]any {
	affected := []int{}
	for _, spell := range mod.AffectedSpells {
		position := spellPosition(character, spell)
		note(position < 0, "an item swap switches a spell modifier on a pet's spell, which is unsupported")
		affected = append(affected, position)
	}
	kind := ""
	switch mod.Kind {
	case core.SpellMod_DamageDone_Pct:
		kind = "damage_done_pct"
	case core.SpellMod_BonusCrit_Percent:
		kind = "bonus_crit_percent"
	default:
		note(true, fmt.Sprintf("an item swap switches a spell modifier of kind %d, which is unsupported", mod.Kind))
	}
	return map[string]any{"kind": kind, "value": readPrivate(privateField(mod, "floatValue")).Float(), "spells": affected,
		"active": mod.IsActive}
}

func mainHandType(equipment *core.Equipment) proto.HandType {
	if mh := equipment.MainHand(); mh.ID != 0 && mh.HandType == proto.HandType_HandTypeTwoHand {
		return proto.HandType_HandTypeTwoHand
	}
	return proto.HandType_HandTypeOneHand
}

// What each callback of a swapped slot does under each set, in Go's order.
func itemSwapCallbacks(character *core.Character, sets swapSets, note func(bool, string)) []map[string]any {
	swap := &character.ItemSwap
	field := privateField(swap, "onItemSwapCallbacks")
	callbacks := reflect.NewAt(field.Type(), unsafe.Pointer(field.UnsafeAddr())).Elem()
	out := []map[string]any{}
	for _, slot := range sets.slots {
		list := callbacks.Index(int(slot))
		for i := 0; i < list.Len(); i++ {
			callback := list.Index(i)
			name := runtime.FuncForPC(callback.Pointer()).Name()
			if itemSwapRebuilds[name] {
				continue
			}
			c := closureOf(callback)
			op := map[string]any{"slot": slot.String()}
			both := func(fn func(equipment *core.Equipment) string) {
				op["main"], op["swapped"] = fn(sets.set(false)), fn(sets.set(true))
			}
			switch name {
			case "github.com/wowsims/forever/sim/core.(*ItemSwap).RegisterActive.func1":
				// character, itemActionID, itemID, slots, swap.
				itemID := int32(c.word(4))
				slots := c.slots(5)
				spell := character.GetSpell(core.ActionID{ItemID: itemID})
				note(spell == nil, fmt.Sprintf("item %d's use is not registered", itemID))
				op["kind"], op["spell"] = "item_use", spellPosition(character, spell)
				if aura := character.GetAuraByID(core.ActionID{ItemID: itemID}); aura != nil {
					op["aura"] = aura.Label
				}
				op["main"], op["swapped"] = hasItemIn(sets.set(false), itemID, slots), hasItemIn(sets.set(true), itemID, slots)
			case "github.com/wowsims/forever/sim/core.(*ItemSwap).registerProcInternal.func1":
				// isItemProc, character, config: ItemID, EnchantId, Aura, Slots.
				isItem := c.word(1)&0xff != 0
				itemID, enchantID := int32(uint32(c.word(3))), int32(uint32(c.word(3)>>32))
				aura := c.aura(character, 4)
				slots := c.slots(5)
				op["kind"], op["aura"] = "aura", aura.Label
				both(func(e *core.Equipment) string {
					on := false
					if isItem {
						on = hasItemIn(e, itemID, slots)
					} else if enchantID != 0 {
						on = hasEnchantIn(e, enchantID, slots)
					}
					return auraState(on, "on", "off")
				})
			case "github.com/wowsims/forever/sim/core.(*ItemSwap).RegisterEnchantBuffWithSlots.func1":
				// character, enchantID, slots, buffAura.
				enchantID := int32(c.word(2))
				slots := c.slots(3)
				aura := c.aura(character, 6)
				op["kind"], op["aura"] = "aura", aura.Label
				both(func(e *core.Equipment) string { return auraState(hasEnchantIn(e, enchantID, slots), "keep", "off") })
			case "github.com/wowsims/forever/sim/paladin.(*Paladin).followMainHand.func1",
				"github.com/wowsims/forever/sim/warrior.(*Warrior).registerTwoHandedWeaponSpecialization.func1":
				// The paladin's: paladin, handType, weaponMod. The warrior's: warrior, weaponMod, for
				// a two-hander.
				handType, word := proto.HandType(c.word(2)), 3
				if strings.Contains(name, "warrior") {
					handType, word = proto.HandType_HandTypeTwoHand, 2
				}
				mod := (*core.SpellMod)(unsafe.Pointer(c.word(word)))
				note(mod.Kind != core.SpellMod_DamageDone_Pct || mod.School != core.SpellSchoolPhysical,
					"a weapon specialization's modifier is not the physical damage done the runtime implements")
				op["kind"], op["mod"] = "spell_mod", spellModOf(character, mod, note)
				op["main"], op["swapped"] = mainHandType(sets.set(false)) == handType, mainHandType(sets.set(true)) == handType
			case "github.com/wowsims/forever/sim/warrior.(*Warrior).registerWeaponmaster.func11":
				// readWeapons, setCritMods, setActive, armorIgnoreAura, &armorIgnoreOn. setCritMods
				// holds critMods, the crit modifier of each hand, and &critMask.
				setCritMods := closure{words: (*[16]uintptr)(unsafe.Pointer(c.word(2)))}
				code := runtime.FuncForPC(setCritMods.word(0)).Name()
				note(code != "github.com/wowsims/forever/sim/warrior.(*Warrior).registerWeaponmaster.func3",
					"Weaponmaster's swap callback is not the shape the exporter reads")
				hmap := setCritMods.word(1)
				critMods := reflect.NewAt(reflect.TypeOf(map[core.ProcMask]*core.SpellMod{}), unsafe.Pointer(&hmap)).Elem()
				weaponIs := func(item *core.Item, types ...proto.WeaponType) bool {
					return item.ID != 0 && slices.Contains(types, item.WeaponType)
				}
				// The armor ignore aura first, then each hand's crit, in hand order: the map's
				// order does not change what the modifiers do.
				aura := c.aura(character, 4)
				out = append(out, map[string]any{"slot": slot.String(), "kind": "aura", "aura": aura.Label,
					"main": auraState(weaponIs(sets.main.MainHand(), proto.WeaponType_WeaponTypeMace, proto.WeaponType_WeaponTypeStaff), "activate", "off"),
					"swapped": auraState(weaponIs(sets.swapped.MainHand(), proto.WeaponType_WeaponTypeMace, proto.WeaponType_WeaponTypeStaff), "activate", "off")})
				for _, hand := range []core.ProcMask{core.ProcMaskMeleeMH, core.ProcMaskMeleeOH} {
					mod := critMods.MapIndex(reflect.ValueOf(hand))
					note(!mod.IsValid(), "Weaponmaster's crit modifiers are not the shape the exporter reads")
					item := func(e *core.Equipment) *core.Item {
						if hand == core.ProcMaskMeleeMH {
							return e.MainHand()
						}
						return e.OffHand()
					}
					out = append(out, map[string]any{"slot": slot.String(), "kind": "spell_mod",
						"mod":     spellModOf(character, mod.Interface().(*core.SpellMod), note),
						"main":    weaponIs(item(sets.set(false)), proto.WeaponType_WeaponTypeAxe, proto.WeaponType_WeaponTypePolearm),
						"swapped": weaponIs(item(sets.set(true)), proto.WeaponType_WeaponTypeAxe, proto.WeaponType_WeaponTypePolearm)})
				}
				continue
			case "github.com/wowsims/forever/sim/shaman.(*Shaman).registerWindfuryTotemSpell.func9":
				// wfIntermediateAuraForExclusitivity, taken down at every swap.
				aura := c.aura(character, 1)
				op["kind"], op["aura"], op["main"], op["swapped"] = "aura", aura.Label, "off", "off"
			case "github.com/wowsims/forever/sim/shaman.(*Shaman).RegisterOnItemSwapWithImbue.func1":
				// shaman, effectID, procMask, aura: up while a weapon in the hand carries the imbue.
				effectID := int32(c.word(2))
				aura := c.aura(character, 4)
				op["kind"], op["aura"] = "aura", aura.Label
				both(func(e *core.Equipment) string {
					return auraState(e.MainHand().TempEnchant == effectID || e.OffHand().TempEnchant == effectID, "activate", "off")
				})
			case "github.com/wowsims/forever/sim/shaman.(*Shaman).RegisterRockbiterImbue.func3":
				// shaman, aura: up while a weapon in the hand carries Rockbiter.
				aura := c.aura(character, 2)
				op["kind"], op["aura"] = "aura", aura.Label
				both(func(e *core.Equipment) string {
					return auraState(e.MainHand().TempEnchant == 3021 || e.OffHand().TempEnchant == 3021, "on", "off")
				})
			case "github.com/wowsims/forever/sim/shaman.(*Shaman).makeFTProcTriggerAura.func2":
				// shaman, aura: off when the weapon now in the hand lacks Flametongue, Activate when
				// it carries it and the weapon it replaced did not.
				note(slot != proto.ItemSlot_ItemSlotMainHand, "Flametongue's swap callback on another slot is unsupported")
				aura := c.aura(character, 2)
				op["kind"], op["aura"] = "aura_on_change", aura.Label
				op["main"], op["swapped"] = sets.main.MainHand().TempEnchant == 5, sets.swapped.MainHand().TempEnchant == 5
			case "github.com/wowsims/forever/sim/core.registerPerWeaponMainHandImbue.func1":
				// character, pullID and swapID, effects: each stone's auras are up while its
				// weapon is in the main hand. A Dense stone's flat damage is unsupported.
				pullID, swapID := int32(uint32(c.word(2))), int32(uint32(c.word(2)>>32))
				type weaponImbueEffect struct {
					auras      []*core.Aura
					flat       float64
					pull, swap bool
				}
				effects := unsafe.Slice((**weaponImbueEffect)(unsafe.Pointer(c.word(3))), int(c.word(4)))
				for _, imbue := range effects {
					note(imbue.flat != 0, "a Dense stone's damage on an item swap's weapon is unsupported")
					inHand := func(e *core.Equipment) bool {
						id := e.MainHand().ID
						return id == pullID && imbue.pull || id == swapID && imbue.swap
					}
					for _, aura := range imbue.auras {
						if !slices.Contains(character.GetAuras(), aura) {
							fail(fmt.Errorf("a main hand imbue's aura is not the character's"))
						}
						out = append(out, map[string]any{"slot": slot.String(), "kind": "aura", "aura": aura.Label,
							"main": auraState(inHand(sets.set(false)), "on", "off"), "swapped": auraState(inHand(sets.set(true)), "on", "off")})
					}
				}
				continue
			default:
				note(true, fmt.Sprintf("item swap callback %s is unsupported", strings.TrimPrefix(name, "github.com/wowsims/forever/sim/")))
				continue
			}
			out = append(out, op)
		}
	}
	return out
}

// The item swap effect, or nil without an item swap.
func itemSwapEffect(request *proto.RaidSimRequest, digest, scenario string, simulation *core.Simulation,
	character *core.Character, main Prepared, note func(bool, string)) map[string]any {
	swap := &character.ItemSwap
	if !swap.IsEnabled() {
		return nil
	}
	sets := itemSwapSets(character)
	note(len(sets.slots) != 1 || sets.slots[0] != proto.ItemSlot_ItemSlotMainHand,
		"an item swap of a slot other than the main hand alone is unsupported")
	equipmentStats := readPrivate(privateField(swap, "equipmentStats"))
	weaponStats := readPrivate(equipmentStats.FieldByName("weaponSlots")).Interface().(stats.Stats)
	effect := map[string]any{
		"kind": "item_swap", "slots": []string{proto.ItemSlot_ItemSlotMainHand.String()},
		"main_aura": itemSwapMainLabel, "swapped_aura": itemSwapSwappedLabel,
		"to_swapped_log": "Item Swap - Stats Change: " + weaponStats.FlatString(),
		"to_main_log":    "Item Swap - Stats Change: " + weaponStats.Invert().FlatString(),
		"callbacks":      itemSwapCallbacks(character, sets, note),
		"swapped":        swappedSnapshot(request, digest, scenario, main),
	}
	logs := []map[string]any{}
	for _, label := range itemSwapStatAuras(character) {
		if label == itemSwapSwappedLabel {
			continue
		}
		gain, expire := itemSwapAuraLogs(label)
		if len(gain) == 0 && len(expire) == 0 {
			continue
		}
		logs = append(logs, map[string]any{"aura": label, "gain_logs": gain, "expire_logs": expire})
	}
	effect["stat_aura_logs"] = logs
	// An aura the unit's reset activates (a proc trigger's OnReset) that the swap's reset then
	// switches off for the Main set: the runtime's reset activates it too, so that its
	// deactivation leaves the listener lists in Go's order.
	resetActivates := []string{}
	for _, op := range effect["callbacks"].([]map[string]any) {
		label, ok := op["aura"].(string)
		if !ok || op["main"] != "off" || slices.Contains(resetActivates, label) {
			continue
		}
		if aura := character.GetAura(label); aura == nil || aura.IsActive() || aura.OnReset == nil {
			continue
		}
		probeSim, probe := itemSwapProbe()
		aura := probe.GetAura(label)
		aura.OnReset(aura, probeSim)
		if aura.IsActive() {
			resetActivates = append(resetActivates, label)
		}
	}
	effect["reset_activates"] = resetActivates
	// What a switched aura does beyond its stats as it is gained and fades.
	auraEffects := []map[string]any{}
	if aura := character.GetAura("Weaponmaster (Mace/Staff)"); aura != nil {
		for _, op := range effect["callbacks"].([]map[string]any) {
			if op["aura"] == aura.Label {
				// talents_arms.go registerWeaponmaster: every attack table ignores more armor.
				talents := classTalents(character.Env.Raid.Parties[0].Players[0]).Interface().(*proto.WarriorTalents)
				auraEffects = append(auraEffects, map[string]any{"aura": aura.Label,
					"armor_ignore_factor": warriorWeaponmaster.EffectAt(2).FractionAt(talents.Weaponmaster)})
				break
			}
		}
	}
	effect["aura_effects"] = auraEffects
	return effect
}

// The prepared state once the character has swapped to its item swap set at the pull, as the
// difference from the reset's.
func swappedSnapshot(request *proto.RaidSimRequest, digest, scenario string, main Prepared) []jsonChange {
	swapped := prepareSet(request, digest, scenario, true)
	changes := []jsonChange{}
	jsonDiff("", genericJSON(main), genericJSON(swapped), &changes)
	return changes
}

// The request prepareSet is exporting, which the item swap's stat aura reads need.
var itemSwapRequest *proto.RaidSimRequest

// A reset simulation of the request being exported.
func itemSwapProbe() (*core.Simulation, *core.Character) {
	simulation := core.NewSim(itemSwapRequest, simsignals.CreateSignals())
	simulation.Reset()
	return simulation, simulation.Raid.Parties[0].Players[0].GetCharacter()
}

// The auras an item swap callback switches whose gain or expiry changes the character's stats, in
// callback order, and the stat aura that carries the swap's own equipment stats: unit.go
// AddStatsDynamic makes them stat auras. Each is read from a reset simulation of its own.
func itemSwapStatAuras(character *core.Character) []string {
	if !character.ItemSwap.IsEnabled() || itemSwapRequest == nil {
		return nil
	}
	labels := []string{}
	ignore := func(bool, string) {}
	for _, op := range itemSwapCallbacks(character, itemSwapSets(character), ignore) {
		label, ok := op["aura"].(string)
		if !ok || slices.Contains(labels, label) {
			continue
		}
		simulation, probe := itemSwapProbe()
		aura := probe.GetAura(label)
		before := probe.GetStats()
		if aura.IsActive() {
			aura.Deactivate(simulation)
		} else {
			aura.Activate(simulation)
		}
		if probe.GetStats() != before {
			labels = append(labels, label)
		}
	}
	return append(labels, itemSwapSwappedLabel)
}

// The lines a stat aura logs as it is gained and as it fades beyond the aura's own, read from a
// reset simulation that logs: core.NewTemporaryStatsAura's "Gained" and "Lost" lines.
func itemSwapAuraLogs(label string) (gain, expire []string) {
	simulation, probe := itemSwapProbe()
	aura := probe.GetAura(label)
	lines := []string{}
	prefix := probe.LogLabel() + " "
	simulation.Log = func(message string, vals ...any) {
		line := fmt.Sprintf(message, vals...)
		if !strings.HasPrefix(line, prefix) {
			return
		}
		line = strings.TrimPrefix(line, prefix)
		for _, skip := range []string{"Aura gained:", "Aura faded:", "Aura refreshed:", "Dynamic stat change:", "Dynamic dep "} {
			if strings.HasPrefix(line, skip) {
				return
			}
		}
		lines = append(lines, line)
	}
	if aura.IsActive() {
		aura.Deactivate(simulation)
		expire = lines
		lines = []string{}
		aura.Activate(simulation)
		gain = lines
	} else {
		aura.Activate(simulation)
		gain = lines
		lines = []string{}
		aura.Deactivate(simulation)
		expire = lines
	}
	simulation.Log = nil
	return gain, expire
}

// unit.go AddDynamicEquipStats with the swap's weapon slot stats, which activating the swap's stat
// aura stands for in a combination's simulation.
func addItemSwapEquipStats(simulation *core.Simulation, aura *core.Aura) {
	if aura.Label != itemSwapSwappedLabel {
		return
	}
	character := simulation.Raid.Parties[0].Players[0].GetCharacter()
	equipmentStats := readPrivate(privateField(&character.ItemSwap, "equipmentStats"))
	character.AddDynamicEquipStats(simulation, readPrivate(equipmentStats.FieldByName("weaponSlots")).Interface().(stats.Stats))
}
