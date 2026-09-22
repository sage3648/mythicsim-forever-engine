package mage

import (
	"testing"
	"time"

	"github.com/wowsims/classic/sim/core"
	"github.com/wowsims/classic/sim/core/proto"
	"github.com/wowsims/classic/sim/core/simsignals"
)

// Clearcasting zeroes every mage spell's cost, so it must not decide "was this a paid cast" from
// the current cost: that made the proc never be consumed and always run its full 15s.
func TestClearcastingIsConsumedByTheNextCast(t *testing.T) {
	sim := core.NewSim(&proto.RaidSimRequest{
		SimOptions: &proto.SimOptions{RandomSeed: 1, Ruleset: proto.Ruleset_RulesetForever},
		Raid: core.SinglePlayerRaidProto(&proto.Player{
			Class:         proto.Class_ClassMage,
			Race:          proto.Race_RaceTroll,
			Equipment:     core.GetGearSet("../../ui/mage/gear_sets", "p0.bis").GearSet,
			TalentsString: ForeverFrostTalents,
			Rotation:      core.GetAplRotation("../../ui/mage/apls", "forever_frost").Rotation,
			Spec:          PlayerOptions,
		}, nil, nil, nil),
		Encounter: core.MakeSingleTargetEncounter(0),
	}, simsignals.CreateSignals())
	sim.Reset()

	mage := sim.Raid.Parties[0].Players[0].(MageAgent).GetMage()
	if mage.ClearcastingAura == nil {
		t.Fatal("talents have no Arcane Concentration")
	}
	fireBlast := mage.FireBlast[len(mage.FireBlast)-1]

	mage.ClearcastingAura.Activate(sim)
	sim.CurrentTime += time.Second
	mage.SetGCDTimer(sim, sim.CurrentTime)
	fireBlast.Cast(sim, mage.CurrentTarget)

	if mage.ClearcastingAura.IsActive() {
		t.Error("Clearcasting is still active after a Fire Blast")
	}
}
