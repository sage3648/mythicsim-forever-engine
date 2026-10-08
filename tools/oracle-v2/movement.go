// Player movement export: the movement speed a move runs at.
package main

import (
	"github.com/wowsims/forever/sim/core"
	"github.com/wowsims/forever/sim/core/proto"
)

// Whether the rotation moves: an APLActionMove in its prepull, which runs unchecked before the pull,
// or in its priority list.
func rotationMoves(rotation *proto.APLRotation) bool {
	for _, prepull := range rotation.GetPrepullActions() {
		if prepull.GetAction().GetMove() != nil {
			return true
		}
	}
	for _, item := range rotation.GetPriorityList() {
		if item.GetAction().GetMove() != nil {
			return true
		}
	}
	return false
}

// movement.go: a unit moves at 7 yards a second times PseudoStats.MovementSpeedMultiplier, and
// MultiplyMovementSpeed changes the multiplier as an aura gains or fades. The effect holds the
// multiplier after the reset, and names every aura that can change it other than the class's own
// dash aura: the passive and active movement speed categories' members, whose gain at reset logs
// a line, and Elemental Blessing, which multiplies it directly. Rust follows only the multiplier
// and the class's dash.
func playerMovementEffect(character *core.Character, rotation *proto.APLRotation) map[string]any {
	if !rotationMoves(rotation) {
		return nil
	}
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
	return map[string]any{
		"kind": "player_movement", "speed_multiplier": character.PseudoStats.MovementSpeedMultiplier,
		"speed_auras": speedAuras,
	}
}
