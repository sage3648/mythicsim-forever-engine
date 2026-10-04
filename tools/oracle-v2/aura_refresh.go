// Rotation auraShouldRefresh export: how each exclusive effect of an aura the rotation asks
// about reads in this fight, by exclusive_effect.go ShouldRefreshExclusiveEffects at the pinned
// revision.
package main

import (
	"encoding/json"
	"sort"

	"github.com/wowsims/forever/sim/core"
	"github.com/wowsims/forever/sim/core/proto"
	"google.golang.org/protobuf/encoding/protojson"
)

// How exclusive_effect.go ShouldRefreshExclusiveEffects reads an aura in this fight, effect by
// effect: "own" when no other effect shares its category, so it refreshes when inactive or
// about to expire; "never" when a permanent effect of another aura holds the category for the
// whole fight and the effect never takes it, so the aura still activates but its effect never
// applies. Anything else is "unknown". A stacking aura weighs its priority by stacks, which
// neither reading covers.
func exclusiveRefresh(aura *core.Aura) []string {
	modes := []string{}
	for _, ee := range aura.ExclusiveEffects {
		effects := privateField(ee.Category, "effects")
		active := ee.Category.GetActiveEffect()
		permanent := active != nil && active != ee && active.Aura.IsActive() && active.Aura.Duration == core.NeverExpires
		switch {
		case aura.MaxStacks > 0:
			modes = append(modes, "unknown")
		case effects.Len() == 1 && (active == nil || active == ee):
			modes = append(modes, "own")
		// A permanent effect that outranks this one, or bids the same for another spell and so
		// keeps the tie, never lets it take the category. One that bids the same for the same spell
		// gives the category up while this one is up and takes it back after, at the same value;
		// either way it outlasts any refresh window, so the aura never needs refreshing. A
		// single-aura category would block the aura itself instead.
		case permanent && !ee.Category.SingleAura && active.Priority >= ee.Priority:
			modes = append(modes, "never")
		default:
			modes = append(modes, "unknown")
		}
	}
	return modes
}

// apl_values_aura.go newValueAuraShouldRefresh: every aura an auraShouldRefresh value names,
// on the player or its current target (the default), with how its exclusive effects read. A value naming an
// aura the unit lacks has no aura and exports nothing.
func auraShouldRefreshEffects(character *core.Character, target *core.Unit, rotation *proto.APLRotation) []map[string]any {
	data, err := protojson.Marshal(rotation)
	fail(err)
	var tree any
	fail(json.Unmarshal(data, &tree))
	found := map[string]map[string]any{}
	var walk func(node any)
	walk = func(node any) {
		switch value := node.(type) {
		case map[string]any:
			if config, ok := value["auraShouldRefresh"].(map[string]any); ok {
				raw, err := json.Marshal(config)
				fail(err)
				parsed := &proto.APLValueAuraShouldRefresh{}
				fail(protojson.Unmarshal(raw, parsed))
				// apl_helpers.go GetTargetUnit: no unit reference means the current target.
				unit, name := target, "target"
				if parsed.GetSourceUnit().GetType() == proto.UnitReference_Self {
					unit, name = &character.Unit, "player"
				}
				if aura := unit.GetAuraByID(core.ProtoToActionID(parsed.GetAuraId())); aura != nil {
					found[name+"\x00"+aura.Label] = map[string]any{
						"kind": "aura_should_refresh", "unit": name, "aura": aura.Label, "modes": exclusiveRefresh(aura),
					}
				}
			}
			for _, child := range value {
				walk(child)
			}
		case []any:
			for _, child := range value {
				walk(child)
			}
		}
	}
	walk(tree)
	keys := []string{}
	for key := range found {
		keys = append(keys, key)
	}
	sort.Strings(keys)
	effects := []map[string]any{}
	for _, key := range keys {
		effects = append(effects, found[key])
	}
	return effects
}
