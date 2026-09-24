package mage

import (
	"math"
	"testing"

	"github.com/wowsims/classic/sim/core"
	"github.com/wowsims/classic/sim/core/proto"
	"github.com/wowsims/classic/sim/core/simsignals"
)

// The Arcane Blast buff (400573) raises the damage of the spells its client mask names; Arcane
// Blast itself, Arcane Missiles, Blizzard and Flamestrike are not among them.
func TestArcaneBlastStacksSkipMissilesBlizzardFlamestrike(t *testing.T) {
	sim := core.NewSim(&proto.RaidSimRequest{
		SimOptions: &proto.SimOptions{RandomSeed: 1, Ruleset: proto.Ruleset_RulesetForever},
		Raid: core.SinglePlayerRaidProto(&proto.Player{
			Class:         proto.Class_ClassMage,
			Race:          proto.Race_RaceGnome,
			Equipment:     &proto.EquipmentSpec{},
			TalentsString: ForeverArcaneTalents,
			Rotation:      &proto.APLRotation{Type: proto.APLRotation_TypeAPL},
			Spec:          PlayerOptions,
		}, nil, nil, nil),
		Encounter: core.MakeSingleTargetEncounter(0),
	}, simsignals.CreateSignals())
	sim.Reset()

	mage := sim.Raid.Parties[0].Players[0].(MageAgent).GetMage()
	if mage.ArcaneBlastAura == nil {
		t.Fatal("talents have no Arcane Blast")
	}
	// The highest rank a level 60 mage has learned.
	last := func(spells []*core.Spell) *core.Spell {
		for i := len(spells) - 1; i >= 0; i-- {
			if spells[i] != nil {
				return spells[i]
			}
		}
		return nil
	}

	boosted := map[string]*core.Spell{
		"Fireball":         last(mage.Fireball),
		"Frostbolt":        last(mage.Frostbolt),
		"Fire Blast":       last(mage.FireBlast),
		"Scorch":           last(mage.Scorch),
		"Arcane Explosion": last(mage.ArcaneExplosion),
	}
	unboosted := map[string]*core.Spell{
		"Arcane Blast":          mage.ArcaneBlast,
		"Arcane Missiles":       last(mage.ArcaneMissiles),
		"Arcane Missiles (hit)": last(mage.ArcaneMissilesTickSpell),
		"Blizzard":              last(mage.Blizzard),
		"Flamestrike":           last(mage.Flamestrike),
	}

	before := map[*core.Spell]float64{}
	for _, spells := range []map[string]*core.Spell{boosted, unboosted} {
		for name, spell := range spells {
			if spell == nil {
				t.Fatalf("%s is not registered", name)
			}
			before[spell] = spell.DamageMultiplierAdditive
		}
	}

	mage.ArcaneBlastAura.Activate(sim)
	mage.ArcaneBlastAura.SetStacks(sim, ArcaneBlastMaxStacks)

	for name, spell := range boosted {
		if got, want := spell.DamageMultiplierAdditive-before[spell], .10*ArcaneBlastMaxStacks; math.Abs(got-want) > 1e-9 {
			t.Errorf("%s: +%.3f from %d stacks, want +%.3f", name, got, ArcaneBlastMaxStacks, want)
		}
	}
	for name, spell := range unboosted {
		if got := spell.DamageMultiplierAdditive - before[spell]; math.Abs(got) > 1e-9 {
			t.Errorf("%s: +%.3f from %d stacks, want none", name, got, ArcaneBlastMaxStacks)
		}
	}

	mage.ArcaneBlastAura.Deactivate(sim)
	for spell, was := range before {
		if math.Abs(spell.DamageMultiplierAdditive-was) > 1e-9 {
			t.Errorf("%v: %.3f after the stacks fell, want %.3f", spell.ActionID, spell.DamageMultiplierAdditive, was)
		}
	}
}
