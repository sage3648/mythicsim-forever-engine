package mage

import (
	"testing"

	"github.com/wowsims/forever/sim/core"
	"github.com/wowsims/forever/sim/core/proto"
	"github.com/wowsims/forever/sim/core/simsignals"
)

// Eureka! is a per-class spell in the Forever client. The mage's is 1259817, and it halves the
// mana cost of the mage's damaging spells while it is up.
func TestGnomeMageEureka(t *testing.T) {
	sim := core.NewSim(&proto.RaidSimRequest{
		Raid: core.SinglePlayerRaidProto(&proto.Player{
			Name:          "Mage",
			Class:         proto.Class_ClassMage,
			Race:          proto.Race_RaceGnome,
			Equipment:     &proto.EquipmentSpec{},
			Consumables:   &proto.ConsumesSpec{},
			TalentsString: FireTalents,
			Rotation:      &proto.APLRotation{Type: proto.APLRotation_TypeAPL},
			Spec:          &proto.Player_Mage{Mage: &proto.Mage{Options: &proto.Mage_Options{ClassOptions: &proto.MageOptions{}}}},
		}, &proto.PartyBuffs{}, &proto.RaidBuffs{}, &proto.Debuffs{}),
		Encounter:  core.MakeSingleTargetEncounter(0),
		SimOptions: &proto.SimOptions{RandomSeed: 101},
	}, simsignals.CreateSignals())
	sim.Reset()
	mage := sim.Raid.Parties[0].Players[0].GetCharacter()

	eureka := mage.GetAuraByID(core.ActionID{SpellID: 1259817})
	if eureka == nil {
		t.Fatal("a gnome mage has no Eureka! (1259817)")
	}
	if mage.GetAuraByID(core.ActionID{SpellID: 1259813}) != nil {
		t.Error("a gnome mage has the warrior's Eureka!")
	}

	var spell *core.Spell
	for _, s := range mage.Spellbook {
		if s.Cost != nil && s.Cost.BaseCost > 0 && s.ProcMask.Matches(core.ProcMaskSpellDamage) {
			spell = s
			break
		}
	}
	if spell == nil {
		t.Fatal("a mage with no damaging spell that costs mana")
	}

	before := spell.Cost.GetCurrentCost()
	eureka.Activate(sim)
	if got, want := spell.Cost.GetCurrentCost(), before*0.5; !core.WithinToleranceFloat64(want, got, 1) {
		t.Errorf("%v costs %.1f mana under Eureka!, want %.1f (half of %.1f)", spell.ActionID, got, want, before)
	}
}
