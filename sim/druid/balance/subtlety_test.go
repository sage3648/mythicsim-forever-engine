package balance

import (
	"math"
	"testing"

	"github.com/wowsims/classic/sim/core"
	"github.com/wowsims/classic/sim/core/proto"
	"github.com/wowsims/classic/sim/druid"
)

// Subtlety (17118, MOD_THREAT on Arcane and Nature) cuts all the threat those spells make, Faerie
// Fire's flat threat included.
func TestSubtletyCutsFaerieFireFlatThreat(t *testing.T) {
	flatThreat := func(talents string) float64 {
		raid := core.SinglePlayerRaidProto(&proto.Player{
			Class:         proto.Class_ClassDruid,
			Race:          proto.Race_RaceNightElf,
			Equipment:     core.GetGearSet("../../../ui/balance_druid/gear_sets", "p0.bis").GearSet,
			TalentsString: talents,
			Rotation:      &proto.APLRotation{Type: proto.APLRotation_TypeAPL},
			Spec:          PlayerOptionsAdaptive,
		}, nil, nil, nil)
		env, _, _ := core.NewEnvironment(raid, core.MakeSingleTargetEncounter(0), proto.Ruleset_RulesetForever, false)
		return env.Raid.Parties[0].Players[0].(druid.DruidAgent).GetDruid().FaerieFire.FlatThreatBonus
	}

	without := flatThreat("5532220115001351--505002") // P1Talents with Subtlety taken out
	with := flatThreat(P1Talents)                     // 3/3 Subtlety
	if without == 0 || math.Abs(with-without*0.7) > 1e-9 {
		t.Errorf("Faerie Fire flat threat %v with 3/3 Subtlety, want 70%% of %v", with, without)
	}
}
