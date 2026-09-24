package shadow

import (
	"math"
	"testing"

	"github.com/wowsims/classic/sim/core"
	"github.com/wowsims/classic/sim/core/proto"
	"github.com/wowsims/classic/sim/core/simsignals"
	"github.com/wowsims/classic/sim/priest"
)

// A reset P1 shadow priest: Twin Disciplines 5, Mental Agility 3, Inner Focus, Shadowform.
func newTestShadowPriest(t *testing.T) (*core.Simulation, *priest.Priest) {
	t.Helper()
	sim := core.NewSim(&proto.RaidSimRequest{
		SimOptions: &proto.SimOptions{RandomSeed: 1, Ruleset: proto.Ruleset_RulesetForever},
		Raid: core.SinglePlayerRaidProto(&proto.Player{
			Class:         proto.Class_ClassPriest,
			Race:          proto.Race_RaceUndead,
			Equipment:     core.GetGearSet("../../../ui/shadow_priest/gear_sets", "p0.bis").GearSet,
			TalentsString: P1Talents,
			Rotation:      core.GetAplRotation("../../../ui/shadow_priest/apls", "p1").Rotation,
			Spec:          PlayerOptionsBasic,
		}, nil, nil, nil),
		Encounter: core.MakeSingleTargetEncounter(0),
	}, simsignals.CreateSignals())
	sim.Reset()

	p := sim.Raid.Parties[0].Players[0].(*ShadowPriest).Priest
	return sim, p
}

func topRank(spells []*core.Spell) *core.Spell {
	return spells[len(spells)-1]
}

func near(a, b float64) bool {
	return math.Abs(a-b) < 1e-9
}

// The channel the APL casts: the top rank's full-length Mind Flay.
func mindFlay(p *priest.Priest) *core.Spell {
	return p.MindFlay[priest.MindFlayRanks][0]
}

// Twin Disciplines follows 1225132's masks: the dot half on Shadow Word: Pain and Devouring
// Plague, nothing on the channels or Shadow Word: Death.
func TestTwinDisciplinesMask(t *testing.T) {
	_, p := newTestShadowPriest(t)
	if p.Talents.TwinDisciplines != 5 {
		t.Fatalf("test talents have Twin Disciplines %d, want 5", p.Talents.TwinDisciplines)
	}

	for name, spell := range map[string]*core.Spell{
		"Shadow Word: Pain": topRank(p.ShadowWordPain),
		"Devouring Plague":  topRank(p.DevouringPlague),
	} {
		if !near(spell.PeriodicDamageMultiplierAdditive, 1.05) || !near(spell.DamageMultiplierAdditive, 1) {
			t.Errorf("%s periodic/damage additive %v/%v, want 1.05/1", name, spell.PeriodicDamageMultiplierAdditive, spell.DamageMultiplierAdditive)
		}
	}
	for name, spell := range map[string]*core.Spell{
		"Mind Flay":          mindFlay(p),
		"Shadow Word: Death": topRank(p.ShadowWordDeath),
		"Mind Blast":         topRank(p.MindBlast),
	} {
		if !near(spell.PeriodicDamageMultiplierAdditive, 1) || !near(spell.DamageMultiplierAdditive, 1) {
			t.Errorf("%s periodic/damage additive %v/%v, want 1/1", name, spell.PeriodicDamageMultiplierAdditive, spell.DamageMultiplierAdditive)
		}
	}
}

// Mental Agility follows 14520's mask: Shadow Word: Pain and Devouring Plague are cheaper, the
// channels and Shadow Word: Death are not.
func TestMentalAgilityMask(t *testing.T) {
	_, p := newTestShadowPriest(t)
	if p.Talents.MentalAgility != 3 {
		t.Fatalf("test talents have Mental Agility %d, want 3", p.Talents.MentalAgility)
	}

	// Devouring Contagion takes its own 25% a point off Devouring Plague.
	want := map[string]struct {
		spell      *core.Spell
		multiplier int32
	}{
		"Shadow Word: Pain":  {topRank(p.ShadowWordPain), 90},
		"Devouring Plague":   {topRank(p.DevouringPlague), 90 - 25*p.Talents.DevouringContagion},
		"Vampiric Embrace":   {p.VampiricEmbrace, 90},
		"Mind Flay":          {mindFlay(p), 100},
		"Shadow Word: Death": {topRank(p.ShadowWordDeath), 100},
		"Mind Blast":         {topRank(p.MindBlast), 100},
	}
	for name, w := range want {
		if got := w.spell.Cost.Multiplier; got != w.multiplier {
			t.Errorf("%s cost multiplier %d, want %d", name, got, w.multiplier)
		}
	}
}
