package hunter

import (
	"testing"

	"github.com/wowsims/forever/sim/core"
	"github.com/wowsims/forever/sim/core/proto"
	"github.com/wowsims/forever/sim/core/simsignals"
	"github.com/wowsims/forever/sim/core/stats"
)

func aspectSim(talents string) (*core.Simulation, *Hunter) {
	player := &proto.Player{
		Name: "sv", Class: proto.Class_ClassHunter, Race: proto.Race_RaceOrc, TalentsString: talents,
		Equipment: WeaponsOnly, Consumables: &proto.ConsumesSpec{}, DistanceFromTarget: core.MaxMeleeRange,
		Spec: &proto.Player_Hunter{Hunter: &proto.Hunter{Options: &proto.Hunter_Options{ClassOptions: &proto.HunterOptions{
			Ammo: proto.HunterOptions_Doomshot, QuiverBonus: proto.HunterOptions_Speed15, PetType: proto.HunterOptions_Cat,
			PetAttackSpeed: proto.HunterOptions_OneTwo, PetUptime: 1}}}},
		Rotation: &proto.APLRotation{Type: proto.APLRotation_TypeAPL},
	}
	sim := core.NewSim(&proto.RaidSimRequest{
		SimOptions: &proto.SimOptions{RandomSeed: 1},
		Raid:       core.SinglePlayerRaidProto(player, &proto.PartyBuffs{}, &proto.RaidBuffs{}, &proto.Debuffs{}),
		Encounter:  core.MakeSingleTargetEncounter(0),
	}, simsignals.CreateSignals())
	sim.Reset()
	return sim, sim.Raid.Parties[0].Players[0].(HunterAgent).GetHunter()
}

// Aspect of the Beast rank 4 (1299447) is 110 melee attack power, and it and Aspect of the Hawk
// replace each other.
func TestAspectOfTheBeastMeleeAttackPower(t *testing.T) {
	sim, hunter := aspectSim(SurvivalTalents)
	ap, rap := hunter.GetStat(stats.AttackPower), hunter.GetStat(stats.RangedAttackPower)

	hunter.AspectOfTheBeastAura.Activate(sim)
	if got := hunter.GetStat(stats.AttackPower) - ap; got != 110 {
		t.Errorf("Aspect of the Beast adds %v attack power, want 110", got)
	}
	if got := hunter.GetStat(stats.RangedAttackPower); got != rap {
		t.Errorf("Aspect of the Beast moves ranged attack power from %v to %v", rap, got)
	}

	hunter.AspectOfTheHawkAura.Activate(sim)
	if hunter.AspectOfTheBeastAura.IsActive() {
		t.Error("Aspect of the Hawk left Aspect of the Beast up")
	}
	if got := hunter.GetStat(stats.AttackPower); got != ap {
		t.Errorf("attack power is %v after swapping to Hawk, want %v", got, ap)
	}
}

// Beast states no proc chance of its own: Quick Strikes (1299448) comes only with Deadly Aspects.
func TestQuickStrikesNeedsDeadlyAspects(t *testing.T) {
	_, without := aspectSim(SurvivalTalents)
	if without.GetAura("Quick Strikes") != nil {
		t.Error("Quick Strikes registered without Deadly Aspects")
	}
	_, with := aspectSim("50230005041--510230031050220151")
	if with.GetAura("Quick Strikes") == nil {
		t.Error("Quick Strikes missing with 5/5 Deadly Aspects")
	}
}
