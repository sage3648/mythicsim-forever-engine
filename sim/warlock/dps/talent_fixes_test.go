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
	return newTestWarlockWithGear(t, talents, spec, core.GetGearSet("../../../ui/warlock/gear_sets", "prebis").GearSet)
}

func newTestWarlockWithGear(t *testing.T, talents string, spec *proto.Player_Warlock, gear *proto.EquipmentSpec) (*core.Simulation, *warlock.Warlock) {
	t.Helper()
	sim := core.NewSim(&proto.RaidSimRequest{
		SimOptions: &proto.SimOptions{RandomSeed: 1, Ruleset: proto.Ruleset_RulesetForever},
		Raid: core.SinglePlayerRaidProto(&proto.Player{
			Class:         proto.Class_ClassWarlock,
			Race:          proto.Race_RaceOrc,
			Equipment:     gear,
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

// Demonic Knowledge (412732) gives the demon the same 33/67/100% of the warlock's level in spell
// power as the warlock, while it is out.
func TestDemonicKnowledgeReachesTheDemon(t *testing.T) {
	sim, wl := newTestWarlock(t, TalentsDemonicPact, DefaultPactWarlock)
	if wl.Talents.DemonicKnowledge != 3 {
		t.Fatalf("test talents have Demonic Knowledge %d, want 3", wl.Talents.DemonicKnowledge)
	}
	pet := wl.ActivePet
	if pet == nil || !pet.IsEnabled() {
		t.Fatal("no demon out")
	}

	bonus := float64(wl.Level)
	for _, unit := range []*core.Unit{&wl.Unit, &pet.Unit} {
		aura := unit.GetAura("Demonic Knowledge")
		if aura == nil || !aura.IsActive() {
			t.Fatalf("%s: Demonic Knowledge is not active", unit.Label)
		}
		with := unit.GetStat(stats.SpellPower)
		aura.Deactivate(sim)
		if got := with - unit.GetStat(stats.SpellPower); !near(got, bonus) {
			t.Errorf("%s: Demonic Knowledge gives %.2f spell power, want %.2f", unit.Label, got, bonus)
		}
	}
}

// Hazza'rah's Charm of Destruction: Massive Destruction's (24543) crit is over a class mask that
// leaves out Incinerate.
func TestHazzarahsCharmOfDestructionSkipsIncinerate(t *testing.T) {
	gear := core.GetGearSet("../../../ui/warlock/gear_sets", "prebis").GearSet
	gear.Items[proto.ItemSlot_ItemSlotTrinket1] = &proto.ItemSpec{Id: warlock.HazzarahsCharmOfDestruction}
	// Affliction with Incinerate, the Destruction tree's last talent.
	sim, wl := newTestWarlockWithGear(t, "2535002013521105--0550005100000001", DefaultDestroWarlock, gear)
	if wl.Incinerate == nil {
		t.Fatal("test talents have no Incinerate")
	}
	aura := wl.GetAura("Massive Destruction")
	if aura == nil {
		t.Fatal("Hazza'rah's Charm of Destruction is not equipped")
	}

	shadowBolt := wl.ShadowBolt[len(wl.ShadowBolt)-1]
	boltCrit, incinerateCrit := shadowBolt.BonusCritRating, wl.Incinerate.BonusCritRating
	aura.Activate(sim)
	if got, want := shadowBolt.BonusCritRating, boltCrit+10*core.SpellCritRatingPerCritChance; !near(got, want) {
		t.Errorf("Shadow Bolt bonus crit %v under Massive Destruction, want %v", got, want)
	}
	if got := wl.Incinerate.BonusCritRating; !near(got, incinerateCrit) {
		t.Errorf("Incinerate bonus crit %v under Massive Destruction, want %v", got, incinerateCrit)
	}
}
