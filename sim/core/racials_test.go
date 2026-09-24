package core

import (
	"testing"

	"github.com/wowsims/forever/sim/core/proto"
	"github.com/wowsims/forever/sim/core/simsignals"
	"github.com/wowsims/forever/sim/core/stats"
)

// racialWarrior builds a finalized (fake) warrior of the given race, so a test can read what
// its racials registered. mutate, if set, edits the player proto first.
func racialWarrior(race proto.Race, mutate func(*proto.Player)) *Character {
	player := &proto.Player{
		Name:        "Warrior",
		Race:        race,
		Class:       proto.Class_ClassWarrior,
		Buffs:       &proto.IndividualBuffs{},
		Consumables: &proto.ConsumesSpec{},
		Spec:        &proto.Player_DpsWarrior{},
		Equipment:   &proto.EquipmentSpec{},
		Rotation:    &proto.APLRotation{Type: proto.APLRotation_TypeAPL},
	}
	if mutate != nil {
		mutate(player)
	}
	sim := NewSim(&proto.RaidSimRequest{
		SimOptions: &proto.SimOptions{RandomSeed: 100},
		Raid: &proto.Raid{Parties: []*proto.Party{{
			Players: []*proto.Player{player},
			Buffs:   &proto.PartyBuffs{},
		}}},
		Encounter: &proto.Encounter{Targets: []*proto.Target{{Level: 63}}, Duration: 180},
	}, simsignals.CreateSignals())
	sim.Reset()
	return sim.Raid.Parties[0].Players[0].GetCharacter()
}

func disableRacials(player *proto.Player) { player.DisableRacials = true }

// DisableRacials measures what a race's racials are worth: the same character, base stats
// included, with none of its racial effects.
func TestDisableRacialsDropsRacialsButKeepsBaseStats(t *testing.T) {
	withRacials := racialWarrior(proto.Race_RaceOrc, nil)
	orc := racialWarrior(proto.Race_RaceOrc, disableRacials)

	if withRacials.GetAura("Blood Fury") == nil {
		t.Fatal("an orc warrior has no Blood Fury, so this test checks nothing")
	}
	if orc.GetAura("Blood Fury") != nil {
		t.Error("an orc with racials disabled still has Blood Fury")
	}
	if got, want := orc.GetBaseStats(), withRacials.GetBaseStats(); got != want {
		t.Errorf("disabling racials changed the orc's base stats: %v, want %v", got, want)
	}

	// The human's spirit racial is a multiplier on top of the base stats, so it goes too.
	human := racialWarrior(proto.Race_RaceHuman, nil)
	humanWithout := racialWarrior(proto.Race_RaceHuman, disableRacials)
	if human.GetStat(stats.Spirit) <= humanWithout.GetStat(stats.Spirit) {
		t.Errorf("human spirit %.1f with racials, %.1f without; the racial should raise it",
			human.GetStat(stats.Spirit), humanWithout.GetStat(stats.Spirit))
	}
	if got, want := humanWithout.GetStat(stats.Spirit), humanWithout.GetBaseStats()[stats.Spirit]; got != want {
		t.Errorf("human spirit with racials disabled is %.1f, want the base %.1f", got, want)
	}
}

// Race-only effects outside applyRaceEffects follow the option as well: Bloodthistle does
// nothing for a blood elf whose racials are off.
func TestDisableRacialsDropsBloodthistle(t *testing.T) {
	withThistle := func(player *proto.Player) { player.Consumables.Bloodthistle = true }

	on := racialWarrior(proto.Race_RaceBloodElf, withThistle)
	off := racialWarrior(proto.Race_RaceBloodElf, func(player *proto.Player) {
		withThistle(player)
		disableRacials(player)
	})

	if got, want := on.GetStat(stats.SpellDamage)-off.GetStat(stats.SpellDamage), 10.0; got != want {
		t.Errorf("Bloodthistle is worth %.1f spell damage with racials on and off, want %.1f", got, want)
	}
}
