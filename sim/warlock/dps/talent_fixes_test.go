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

// Affliction with Wrack (the tree's seventeenth talent) on top.
var talentsAfflictionWrack = "25350020135211051--05500051"

// Wrack's +10% (1316697 effect 2, mask 1026) reaches Corruption and Bane of Agony only, not the
// warlock's other shadow dots.
func TestWrackBoostsCorruptionAndAgonyOnly(t *testing.T) {
	sim, wl := newTestWarlock(t, talentsAfflictionWrack, DefaultDestroWarlock)
	if wl.Wrack == nil {
		t.Fatal("test talents have no Wrack")
	}
	target := wl.CurrentTarget

	damageTaken := func(spell *core.Spell) float64 {
		result := spell.NewResult(target)
		result.Damage = 1000
		spell.ApplyPostOutcomeDamageModifiers(sim, result)
		return result.Damage
	}
	spells := map[string]*core.Spell{
		"Corruption":    wl.Corruption[len(wl.Corruption)-1],
		"Bane of Agony": wl.BaneOfAgony[len(wl.BaneOfAgony)-1],
		"Siphon Life":   wl.SiphonLife[len(wl.SiphonLife)-1],
		"Drain Life":    wl.DrainLife[len(wl.DrainLife)-1],
	}
	before := map[string]float64{}
	for name, spell := range spells {
		before[name] = damageTaken(spell)
	}

	wl.Wrack.Dot(target).Activate(sim)
	for name, spell := range spells {
		want := before[name]
		if name == "Corruption" || name == "Bane of Agony" {
			want *= 1.1
		}
		if got := damageTaken(spell); !near(got, want) {
			t.Errorf("%s: %.3f with Wrack on the target, want %.3f", name, got, want)
		}
	}
}
