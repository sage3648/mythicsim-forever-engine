package rogue

import (
	"testing"

	"github.com/wowsims/forever/sim/core"
	"github.com/wowsims/forever/sim/core/proto"
)

// The undead's Forever racial replaced Shadow Resistance with Touch of the Grave, and the
// previous engine line once carried the removal without the replacement, so an undead player
// had no racial at all and nothing in the log to show for it. The rogue suites run Human and
// Orc only, which is how that went unnoticed; this asserts the racial fires and is attributed
// to its own spell id (the drain, 1260198).
func TestTouchOfTheGraveIsLogged(t *testing.T) {
	player := core.WithSpec(&proto.Player{
		Name:          "Rogue",
		Class:         proto.Class_ClassRogue,
		Race:          proto.Race_RaceUndead,
		Equipment:     DefaultGear.GearSet,
		Consumables:   DefaultConsumables,
		TalentsString: CombatTalents,
		Rotation:      core.GetAplRotation("../../ui/specs/rogue/dps/apls", "combat_sinister_strike").Rotation,
	}, DefaultOptions)

	result := core.RunRaidSim(&proto.RaidSimRequest{
		Raid:       core.SinglePlayerRaidProto(player, &proto.PartyBuffs{}, &proto.RaidBuffs{}, &proto.Debuffs{}),
		Encounter:  core.MakeSingleTargetEncounter(0),
		SimOptions: &proto.SimOptions{Iterations: 20, RandomSeed: 101},
	})
	if result.Error != nil {
		t.Fatalf("sim failed: %s", result.Error.Message)
	}

	var damage float64
	for _, action := range result.RaidMetrics.Parties[0].Players[0].Actions {
		if action.Id.GetSpellId() != 1260198 {
			continue
		}
		for _, target := range action.Targets {
			damage += target.Damage
		}
	}
	if damage <= 0 {
		t.Error("Touch of the Grave never dealt damage for an undead rogue; the racial is missing from the sim")
	}
}
