package hunter

import (
	"testing"

	"github.com/wowsims/classic/sim/core"
	"github.com/wowsims/classic/sim/core/proto"
)

// Summon Hawk keeps two hawks out at once (1293527's third effect), each assaulting on its own:
// casting on cooldown, both hawks tick and no third one ever does.
func TestSummonHawkTwoHawks(t *testing.T) {
	hawkID := spellData.SummonHawk.ByRank(4).SpellID
	castHawk := &proto.APLListItem{Action: &proto.APLAction{Action: &proto.APLAction_CastSpell{
		CastSpell: &proto.APLActionCastSpell{SpellId: &proto.ActionID{RawId: &proto.ActionID_SpellId{SpellId: hawkID}}},
	}}}

	player := &proto.Player{
		Class:              proto.Class_ClassHunter,
		Race:               proto.Race_RaceOrc,
		Equipment:          core.GetGearSet("../../ui/hunter/gear_sets", "p0.bis").GearSet,
		TalentsString:      "00000000001", // Summon Hawk alone
		Rotation:           &proto.APLRotation{Type: proto.APLRotation_TypeAPL, PriorityList: []*proto.APLListItem{castHawk}},
		Spec:               P1PlayerOptions,
		DistanceFromTarget: 30,
	}
	result := core.RunRaidSim(&proto.RaidSimRequest{
		Raid:       core.SinglePlayerRaidProto(player, nil, nil, nil),
		Encounter:  core.MakeSingleTargetEncounter(0),
		SimOptions: &proto.SimOptions{Iterations: 10, RandomSeed: 1, Ruleset: proto.Ruleset_RulesetForever},
	})
	if result.Error != nil {
		t.Fatal(result.Error.Message)
	}

	ticks := map[int32]int32{}
	for _, action := range result.RaidMetrics.Parties[0].Players[0].Actions {
		if action.Id.GetSpellId() != hawkID {
			continue
		}
		for _, target := range action.Targets {
			ticks[action.Id.GetTag()] += target.Ticks + target.CritTicks
		}
	}
	if ticks[1] == 0 || ticks[2] == 0 || ticks[3] != 0 {
		t.Fatalf("hawk ticks by hawk: %v, want two hawks both assaulting and no third", ticks)
	}
}
