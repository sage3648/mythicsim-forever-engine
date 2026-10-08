// Player movement export: the movement speed a move runs at.
package main

import (
	"encoding/json"

	"github.com/wowsims/forever/sim/core"
	"github.com/wowsims/forever/sim/core/proto"
	"google.golang.org/protobuf/encoding/protojson"
)

// Whether the rotation moves: an APLActionMove or APLActionMoveDuration anywhere in it, in its
// prepull, which runs unchecked before the pull, in its priority list, or in a group or a
// sequence.
func rotationMoves(rotation *proto.APLRotation) bool {
	data, err := protojson.Marshal(rotation)
	fail(err)
	var tree any
	fail(json.Unmarshal(data, &tree))
	var moves func(node any) bool
	moves = func(node any) bool {
		switch value := node.(type) {
		case map[string]any:
			if _, ok := value["move"]; ok {
				return true
			}
			if _, ok := value["moveDuration"]; ok {
				return true
			}
			for _, child := range value {
				if moves(child) {
					return true
				}
			}
		case []any:
			for _, child := range value {
				if moves(child) {
					return true
				}
			}
		}
		return false
	}
	return moves(tree)
}

// movement.go: a unit moves at 7 yards a second times PseudoStats.MovementSpeedMultiplier, and
// MultiplyMovementSpeed changes the multiplier as an aura gains or fades. The effect holds the
// multiplier after the reset and the one every reset restores before the permanent auras change
// it, and names every aura that can change it other than the class's own dash aura and Prowl:
// the passive and active movement speed categories' members, whose effect multiplies the speed
// as it takes hold and logs a line, and Elemental Blessing, which multiplies it directly. It is
// written when the rotation moves, and when one of those auras exists, since the log follows
// them. Rust follows the multiplier, the categories and the class's dash and Prowl.
func playerMovementEffect(character *core.Character, rotation *proto.APLRotation) map[string]any {
	speedAuras := []string{}
	for _, aura := range character.GetAuras() {
		changes := aura.Label == "Elemental Blessing"
		for _, effect := range aura.ExclusiveEffects {
			if name := effect.Category.Name; name == "PassiveMovementSpeed" || name == "ActiveMovementSpeed" {
				changes = true
			}
		}
		if changes {
			speedAuras = append(speedAuras, aura.Label)
		}
	}
	if !rotationMoves(rotation) && len(speedAuras) == 0 {
		return nil
	}
	return map[string]any{
		"kind": "player_movement", "speed_multiplier": character.PseudoStats.MovementSpeedMultiplier,
		"initial_speed_multiplier": initialPseudoStats(&character.Unit).MovementSpeedMultiplier,
		"speed_auras":              speedAuras,
	}
}
