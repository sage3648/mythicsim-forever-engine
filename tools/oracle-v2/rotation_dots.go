// Rotation value export: the dot base durations the rotation reads when it is built.
package main

import (
	"sort"

	"github.com/wowsims/forever/sim/core"
	"github.com/wowsims/forever/sim/core/proto"
	"google.golang.org/protobuf/reflect/protoreflect"
)

// RotationDotBaseDuration is what apl_values_dot.go APLValueDotBaseDuration captured for a spell.
type RotationDotBaseDuration struct {
	Spell          ActionID `json:"spell"`
	BaseDurationNs int64    `json:"base_duration_ns"`
}

// dotBaseDurationSpells lists the spells of the rotation's dotBaseDuration values.
func dotBaseDurationSpells(rotation *proto.APLRotation) []*proto.ActionID {
	var spells []*proto.ActionID
	var visit func(message protoreflect.Message)
	visit = func(message protoreflect.Message) {
		if value, ok := message.Interface().(*proto.APLValueDotBaseDuration); ok {
			spells = append(spells, value.GetSpellId())
			return
		}
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
	return spells
}

// rotationDotBaseDurations is APLValueDotBaseDuration's baseDuration: the dot's BaseDuration at the
// moment environment.go finalize builds the rotation, on the first target, which the rotation reads
// through GetAPLDot. Spell mods that an aura applies as it gains, such as a set bonus's, have not
// run yet, so the value is read from an environment built without a rotation and never reset. A
// spell without such a dot gives the value no meaning and is left out.
func rotationDotBaseDurations(request *proto.RaidSimRequest) []RotationDotBaseDuration {
	player := request.Raid.Parties[0].Players[0]
	spells := dotBaseDurationSpells(player.GetRotation())
	if len(spells) == 0 {
		return nil
	}
	env, _, _ := core.NewEnvironment(request.Raid, request.Encounter, false, true)
	character := env.Raid.Parties[0].Players[0].GetCharacter()
	seen := map[core.ActionID]bool{}
	var durations []RotationDotBaseDuration
	for _, id := range spells {
		if id == nil {
			continue
		}
		actionID := core.ProtoToActionID(id)
		if seen[actionID] {
			continue
		}
		seen[actionID] = true
		spell := aplSpell(&character.Unit, actionID)
		if spell == nil {
			continue
		}
		dot := spell.AOEDot()
		if dot == nil && len(env.Encounter.AllTargetUnits) > 0 {
			dot = spell.Dot(env.Encounter.AllTargetUnits[0])
		}
		if dot == nil {
			continue
		}
		durations = append(durations, RotationDotBaseDuration{Spell: *actionIDOrEmpty(actionID), BaseDurationNs: nanos(dot.BaseDuration())})
	}
	sort.Slice(durations, func(i, j int) bool {
		a, b := durations[i].Spell, durations[j].Spell
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
	})
	return durations
}

// aplSpell is APLRotation.GetAPLSpell: a spell marked for the APL, otherwise any with the action.
func aplSpell(unit *core.Unit, id core.ActionID) *core.Spell {
	if id.IsOtherAction(proto.OtherAction_OtherActionPotion) {
		for _, spell := range unit.Spellbook {
			if spell.Flags.Matches(core.SpellFlagCombatPotion) {
				return spell
			}
		}
		return nil
	}
	for _, spell := range unit.Spellbook {
		if spell.ActionID.SameAction(id) && spell.Flags.Matches(core.SpellFlagAPL) {
			return spell
		}
	}
	return unit.GetSpell(id)
}

func actionIDOrEmpty(id core.ActionID) *ActionID {
	if out := actionID(id); out != nil {
		return out
	}
	return &ActionID{}
}
