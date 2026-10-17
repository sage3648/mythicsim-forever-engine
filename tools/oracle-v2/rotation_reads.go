// Rotation reads: what the rotation's values read of the simulation that the rest of the export
// leaves out, written only for a rotation that holds such a value.
package main

import (
	"fmt"
	"slices"
	"sort"

	"github.com/wowsims/forever/sim/core"
	"github.com/wowsims/forever/sim/core/proto"
	"github.com/wowsims/forever/sim/core/stats"
	googleProto "google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/reflect/protoreflect"
)

// RotationReads holds, for the values a rotation names, what they read.
type RotationReads struct {
	// apl_values_consumables.go: the potion and conjured item the consumables select.
	PotionID   *int32 `json:"potion_id,omitempty"`
	ConjuredID *int32 `json:"conjured_id,omitempty"`
	// apl_values_boss.go: the spells a target knows that a boss value names, and whether each
	// has a cooldown, which a fight without a target AI never starts.
	TargetSpells []RotationTargetSpell `json:"target_spells,omitempty"`
	// apl_values_misc.go APLValueMultipleCdUsages: the related self buff of each spell the value
	// names that has one, and whether the spell has a cooldown.
	SelfBuffs []RotationSelfBuff `json:"self_buffs,omitempty"`
	// apl_values_aura_sets.go: Character.ItemProcBuffs, which the item stat proc values and the
	// prepull's activateAllStatBuffProcAuras filter by stat and ICD.
	ItemProcBuffs []RotationItemProcBuff `json:"item_proc_buffs,omitempty"`
	// The major cooldowns whose buff aura moves one of the stats a stat buff cooldown value names,
	// as the value finds them while the rotation is built, by the stats.
	CooldownBuffs []RotationCooldownBuffs `json:"cooldown_buffs,omitempty"`
	// The spells each castAllStatBuffCooldowns casts, by its stats, as it found them after the
	// rotation was finalized; they leave the major cooldowns.
	CastCooldowns []RotationCooldownBuffs `json:"cast_cooldowns,omitempty"`
}

// RotationItemProcBuff is a StatBuffAura of Character.ItemProcBuffs.
type RotationItemProcBuff struct {
	Aura    string  `json:"aura"`
	Stats   []int32 `json:"stats"`
	Swapped bool    `json:"swapped,omitempty"`
}

// RotationCooldownBuffs are the cooldowns matching stats: their spells and buff auras.
type RotationCooldownBuffs struct {
	Stats  []int32    `json:"stats"`
	Spells []ActionID `json:"spells,omitempty"`
	Auras  []string   `json:"auras,omitempty"`
}

func statInts(list []stats.Stat) []int32 {
	out := make([]int32, len(list))
	for i, stat := range list {
		out[i] = int32(stat)
	}
	return out
}

// statKey is stats.IntTupleToStatsList of a value's stat types, as ints.
func statKey(s1, s2, s3 int32) []int32 {
	return statInts(stats.IntTupleToStatsList(s1, s2, s3))
}

// cooldownBuffsWithoutRotation reads the stat buff cooldowns as a value finds them while the
// rotation is built, before the rotation takes the cooldowns it casts: from an environment built
// with an empty rotation.
func cooldownBuffsWithoutRotation(request *proto.RaidSimRequest, keys [][]int32) []RotationCooldownBuffs {
	raid := googleProto.Clone(request.Raid).(*proto.Raid)
	raid.Parties[0].Players[0].Rotation = &proto.APLRotation{Type: proto.APLRotation_TypeAPL}
	env, _, _ := core.NewEnvironment(raid, request.Encounter, false, true)
	character := env.Raid.Parties[0].Players[0].GetCharacter()
	var out []RotationCooldownBuffs
	for _, key := range keys {
		statList := make([]stats.Stat, len(key))
		for i, stat := range key {
			statList[i] = stats.Stat(stat)
		}
		entry := RotationCooldownBuffs{Stats: key}
		for _, spell := range character.GetMatchingStatBuffSpells(statList) {
			entry.Spells = append(entry.Spells, *actionIDOrEmpty(spell.ActionID))
		}
		for _, aura := range character.GetMatchingStatBuffCooldownAuras(statList) {
			entry.Auras = append(entry.Auras, aura.Label)
		}
		out = append(out, entry)
	}
	return out
}

// castCooldowns reads the spells each castAllStatBuffCooldowns of the built rotation casts.
func castCooldowns(character *core.Character, keys [][]int32) []RotationCooldownBuffs {
	rotation := character.Rotation
	var actions []*core.APLAction
	for _, field := range []string{"priorityList", "prepullActions"} {
		list := readPrivate(privateField(rotation, field)).Interface().([]*core.APLAction)
		for _, action := range list {
			actions = append(actions, action.GetAllActions()...)
		}
	}
	found := map[string][]ActionID{}
	for _, action := range actions {
		impl, ok := readPrivate(privateField(action, "impl")).Interface().(*core.APLActionCastAllStatBuffCooldowns)
		if !ok {
			continue
		}
		key := fmt.Sprint(statInts(readPrivate(privateField(impl, "statTypesToMatch")).Interface().([]stats.Stat)))
		if _, seen := found[key]; seen {
			continue
		}
		subactions := readPrivate(privateField(impl, "allSubactions"))
		spells := []ActionID{}
		for i := 0; i < subactions.Len(); i++ {
			spell := readPrivate(privateField(subactions.Index(i).Interface(), "spell")).Interface().(*core.Spell)
			spells = append(spells, *actionIDOrEmpty(spell.ActionID))
		}
		found[key] = spells
	}
	var out []RotationCooldownBuffs
	for _, key := range keys {
		if spells, ok := found[fmt.Sprint(key)]; ok {
			out = append(out, RotationCooldownBuffs{Stats: key, Spells: spells})
		}
	}
	return out
}

func addKey(keys [][]int32, key []int32) [][]int32 {
	if slices.ContainsFunc(keys, func(known []int32) bool { return slices.Equal(known, key) }) {
		return keys
	}
	return append(keys, key)
}

func sortKeys(keys [][]int32) {
	sort.Slice(keys, func(i, j int) bool { return slices.Compare(keys[i], keys[j]) < 0 })
}

// RotationTargetSpell is a spell the first target knows, which its identical copies share.
type RotationTargetSpell struct {
	Spell       ActionID `json:"spell"`
	HasCooldown bool     `json:"has_cooldown,omitempty"`
}

// RotationSelfBuff is a spell's RelatedSelfBuff.
type RotationSelfBuff struct {
	Spell       ActionID `json:"spell"`
	Aura        string   `json:"aura"`
	HasCooldown bool     `json:"has_cooldown,omitempty"`
}

// rotationValues lists every value of the rotation, depth first, as messages.
func rotationValues(rotation *proto.APLRotation) []googleProto.Message {
	var values []googleProto.Message
	var visit func(message protoreflect.Message)
	visit = func(message protoreflect.Message) {
		values = append(values, message.Interface())
		message.Range(func(field protoreflect.FieldDescriptor, value protoreflect.Value) bool {
			switch {
			case field.IsList() && field.Message() != nil:
				for i := 0; i < value.List().Len(); i++ {
					visit(value.List().Get(i).Message())
				}
			case field.IsMap():
				value.Map().Range(func(_ protoreflect.MapKey, entry protoreflect.Value) bool {
					if field.MapValue().Message() != nil {
						visit(entry.Message())
					}
					return true
				})
			case field.Message() != nil:
				visit(value.Message())
			}
			return true
		})
	}
	if rotation != nil {
		visit(rotation.ProtoReflect())
	}
	return values
}

// targetOfBossValue is GetTargetUnit of a boss value's target unit: a target for no reference, the
// current, next, previous or an indexed one, nil for the player or a set, which read no target.
func targetOfBossValue(ref *proto.UnitReference, targets []*core.Unit) *core.Unit {
	if len(targets) == 0 {
		return nil
	}
	switch ref.GetType() {
	case proto.UnitReference_Unknown, proto.UnitReference_CurrentTarget,
		proto.UnitReference_NextTarget, proto.UnitReference_PreviousTarget:
		return targets[0]
	case proto.UnitReference_Target:
		if int(ref.GetIndex()) < len(targets) {
			return targets[ref.GetIndex()]
		}
	}
	return nil
}

func lessActionID(a, b ActionID) bool {
	if a.SpellID != b.SpellID {
		return a.SpellID < b.SpellID
	}
	if a.ItemID != b.ItemID {
		return a.ItemID < b.ItemID
	}
	if a.OtherID != b.OtherID {
		return a.OtherID < b.OtherID
	}
	return a.Tag < b.Tag
}

// rotationReads collects what the rotation's values read, or nil when none of them reads anything
// the export leaves out.
func rotationReads(request *proto.RaidSimRequest, simulation *core.Simulation, character *core.Character) *RotationReads {
	player := request.Raid.Parties[0].Players[0]
	reads := &RotationReads{}
	used := false
	seenTarget := map[core.ActionID]bool{}
	seenBuff := map[core.ActionID]bool{}
	selfBuff := func(id *proto.ActionID) {
		if id == nil {
			return
		}
		spell := aplSpell(&character.Unit, core.ProtoToActionID(id))
		if spell == nil || spell.RelatedSelfBuff == nil || seenBuff[spell.ActionID] {
			return
		}
		seenBuff[spell.ActionID] = true
		reads.SelfBuffs = append(reads.SelfBuffs, RotationSelfBuff{Spell: *actionIDOrEmpty(spell.ActionID),
			Aura: spell.RelatedSelfBuff.Label, HasCooldown: spell.CD.Timer != nil})
		used = true
	}
	targetSpell := func(id *proto.ActionID, ref *proto.UnitReference) {
		target := targetOfBossValue(ref, simulation.Encounter.AllTargetUnits)
		if id == nil || target == nil {
			return
		}
		actionID := core.ProtoToActionID(id)
		spell := target.GetSpell(actionID)
		if spell == nil || seenTarget[actionID] {
			return
		}
		seenTarget[actionID] = true
		reads.TargetSpells = append(reads.TargetSpells, RotationTargetSpell{Spell: *actionIDOrEmpty(actionID),
			HasCooldown: spell.CD.Timer != nil || spell.SharedCD.Timer != nil})
		used = true
	}
	itemProcs := false
	var cooldownKeys, castKeys [][]int32
	for _, value := range rotationValues(player.GetRotation()) {
		switch v := value.(type) {
		case *proto.APLValueAllItemStatProcsActive, *proto.APLValueAnyItemStatProcsActive,
			*proto.APLValueAnyItemStatProcsAvailable, *proto.APLValueItemProcsMinRemainingTime,
			*proto.APLValueItemProcsMaxRemainingICD, *proto.APLValueNumEquippedStatProcItems,
			*proto.APLActionActivateAllStatBuffProcAuras:
			itemProcs = true
			used = true
		case *proto.APLValueNumStatBuffCooldowns:
			cooldownKeys = addKey(cooldownKeys, statKey(v.StatType1, v.StatType2, v.StatType3))
			used = true
		case *proto.APLValueAnyStatBuffCooldownsActive:
			cooldownKeys = addKey(cooldownKeys, statKey(v.StatType1, v.StatType2, v.StatType3))
			used = true
		case *proto.APLValueAnyStatBuffCooldownsMinDuration:
			cooldownKeys = addKey(cooldownKeys, statKey(v.StatType1, v.StatType2, v.StatType3))
			used = true
		case *proto.APLActionCastAllStatBuffCooldowns:
			castKeys = addKey(castKeys, statKey(v.StatType1, v.StatType2, v.StatType3))
			used = true
		case *proto.APLValueSelectedPotion:
			id := character.Consumables.GetPotId()
			reads.PotionID = &id
			used = true
		case *proto.APLValueSelectedConjured:
			id := character.Consumables.GetConjuredId()
			reads.ConjuredID = &id
			used = true
		case *proto.APLValueBossSpellIsCasting:
			targetSpell(v.GetSpellId(), v.GetTargetUnit())
		case *proto.APLValueBossSpellTimeToReady:
			targetSpell(v.GetSpellId(), v.GetTargetUnit())
		case *proto.APLValueMultipleCdUsages:
			selfBuff(v.GetBaseSpellId())
			selfBuff(v.GetTargetSpellId())
		}
	}
	if !used {
		return nil
	}
	if itemProcs {
		for _, aura := range character.ItemProcBuffs {
			reads.ItemProcBuffs = append(reads.ItemProcBuffs, RotationItemProcBuff{Aura: aura.Label,
				Stats: statInts(aura.BuffedStatTypes), Swapped: aura.IsSwapped})
		}
	}
	sortKeys(cooldownKeys)
	sortKeys(castKeys)
	if len(cooldownKeys) > 0 {
		reads.CooldownBuffs = cooldownBuffsWithoutRotation(request, cooldownKeys)
	}
	if len(castKeys) > 0 {
		reads.CastCooldowns = castCooldowns(character, castKeys)
	}
	sort.Slice(reads.TargetSpells, func(i, j int) bool {
		return lessActionID(reads.TargetSpells[i].Spell, reads.TargetSpells[j].Spell)
	})
	sort.Slice(reads.SelfBuffs, func(i, j int) bool { return lessActionID(reads.SelfBuffs[i].Spell, reads.SelfBuffs[j].Spell) })
	return reads
}
