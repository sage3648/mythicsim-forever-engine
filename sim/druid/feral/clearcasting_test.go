package feral

import (
	"testing"
	"time"

	"github.com/wowsims/classic/sim/core"
	"github.com/wowsims/classic/sim/core/proto"
	"github.com/wowsims/classic/sim/core/simsignals"
	"github.com/wowsims/classic/sim/druid"
)

// Clearcasting (16870, one charge) makes the next ability in its mask free and is spent by it. A
// spell outside the mask pays in full and leaves it up.
func TestClearcastingSpentByNextCostedAbility(t *testing.T) {
	sim := core.NewSim(&proto.RaidSimRequest{
		SimOptions: &proto.SimOptions{RandomSeed: 1, Ruleset: proto.Ruleset_RulesetForever},
		Raid: core.SinglePlayerRaidProto(&proto.Player{
			Class:         proto.Class_ClassDruid,
			Race:          proto.Race_RaceTauren,
			Equipment:     core.GetGearSet("../../../ui/feral_druid/gear_sets", "p0.bis").GearSet,
			TalentsString: P1Talents,
			Rotation:      &proto.APLRotation{Type: proto.APLRotation_TypeAPL},
			Spec:          PlayerOptionsMonoCat,
		}, nil, nil, nil),
		Encounter: core.MakeSingleTargetEncounter(0),
	}, simsignals.CreateSignals())
	sim.Reset()

	cat := sim.Raid.Parties[0].Players[0].(druid.DruidAgent).GetDruid()
	if cat.ClearcastingAura == nil {
		t.Fatal("no Clearcasting aura: Omen of Clarity is not registered")
	}
	target := cat.CurrentTarget

	// Move past the global cooldown by hand: stepping the sim would let the agent's own rotation act.
	waitForGCD := func() {
		sim.CurrentTime += 2 * time.Second
		cat.SetGCDTimer(sim, sim.CurrentTime)
	}

	cat.ClearcastingAura.Activate(sim)

	// Faerie Fire is castable in Cat Form and is not in Clearcasting's mask.
	mana := cat.CurrentMana()
	if !cat.FaerieFire.Cast(sim, target) {
		t.Fatal("Faerie Fire did not cast")
	}
	if cat.CurrentMana() >= mana {
		t.Error("Faerie Fire, outside the mask, cast for free under Clearcasting")
	}
	if !cat.ClearcastingAura.IsActive() {
		t.Fatal("Faerie Fire, outside the mask, spent Clearcasting")
	}

	waitForGCD()
	energy := cat.CurrentEnergy()
	if !cat.Shred.Cast(sim, target) {
		t.Fatal("Shred did not cast")
	}
	if spent := energy - cat.CurrentEnergy(); spent > 0 {
		t.Errorf("Shred cost %v Energy under Clearcasting, want 0", spent)
	}
	if cat.ClearcastingAura.IsActive() && cat.ClearcastingAura.RemainingDuration(sim) != cat.ClearcastingAura.Duration {
		t.Fatal("Shred did not spend Clearcasting")
	}

	// With the charge spent the next Shred pays, unless its first hit granted a new one.
	wasActive := cat.ClearcastingAura.IsActive()
	waitForGCD()
	energy = cat.CurrentEnergy()
	if !cat.Shred.Cast(sim, target) {
		t.Fatal("second Shred did not cast")
	}
	if !wasActive && cat.CurrentEnergy() >= energy {
		t.Error("the Shred after Clearcasting was spent cast for free")
	}
}

// Over a fight Omen of Clarity grants Clearcasting and the rotation spends it.
func TestOmenOfClarityProcs(t *testing.T) {
	result := core.RunRaidSim(&proto.RaidSimRequest{
		Raid: core.SinglePlayerRaidProto(&proto.Player{
			Class:         proto.Class_ClassDruid,
			Race:          proto.Race_RaceTauren,
			Equipment:     core.GetGearSet("../../../ui/feral_druid/gear_sets", "p0.bis").GearSet,
			TalentsString: P1Talents,
			Rotation:      core.GetAplRotation("../../../ui/feral_druid/apls", "p1").Rotation,
			Spec:          PlayerOptionsMonoCat,
		}, nil, nil, nil),
		Encounter:  core.MakeSingleTargetEncounter(0),
		SimOptions: &proto.SimOptions{Iterations: 10, RandomSeed: 1, Ruleset: proto.Ruleset_RulesetForever},
	})
	if result.Error != nil {
		t.Fatal(result.Error.Message)
	}

	for _, aura := range result.RaidMetrics.Parties[0].Players[0].Auras {
		if aura.Id.GetSpellId() == 16870 {
			// The 10 sec cooldown allows at most 18 over the default 3 minute fight.
			if aura.ProcsAvg < 1 || aura.ProcsAvg > 18 {
				t.Errorf("Clearcasting procced %.2f times a fight, want 1-18", aura.ProcsAvg)
			}
			t.Logf("Clearcasting: %.2f procs, %.1f s uptime a fight", aura.ProcsAvg, aura.UptimeSecondsAvg)
			return
		}
	}
	t.Error("Clearcasting never procced")
}
