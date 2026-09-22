package dpswarrior

import (
	"testing"

	"github.com/wowsims/classic/sim/core"
	"github.com/wowsims/classic/sim/core/proto"
	googleProto "google.golang.org/protobuf/proto"
)

// Moving a whole priority list into an action group, run by one group reference, and
// reading a constant through a variable must not change a single swing: same seed, same DPS.
func TestAPLGroupWrappedRotationMatchesFlat(t *testing.T) {
	flat := core.GetAplRotation("../../../ui/warrior/apls", "dps_no_reck").Rotation

	grouped := googleProto.Clone(flat).(*proto.APLRotation)
	grouped.ValueVariables = []*proto.APLValueVariable{
		{Name: "limit", Value: &proto.APLValue{Value: &proto.APLValue_Const{Const: &proto.APLValueConst{Val: "1"}}}},
	}
	replaced := replaceConst(grouped.PriorityList, "1", &proto.APLValue{Value: &proto.APLValue_VariableRef{VariableRef: &proto.APLValueVariableRef{Name: "limit"}}})
	if replaced == 0 {
		t.Fatalf("expected the preset to contain a const 1 to route through a variable")
	}
	grouped.Groups = []*proto.APLGroup{{Name: "main", Actions: grouped.PriorityList}}
	grouped.PriorityList = []*proto.APLListItem{{Action: &proto.APLAction{
		Action: &proto.APLAction_GroupReference{GroupReference: &proto.APLActionGroupReference{GroupName: "main"}},
	}}}

	flatDps := runWarriorDps(flat)
	groupedDps := runWarriorDps(grouped)
	if flatDps == 0 || flatDps != groupedDps {
		t.Fatalf("flat DPS %v != grouped DPS %v", flatDps, groupedDps)
	}
}

// Replaces every const with the given value inside conditions, returning the count.
func replaceConst(items []*proto.APLListItem, val string, with *proto.APLValue) int {
	n := 0
	var walk func(v *proto.APLValue)
	walk = func(v *proto.APLValue) {
		if v == nil {
			return
		}
		switch x := v.Value.(type) {
		case *proto.APLValue_Const:
			if x.Const.Val == val {
				v.Value = with.Value
				n++
			}
		case *proto.APLValue_And:
			for _, c := range x.And.Vals {
				walk(c)
			}
		case *proto.APLValue_Or:
			for _, c := range x.Or.Vals {
				walk(c)
			}
		case *proto.APLValue_Not:
			walk(x.Not.Val)
		case *proto.APLValue_Cmp:
			walk(x.Cmp.Lhs)
			walk(x.Cmp.Rhs)
		case *proto.APLValue_Math:
			walk(x.Math.Lhs)
			walk(x.Math.Rhs)
		}
	}
	for _, item := range items {
		walk(item.Action.GetCondition())
	}
	return n
}

func runWarriorDps(rotation *proto.APLRotation) float64 {
	result := core.RunRaidSim(&proto.RaidSimRequest{
		Raid: core.SinglePlayerRaidProto(&proto.Player{
			Class:         proto.Class_ClassWarrior,
			Race:          proto.Race_RaceOrc,
			Equipment:     core.GetGearSet("../../../ui/warrior/gear_sets", "p0.bis").GearSet,
			TalentsString: P1Talents,
			Spec:          PlayerOptionsFury,
			Consumes:      P1Consumes.Consumes,
			Rotation:      rotation,
		}, nil, nil, nil),
		Encounter:  core.MakeSingleTargetEncounter(0),
		SimOptions: &proto.SimOptions{Iterations: 50, IsTest: true, RandomSeed: 101, Ruleset: proto.Ruleset_RulesetForever},
	})
	if result.Error != nil {
		panic(result.Error.Message)
	}
	return result.RaidMetrics.Dps.Avg
}
