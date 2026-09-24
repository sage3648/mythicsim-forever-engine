package dps

import (
	"testing"

	"github.com/wowsims/forever/sim/core"
	"github.com/wowsims/forever/sim/core/proto"
	"github.com/wowsims/forever/sim/core/simsignals"
	"github.com/wowsims/forever/sim/core/stats"
)

// The Forever racials against a real warrior. sim/core/racials_test.go checks each racial's
// figures on a stand-in; these check that every race builds and sims, which is what went
// missing twice on the previous engine line (the undead had no Forever racial at all, and the
// Skyborne carried theirs where they should not have) because the DPS suites run one or two
// races per spec and nothing built the rest.

func foreverWarrior(race proto.Race, gear *proto.EquipmentSpec) *proto.Player {
	return core.WithSpec(&proto.Player{
		Name:          "Warrior",
		Race:          race,
		Class:         proto.Class_ClassWarrior,
		Equipment:     gear,
		Consumables:   DefaultConsumables,
		TalentsString: DpsTalents,
		Rotation:      core.GetAplRotation("../../../ui/specs/warrior/dps/apls", "dps_reck").Rotation,
	}, DefaultOptions)
}

func foreverWarriorDps(t *testing.T, player *proto.Player) float64 {
	t.Helper()
	result := core.RunRaidSim(&proto.RaidSimRequest{
		Raid:       core.SinglePlayerRaidProto(player, &proto.PartyBuffs{}, &proto.RaidBuffs{}, &proto.Debuffs{}),
		Encounter:  core.MakeSingleTargetEncounter(0),
		SimOptions: &proto.SimOptions{Iterations: 20, RandomSeed: 101},
	})
	if result.Error != nil {
		t.Fatalf("%v: sim failed: %s", player.Race, result.Error.Message)
	}
	return result.RaidMetrics.Dps.Avg
}

func foreverWarriorCharacter(player *proto.Player) *core.Character {
	sim := core.NewSim(&proto.RaidSimRequest{
		Raid:       core.SinglePlayerRaidProto(player, &proto.PartyBuffs{}, &proto.RaidBuffs{}, &proto.Debuffs{}),
		Encounter:  core.MakeSingleTargetEncounter(0),
		SimOptions: &proto.SimOptions{RandomSeed: 101},
	}, simsignals.CreateSignals())
	sim.Reset()
	return sim.Raid.Parties[0].Players[0].GetCharacter()
}

// Both Skyborne halves share one racial skill line (2980) and the same base stats, so on one
// seed they must sim to the same DPS.
func TestSkyborneHalvesSimAlike(t *testing.T) {
	highOrder := foreverWarriorDps(t, foreverWarrior(proto.Race_RaceSkyborneHighOrder, DualWieldGear.GearSet))
	windshaper := foreverWarriorDps(t, foreverWarrior(proto.Race_RaceSkyborneWindshaper, DualWieldGear.GearSet))
	if highOrder != windshaper {
		t.Errorf("High Order sims %.3f DPS and Windshaper %.3f; the halves share every combat racial", highOrder, windshaper)
	}
}

// Every race the warrior can be builds and sims, the Skyborne included.
func TestEveryWarriorRaceSims(t *testing.T) {
	races := core.ClassRaceCapabilities[proto.Class_ClassWarrior]
	if len(races) < 11 {
		t.Errorf("expected every race but the blood elf to be warrior-eligible, got %d", len(races))
	}
	for _, race := range races {
		t.Run(race.String(), func(t *testing.T) {
			defer func() {
				if r := recover(); r != nil {
					t.Errorf("simming a %v warrior panicked: %v", race, r)
				}
			}()
			if dps := foreverWarriorDps(t, foreverWarrior(race, DualWieldGear.GearSet)); dps <= 0 {
				t.Errorf("a %v warrior does %.1f DPS", race, dps)
			}
		})
	}
}

// The weapon skill racials pay critical strike in both pools while the weapon is held: 1%
// for an orc with an axe in either hand, 2% for a human with a sword.
func TestForeverWeaponSpecializationsPayCrit(t *testing.T) {
	for _, tc := range []struct {
		race       proto.Race
		gear       *proto.EquipmentSpec
		weaponType proto.WeaponType
		crit       float64
	}{
		{proto.Race_RaceOrc, DualWieldGear.GearSet, proto.WeaponType_WeaponTypeAxe, 1},
		{proto.Race_RaceHuman, weaponsOnly(12940, 0), proto.WeaponType_WeaponTypeSword, 2}, // Dal'Rend's Sacred Charge
		// The test gear is all axes, so a human holding it gets nothing.
		{proto.Race_RaceHuman, DualWieldGear.GearSet, proto.WeaponType_WeaponTypeAxe, 0},
	} {
		with := foreverWarriorCharacter(foreverWarrior(tc.race, tc.gear))
		player := foreverWarrior(tc.race, tc.gear)
		player.DisableRacials = true
		without := foreverWarriorCharacter(player)

		if with.MainHand().WeaponType != tc.weaponType && with.OffHand().WeaponType != tc.weaponType {
			t.Fatalf("the %v test gear holds no %v, so this checks nothing", tc.race, tc.weaponType)
		}
		for _, stat := range []stats.Stat{stats.PhysicalCritPercent, stats.SpellCritPercent} {
			if got := with.GetStat(stat) - without.GetStat(stat); !core.WithinToleranceFloat64(tc.crit, got, 1e-9) {
				t.Errorf("%v with a %v: %s +%.2f, want +%.0f", tc.race, tc.weaponType, stat.StatName(), got, tc.crit)
			}
		}
		if got := with.GetStat(stats.ExpertiseRating) - without.GetStat(stats.ExpertiseRating); got != 0 {
			t.Errorf("%v with a %v still gains %.1f expertise rating from its racial", tc.race, tc.weaponType, got)
		}
	}
}
