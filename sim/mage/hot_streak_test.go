package mage

import (
	"testing"
	"time"

	"github.com/wowsims/classic/sim/core"
	"github.com/wowsims/classic/sim/core/proto"
	"github.com/wowsims/classic/sim/core/simsignals"
)

func newHotStreakTestMage(t *testing.T) (*core.Simulation, *Mage, *core.Spell) {
	t.Helper()

	sim := core.NewSim(&proto.RaidSimRequest{
		SimOptions: &proto.SimOptions{RandomSeed: 1, Ruleset: proto.Ruleset_RulesetForever},
		Raid: core.SinglePlayerRaidProto(&proto.Player{
			Class:         proto.Class_ClassMage,
			Race:          proto.Race_RaceGnome,
			Equipment:     &proto.EquipmentSpec{},
			TalentsString: ForeverFireTalents,
			// No actions, so nothing but the test's own casts touches Hot Streak.
			Rotation: &proto.APLRotation{Type: proto.APLRotation_TypeAPL},
			Spec:     PlayerOptions,
		}, nil, nil, nil),
		Encounter: core.MakeSingleTargetEncounter(0),
	}, simsignals.CreateSignals())
	sim.Reset()

	mage := sim.Raid.Parties[0].Players[0].(MageAgent).GetMage()
	if mage.HotStreakAura == nil || len(mage.Pyroblast) == 0 || mage.Pyroblast[PyroblastRanks] == nil {
		t.Fatal("talents have no Hot Streak or Pyroblast")
	}
	return sim, mage, mage.Pyroblast[PyroblastRanks]
}

// Runs the sim until the cast in progress has completed.
func finishHardcast(t *testing.T, sim *core.Simulation, mage *Mage) {
	t.Helper()
	for mage.Hardcast.Expires > 0 && sim.CurrentTime < 20*time.Second {
		sim.Step()
	}
	if mage.Hardcast.Expires > 0 {
		t.Fatalf("cast still running at %v", sim.CurrentTime)
	}
}

// Hot Streak (400625) has one charge: the Pyroblast its stacks speed up spends all of them.
func TestHotStreakSpentByPyroblast(t *testing.T) {
	sim, mage, pyroblast := newHotStreakTestMage(t)

	mage.HotStreakAura.Activate(sim)
	mage.HotStreakAura.SetStacks(sim, 3)
	if !pyroblast.Cast(sim, mage.CurrentTarget) {
		t.Fatal("Pyroblast did not cast")
	}
	if want := pyroblast.DefaultCast.CastTime / 4; mage.Hardcast.Expires != want {
		t.Errorf("Pyroblast cast ends at %v, want %v with 3 stacks", mage.Hardcast.Expires, want)
	}

	finishHardcast(t, sim, mage)
	if mage.HotStreakAura.IsActive() {
		t.Errorf("Hot Streak still up with %d stacks at %v after Pyroblast finished", mage.HotStreakAura.GetStacks(), sim.CurrentTime)
	}
}

// A Pyroblast that was already being cast when the first stack landed was not sped up by it, so the
// stack waits for the next Pyroblast, which it speeds up and which spends it.
func TestHotStreakKeptByPyroblastStartedWithout(t *testing.T) {
	sim, mage, pyroblast := newHotStreakTestMage(t)

	if !pyroblast.Cast(sim, mage.CurrentTarget) {
		t.Fatal("Pyroblast did not cast")
	}
	for sim.CurrentTime < time.Second {
		sim.Step()
	}
	if !mage.IsCasting(sim) {
		t.Fatalf("Pyroblast no longer casting at %v", sim.CurrentTime)
	}
	mage.HotStreakAura.Activate(sim)
	mage.HotStreakAura.AddStack(sim)

	finishHardcast(t, sim, mage)
	if !mage.HotStreakAura.IsActive() || mage.HotStreakAura.GetStacks() != 1 {
		t.Fatalf("Hot Streak spent by a Pyroblast it did not speed up (active %v, %d stacks)", mage.HotStreakAura.IsActive(), mage.HotStreakAura.GetStacks())
	}

	mage.SetGCDTimer(sim, sim.CurrentTime)
	start := sim.CurrentTime
	if !pyroblast.Cast(sim, mage.CurrentTarget) {
		t.Fatal("second Pyroblast did not cast")
	}
	if want := start + pyroblast.DefaultCast.CastTime*3/4; mage.Hardcast.Expires != want {
		t.Errorf("second Pyroblast cast ends at %v, want %v with 1 stack", mage.Hardcast.Expires, want)
	}

	finishHardcast(t, sim, mage)
	if mage.HotStreakAura.IsActive() {
		t.Errorf("Hot Streak still up after the Pyroblast it sped up finished")
	}
}
