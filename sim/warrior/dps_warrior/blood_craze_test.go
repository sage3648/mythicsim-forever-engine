package dpswarrior

import (
	"math"
	"strings"
	"testing"

	"github.com/wowsims/classic/sim/core"
	"github.com/wowsims/classic/sim/core/proto"
	"github.com/wowsims/classic/sim/core/simsignals"
)

// Blood Craze (16488) heals 1% of maximum health a point over its three ticks. Its snapshot used to
// set only the base heal and leave the multiplier at 0, so every tick healed nothing.
func TestBloodCrazeHealsShareOfMaxHealth(t *testing.T) {
	player := foreverWarrior(proto.Race_RaceOrc)
	trees := strings.Split(player.TalentsString, "-")
	fury := []byte(trees[1])
	fury[6] = '3' // Blood Craze 3/3
	trees[1] = string(fury)
	player.TalentsString = strings.Join(trees, "-")

	sim := core.NewSim(&proto.RaidSimRequest{
		SimOptions: &proto.SimOptions{RandomSeed: 1, Ruleset: proto.Ruleset_RulesetForever},
		Raid:       core.SinglePlayerRaidProto(player, nil, nil, nil),
		Encounter:  core.MakeSingleTargetEncounter(0),
	}, simsignals.CreateSignals())
	sim.Reset()

	warrior := sim.Raid.Parties[0].Players[0].GetCharacter()
	bloodCraze := warrior.GetSpell(core.ActionID{SpellID: 16488})
	if bloodCraze == nil {
		t.Fatal("3/3 Blood Craze registered no heal")
	}
	bloodCraze.Cast(sim, &warrior.Unit)
	for i := 0; i < 3; i++ {
		bloodCraze.SelfHot().TickOnce(sim)
	}

	healed := bloodCraze.SpellMetrics[warrior.UnitIndex].TotalHealing
	if want := 0.03 * warrior.MaxHealth(); math.Abs(healed-want) > 1e-6 {
		t.Errorf("three ticks healed %.2f, want 3%% of %.0f maximum health = %.2f", healed, warrior.MaxHealth(), want)
	}
}
