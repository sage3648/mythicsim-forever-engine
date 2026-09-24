package mage

import (
	"math"
	"testing"

	"github.com/wowsims/classic/sim/core"
	"github.com/wowsims/classic/sim/core/proto"
	"github.com/wowsims/classic/sim/core/simsignals"
)

// Arcane Potency (24544) raises Arcane Explosion and Arcane Missiles only, the spells its client
// mask names, not every Arcane spell.
func TestHazzarahsCharmOfMagicOnlyExplosionAndMissiles(t *testing.T) {
	items := make([]*proto.ItemSpec, proto.ItemSlot_ItemSlotTrinket1+1)
	for i := range items {
		items[i] = &proto.ItemSpec{}
	}
	items[proto.ItemSlot_ItemSlotTrinket1] = &proto.ItemSpec{Id: HazzarahsCharmOfMagic}

	sim := core.NewSim(&proto.RaidSimRequest{
		SimOptions: &proto.SimOptions{RandomSeed: 1, Ruleset: proto.Ruleset_RulesetForever},
		Raid: core.SinglePlayerRaidProto(&proto.Player{
			Class:         proto.Class_ClassMage,
			Race:          proto.Race_RaceGnome,
			Equipment:     &proto.EquipmentSpec{Items: items},
			TalentsString: ForeverArcaneTalents,
			Rotation:      &proto.APLRotation{Type: proto.APLRotation_TypeAPL},
			Spec:          PlayerOptions,
		}, nil, nil, nil),
		Encounter: core.MakeSingleTargetEncounter(0),
	}, simsignals.CreateSignals())
	sim.Reset()

	mage := sim.Raid.Parties[0].Players[0].(MageAgent).GetMage()
	potency := mage.GetAuraByID(core.ActionID{SpellID: 24544})
	if potency == nil {
		t.Fatal("no Arcane Potency aura; the charm is not equipped")
	}
	last := func(spells []*core.Spell) *core.Spell {
		for i := len(spells) - 1; i >= 0; i-- {
			if spells[i] != nil {
				return spells[i]
			}
		}
		return nil
	}

	boosted := map[string]*core.Spell{
		"Arcane Explosion":      last(mage.ArcaneExplosion),
		"Arcane Missiles (hit)": last(mage.ArcaneMissilesTickSpell),
	}
	unboosted := map[string]*core.Spell{
		"Arcane Blast": mage.ArcaneBlast,
		"Fireball":     last(mage.Fireball),
	}

	type critState struct{ chance, damage float64 }
	before := map[*core.Spell]critState{}
	for _, spells := range []map[string]*core.Spell{boosted, unboosted} {
		for name, spell := range spells {
			if spell == nil {
				t.Fatalf("%s is not registered", name)
			}
			before[spell] = critState{spell.BonusCritRating, spell.CritDamageBonus}
		}
	}

	potency.Activate(sim)
	for name, spell := range boosted {
		was := before[spell]
		if got := spell.BonusCritRating - was.chance; math.Abs(got-5*core.SpellCritRatingPerCritChance) > 1e-9 {
			t.Errorf("%s: crit +%.2f, want +5%%", name, got)
		}
		if got := spell.CritDamageBonus - was.damage; math.Abs(got-.5) > 1e-9 {
			t.Errorf("%s: crit damage +%.2f, want +0.50", name, got)
		}
	}
	for name, spell := range unboosted {
		if now := (critState{spell.BonusCritRating, spell.CritDamageBonus}); now != before[spell] {
			t.Errorf("%s: %+v with the charm up, want %+v", name, now, before[spell])
		}
	}

	potency.Deactivate(sim)
	for spell, was := range before {
		if now := (critState{spell.BonusCritRating, spell.CritDamageBonus}); now != was {
			t.Errorf("%v: %+v after the charm fell, want %+v", spell.ActionID, now, was)
		}
	}
}
