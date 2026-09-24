package dpswarrior

import (
	"testing"

	"github.com/wowsims/classic/sim/core"
	"github.com/wowsims/classic/sim/core/proto"
	googleProto "google.golang.org/protobuf/proto"
)

// Diamond Flask's 20 Strength (1318070) comes from finishing its 5 sec channel, so drunk at the pull
// it holds for a full minute from the 5 sec mark. Classic's flask gave 75 Strength on use.
func TestDiamondFlask(t *testing.T) {
	player := foreverWarrior(proto.Race_RaceOrc)
	player.Equipment = googleProto.Clone(player.Equipment).(*proto.EquipmentSpec)
	player.Equipment.Items[proto.ItemSlot_ItemSlotTrinket1] = &proto.ItemSpec{Id: DiamondFlaskID}
	player.Rotation = googleProto.Clone(player.Rotation).(*proto.APLRotation)
	player.Rotation.PriorityList = append([]*proto.APLListItem{{Action: &proto.APLAction{Action: &proto.APLAction_CastSpell{
		CastSpell: &proto.APLActionCastSpell{SpellId: &proto.ActionID{RawId: &proto.ActionID_ItemId{ItemId: DiamondFlaskID}}},
	}}}}, player.Rotation.PriorityList...)

	result := core.RunRaidSim(&proto.RaidSimRequest{
		Raid:       core.SinglePlayerRaidProto(player, nil, nil, nil),
		Encounter:  core.MakeSingleTargetEncounter(0),
		SimOptions: &proto.SimOptions{Iterations: 5, RandomSeed: 101, Ruleset: proto.Ruleset_RulesetForever},
	})
	if result.Error != nil {
		t.Fatal(result.Error.Message)
	}
	for _, aura := range result.RaidMetrics.Parties[0].Players[0].Auras {
		if aura.Id.GetSpellId() == 1318070 {
			if aura.UptimeSecondsAvg != 60 {
				t.Errorf("Diamond Flask's Strength up %v sec, want 60", aura.UptimeSecondsAvg)
			}
			return
		}
	}
	t.Error("Diamond Flask never granted its Strength")
}

const DiamondFlaskID = 20130
