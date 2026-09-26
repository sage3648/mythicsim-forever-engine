package core

import (
	"strings"
	"testing"

	"github.com/wowsims/forever/sim/core/proto"
)

// A rotation whose only priority list action casts a spell the warrior does not know.
func unknownSpellRequest() *proto.RaidSimRequest {
	return &proto.RaidSimRequest{
		SimOptions: &proto.SimOptions{Iterations: 4, RandomSeed: 100},
		Raid: &proto.Raid{
			Parties: []*proto.Party{
				{
					Players: []*proto.Player{
						{
							Name:      "Warrior",
							Class:     proto.Class_ClassWarrior,
							Race:      proto.Race_RaceOrc,
							Buffs:     &proto.IndividualBuffs{},
							Spec:      &proto.Player_DpsWarrior{},
							Equipment: &proto.EquipmentSpec{},
							Rotation: &proto.APLRotation{
								Type: proto.APLRotation_TypeAPL,
								PriorityList: []*proto.APLListItem{
									{Action: &proto.APLAction{Action: &proto.APLAction_CastSpell{
										CastSpell: &proto.APLActionCastSpell{
											SpellId: &proto.ActionID{RawId: &proto.ActionID_SpellId{SpellId: 999999}},
										},
									}}},
								},
							},
						},
					},
					Buffs: &proto.PartyBuffs{},
				},
			},
		},
		Encounter: &proto.Encounter{
			Targets: []*proto.Target{
				{Name: "target", Level: 63, MobType: proto.MobType_MobTypeElemental},
			},
			Duration: 30,
		},
	}
}

func TestRaidSimResultCarriesRotationValidations(t *testing.T) {
	result := RunRaidSim(unknownSpellRequest())
	if result.Error != nil {
		t.Fatalf("sim failed: %s", result.Error.Message)
	}

	stats := result.RaidMetrics.Parties[0].Players[0].RotationStats
	if stats == nil {
		t.Fatal("the player's metrics have no rotation stats")
	}
	if len(stats.PriorityList) != 1 {
		t.Fatalf("%d priority list entries, want 1", len(stats.PriorityList))
	}
	validations := stats.PriorityList[0].Validations
	if len(validations) != 1 {
		t.Fatalf("%d validations on the unknown spell, want 1: %v", len(validations), validations)
	}
	if validations[0].LogLevel != proto.LogLevel_Warning || !strings.Contains(validations[0].Validation, "does not know spell") {
		t.Errorf("validation %v, want a warning that the warrior does not know the spell", validations[0])
	}
}

func TestCombinedResultsKeepRotationValidations(t *testing.T) {
	first := RunRaidSim(unknownSpellRequest())
	second := RunRaidSim(unknownSpellRequest())

	combined := CombineConcurrentSimResults([]*proto.RaidSimResult{first, second}, false)
	stats := combined.RaidMetrics.Parties[0].Players[0].RotationStats
	if stats == nil || len(stats.PriorityList) != 1 || len(stats.PriorityList[0].Validations) != 1 {
		t.Fatalf("combined rotation stats %v, want the one unknown spell warning", stats)
	}
}

func TestResultStatsCollapsesRepeatedValidations(t *testing.T) {
	repeated := &proto.APLValidation{LogLevel: proto.LogLevel_Warning, Validation: "could not cast"}
	other := &proto.APLValidation{LogLevel: proto.LogLevel_Information, Validation: "could not cast"}
	rot := &APLRotation{
		prepullValidations: [][]*proto.APLValidation{{repeated, repeated, other, repeated}},
		uuidValidations:    map[*proto.UUID][]*proto.APLValidation{},
	}

	stats := rot.resultStats()
	if got := len(stats.PrepullActions[0].Validations); got != 2 {
		t.Fatalf("%d prepull validations, want 2 (one per log level)", got)
	}
	if rot.resultStats() != stats {
		t.Error("a second call built the stats again")
	}
}
