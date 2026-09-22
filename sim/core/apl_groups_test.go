package core

import (
	"testing"

	"github.com/wowsims/classic/sim/core/proto"
)

func aplConst(val string) *proto.APLValue {
	return &proto.APLValue{Value: &proto.APLValue_Const{Const: &proto.APLValueConst{Val: val}}}
}
func aplVarRef(name string) *proto.APLValue {
	return &proto.APLValue{Value: &proto.APLValue_VariableRef{VariableRef: &proto.APLValueVariableRef{Name: name}}}
}
func aplPlaceholder(name string) *proto.APLValue {
	return &proto.APLValue{Value: &proto.APLValue_VariablePlaceholder{VariablePlaceholder: &proto.APLValueVariablePlaceholder{Name: name}}}
}
func aplGroupUsed(name string) *proto.APLValue {
	return &proto.APLValue{Value: &proto.APLValue_ActionGroupUsed{ActionGroupUsed: &proto.APLValueActionGroupUsed{Name: name}}}
}
func aplAdd(lhs, rhs *proto.APLValue) *proto.APLValue {
	return &proto.APLValue{Value: &proto.APLValue_Math{Math: &proto.APLValueMath{Op: proto.APLValueMath_OpAdd, Lhs: lhs, Rhs: rhs}}}
}
func aplGt(lhs, rhs *proto.APLValue) *proto.APLValue {
	return &proto.APLValue{Value: &proto.APLValue_Cmp{Cmp: &proto.APLValueCompare{Op: proto.APLValueCompare_OpGt, Lhs: lhs, Rhs: rhs}}}
}

// A wait action gated by condition, so tests can see which group entry is ready.
func aplWaitIf(cond *proto.APLValue) *proto.APLAction {
	return &proto.APLAction{
		Condition: cond,
		Action:    &proto.APLAction_Wait{Wait: &proto.APLActionWait{Duration: aplConst("1s")}},
	}
}
func aplGroupRef(name string, vars ...*proto.APLValueVariable) *proto.APLAction {
	return &proto.APLAction{Action: &proto.APLAction_GroupReference{GroupReference: &proto.APLActionGroupReference{GroupName: name, Variables: vars}}}
}

// Builds a rotation on a bare target unit, which is enough for const/math/wait APLs.
func newTestAPLRotation(config *proto.APLRotation) *APLRotation {
	target := &Target{}
	env := &Environment{Raid: &Raid{}}
	env.Encounter.Targets = []*Target{target}
	target.Unit.Env = env
	return target.Unit.newAPLRotation(config)
}

func TestAPLValueVariables(t *testing.T) {
	sim := &Simulation{}
	rot := newTestAPLRotation(&proto.APLRotation{
		ValueVariables: []*proto.APLValueVariable{
			{Name: "x", Value: aplConst("5")},
			{Name: "y", Value: aplAdd(aplVarRef("x"), aplConst("1"))},
			{Name: "loop", Value: aplAdd(aplVarRef("loop"), aplConst("1"))},
		},
	})

	if got := rot.newAPLValue(aplVarRef("y")).GetInt(sim); got != 6 {
		t.Fatalf("y = %d, want 6", got)
	}
	if rot.newAPLValue(aplVarRef("missing")) != nil || len(rot.curWarnings) != 1 {
		t.Fatalf("missing variable should be nil with a warning, got %v", rot.curWarnings)
	}
	rot.curWarnings = nil
	if rot.newAPLValue(aplVarRef("loop")) != nil || len(rot.curWarnings) == 0 {
		t.Fatalf("self-referencing variable should be nil with a warning")
	}
	rot.curWarnings = nil
	// Placeholders only resolve inside a group reference.
	if rot.newAPLValue(aplPlaceholder("x")) != nil || len(rot.curWarnings) != 1 {
		t.Fatalf("unfilled placeholder should be nil with a warning, got %v", rot.curWarnings)
	}
}

func TestAPLActionGroups(t *testing.T) {
	sim := &Simulation{}
	rot := newTestAPLRotation(&proto.APLRotation{
		ValueVariables: []*proto.APLValueVariable{
			{Name: "limit", Value: aplConst("10")},
		},
		Groups: []*proto.APLGroup{
			{
				Name:      "g",
				Variables: []*proto.APLValueVariable{{Name: "bonus", Value: aplConst("100")}},
				Actions: []*proto.APLListItem{
					// Ready when p > limit (global variable).
					{Action: aplWaitIf(aplGt(aplPlaceholder("p"), aplVarRef("limit")))},
					// Ready when p + bonus > 150 (group variable, overridable per reference).
					{Action: aplWaitIf(aplGt(aplAdd(aplPlaceholder("p"), aplVarRef("bonus")), aplConst("150")))},
					{Hide: true, Action: aplWaitIf(aplConst("true"))},
				},
			},
			{Name: "self", Actions: []*proto.APLListItem{{Action: aplGroupRef("self")}}},
			{Name: "unused", Actions: []*proto.APLListItem{{Action: aplWaitIf(aplConst("true"))}}},
		},
		PriorityList: []*proto.APLListItem{
			{Action: aplGroupRef("g", &proto.APLValueVariable{Name: "p", Value: aplConst("60")})},
			// Same group, different binding: each reference gets its own copy.
			{Action: aplGroupRef("g", &proto.APLValueVariable{Name: "p", Value: aplConst("60")}, &proto.APLValueVariable{Name: "bonus", Value: aplConst("0")})},
			{Action: aplGroupRef("g")},       // placeholder not filled
			{Action: aplGroupRef("self")},    // recursion
			{Action: aplGroupRef("nothere")}, // unknown group
			{Hide: true, Action: aplGroupRef("unused")},
		},
	})

	if len(rot.priorityList) != 2 {
		t.Fatalf("expected 2 valid group references, got %d; warnings %v", len(rot.priorityList), rot.priorityListWarnings)
	}
	for i := 2; i < 5; i++ {
		if len(rot.priorityListWarnings[i]) == 0 {
			t.Fatalf("priority list item %d should have a warning", i)
		}
	}

	condsReady := func(ref *APLAction) []bool {
		group := ref.impl.(*APLActionGroupReference)
		return MapSlice(group.actions, func(a *APLAction) bool { return a.condition.GetBool(sim) })
	}
	if got := condsReady(rot.priorityList[0]); len(got) != 2 || !got[0] || !got[1] {
		t.Fatalf("p=60 bonus=100: got %v, want [true true]", got)
	}
	if got := condsReady(rot.priorityList[1]); len(got) != 2 || !got[0] || got[1] {
		t.Fatalf("p=60 bonus=0: got %v, want [true false]", got)
	}

	// Inner actions are visible to reset/MCD handling.
	if n := len(rot.allAPLActions()); n != 6 {
		t.Fatalf("allAPLActions = %d, want 6 (2 refs + 4 inner)", n)
	}

	if !rot.newAPLValue(aplGroupUsed("g")).GetBool(sim) {
		t.Fatalf("group g should be used")
	}
	if rot.newAPLValue(aplGroupUsed("unused")).GetBool(sim) {
		t.Fatalf("group unused is only referenced from a hidden item")
	}
}
