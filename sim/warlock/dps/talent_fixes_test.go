package dps

import (
	"math"
	"testing"
	"time"

	"github.com/wowsims/classic/sim/core"
	"github.com/wowsims/classic/sim/core/proto"
	"github.com/wowsims/classic/sim/core/simsignals"
	"github.com/wowsims/classic/sim/core/stats"
	"github.com/wowsims/classic/sim/warlock"
)

func newTestWarlock(t *testing.T, talents string, spec *proto.Player_Warlock) (*core.Simulation, *warlock.Warlock) {
	t.Helper()
	sim := core.NewSim(&proto.RaidSimRequest{
		SimOptions: &proto.SimOptions{RandomSeed: 1, Ruleset: proto.Ruleset_RulesetForever},
		Raid: core.SinglePlayerRaidProto(&proto.Player{
			Class:         proto.Class_ClassWarlock,
			Race:          proto.Race_RaceOrc,
			Equipment:     core.GetGearSet("../../../ui/warlock/gear_sets", "prebis").GearSet,
			TalentsString: talents,
			Rotation:      core.GetAplRotation("../../../ui/warlock/apls/", "forever_pact").Rotation,
			Spec:          spec,
		}, nil, nil, nil),
		Encounter: core.MakeSingleTargetEncounter(0),
	}, simsignals.CreateSignals())
	sim.Reset()

	return sim, sim.Raid.Parties[0].Players[0].(*DpsWarlock).Warlock
}

func near(a, b float64) bool {
	return math.Abs(a-b) < 1e-6
}

// Life Tap restores (424 + Spirit) * (1 + Improved Life Tap) mana (11689): Soul Link, Master
// Demonologist and the other damage modifiers the Demonic Pact build carries do not touch it.
func TestLifeTapIsAManaGainNotADamageRoll(t *testing.T) {
	sim, wl := newTestWarlock(t, TalentsDemonicPact, DefaultPactWarlock)
	if wl.Talents.ImprovedLifeTap == 0 || !wl.Talents.SoulLink {
		t.Fatal("test talents need Improved Life Tap and Soul Link")
	}
	wl.SoulLinkAura.Activate(sim)

	wl.SpendMana(sim, wl.CurrentMana(), wl.NewManaMetrics(core.ActionID{SpellID: 1}))
	health := wl.CurrentHealth()

	sim.CurrentTime += time.Second
	wl.SetGCDTimer(sim, sim.CurrentTime)
	lifeTap := wl.LifeTap[len(wl.LifeTap)-1]
	lifeTap.Cast(sim, wl.CurrentTarget)

	want := (424 + wl.GetStat(stats.Spirit)) * (1 + 0.1*float64(wl.Talents.ImprovedLifeTap))
	if got := wl.CurrentMana(); !near(got, want) {
		t.Errorf("Life Tap restored %.3f mana, want %.3f", got, want)
	}
	if wl.CurrentHealth() != health {
		t.Errorf("a non-tanking warlock's health moved from %v to %v", health, wl.CurrentHealth())
	}
}
