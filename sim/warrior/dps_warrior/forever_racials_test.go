package dpswarrior

import (
	"testing"
	"time"

	"github.com/wowsims/classic/sim/core"
	"github.com/wowsims/classic/sim/core/proto"
	googleProto "google.golang.org/protobuf/proto"
)

// Checks the Forever racial cooldowns against client build 1.60.1.69977. Players on the
// MythicSim Discord caught the first two from the race comparison page: Windshaper was
// given a copy of Blood Fury that no Skyborne has, and Blood Fury cost a global cooldown.

func foreverWarrior(race proto.Race) *proto.Player {
	return &proto.Player{
		Class:         proto.Class_ClassWarrior,
		Race:          race,
		Equipment:     core.GetGearSet("../../../ui/warrior/gear_sets", "p0.bis").GearSet,
		TalentsString: P1Talents,
		Rotation:      core.GetAplRotation("../../../ui/warrior/apls", "dps_reck").Rotation,
		Consumes:      P1Consumes.Consumes,
		Spec:          PlayerOptionsFury,
	}
}

func foreverWarriorCharacter(t *testing.T, race proto.Race) *core.Character {
	t.Helper()
	raid := core.SinglePlayerRaidProto(foreverWarrior(race), nil, core.ForeverBuffs.Raid, core.ForeverBuffs.Debuffs)
	env, _, _ := core.NewEnvironment(raid, core.MakeSingleTargetEncounter(0), proto.Ruleset_RulesetForever, false)
	return env.Raid.Parties[0].Players[0].GetCharacter()
}

// Both Skyborne halves share one racial skill line (2980): Wind Blessed, Elemental Insight,
// Read Ley Line, Walk on Air and Skysight. Neither has a damage cooldown, and their base
// stats are the same, so on one seed they must sim to the same DPS.
func TestSkyborneHalvesSimAlike(t *testing.T) {
	simOptions := googleProto.Clone(core.DefaultSimTestOptions).(*proto.SimOptions)
	simOptions.Ruleset = proto.Ruleset_RulesetForever

	dps := map[proto.Race]float64{}
	for _, race := range []proto.Race{proto.Race_RaceSkyborneHighOrder, proto.Race_RaceSkyborneWindshaper} {
		result := core.RunRaidSim(&proto.RaidSimRequest{
			Raid:       core.SinglePlayerRaidProto(foreverWarrior(race), nil, core.ForeverBuffs.Raid, core.ForeverBuffs.Debuffs),
			Encounter:  core.MakeSingleTargetEncounter(0),
			SimOptions: simOptions,
		})
		if result.Error != nil {
			t.Fatalf("%v: sim failed: %s", race, result.Error.Message)
		}
		dps[race] = result.RaidMetrics.Dps.Avg
	}

	if dps[proto.Race_RaceSkyborneHighOrder] != dps[proto.Race_RaceSkyborneWindshaper] {
		t.Errorf("High Order sims %.3f DPS and Windshaper %.3f; the halves share every combat racial",
			dps[proto.Race_RaceSkyborneHighOrder], dps[proto.Race_RaceSkyborneWindshaper])
	}
}

// Blood Fury (20572) has no start recovery category or time in the client, so it is off
// the global cooldown.
func TestForeverBloodFuryIsOffTheGlobalCooldown(t *testing.T) {
	bloodFury := foreverWarriorCharacter(t, proto.Race_RaceOrc).GetSpell(core.ActionID{SpellID: 20572})
	if bloodFury == nil {
		t.Fatal("an orc warrior has no Blood Fury")
	}
	if bloodFury.DefaultCast.GCD != 0 {
		t.Errorf("Blood Fury triggers a %v global cooldown", bloodFury.DefaultCast.GCD)
	}
}

// The warrior's Eureka! (1259813) lasts 15 seconds with three charges.
func TestForeverEurekaDurationAndCharges(t *testing.T) {
	eureka := foreverWarriorCharacter(t, proto.Race_RaceGnome).GetAuraByID(core.ActionID{SpellID: 1259813})
	if eureka == nil {
		t.Fatal("a gnome warrior has no Eureka!")
	}
	if eureka.Duration != 15*time.Second {
		t.Errorf("Eureka! lasts %v, want 15s", eureka.Duration)
	}
	if eureka.MaxStacks != 3 {
		t.Errorf("Eureka! has %d charges, want 3", eureka.MaxStacks)
	}
}

// DisableRacials measures what a race's racials are worth: the same character, base
// stats included, with none of its racial effects.
func TestDisableRacialsDropsRacialsButKeepsBaseStats(t *testing.T) {
	player := foreverWarrior(proto.Race_RaceOrc)
	player.DisableRacials = true
	raid := core.SinglePlayerRaidProto(player, nil, core.ForeverBuffs.Raid, core.ForeverBuffs.Debuffs)
	env, _, _ := core.NewEnvironment(raid, core.MakeSingleTargetEncounter(0), proto.Ruleset_RulesetForever, false)
	orc := env.Raid.Parties[0].Players[0].GetCharacter()
	if orc.GetSpell(core.ActionID{SpellID: 20572}) != nil {
		t.Error("an orc with racials disabled still has Blood Fury")
	}

	withRacials := foreverWarriorCharacter(t, proto.Race_RaceOrc)
	if got, want := orc.GetBaseStats(), withRacials.GetBaseStats(); got != want {
		t.Errorf("disabling racials changed the orc's base stats: %v, want %v", got, want)
	}
}
