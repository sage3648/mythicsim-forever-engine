package sim

import (
	"testing"

	"github.com/wowsims/classic/sim/core"
	"github.com/wowsims/classic/sim/core/proto"
	"github.com/wowsims/classic/sim/core/stats"
)

// Item effects whose Forever behaviour differs from Classic Era's in the proc itself, checked
// against client 1.60.1.69977 through the forever engine's item port (ElliotWood/Forever #423).

// A fury warrior in the p0 BiS set with mainHand in the main hand. With no rotation the only
// melee hits are white swings, which keeps the proc arithmetic below simple.
func itemTestWarrior(mainHand int32, rotation *proto.APLRotation) *proto.Player {
	equipment := core.GetGearSet("../ui/warrior/gear_sets", "p0.bis").GearSet
	equipment.Items[proto.ItemSlot_ItemSlotMainHand] = &proto.ItemSpec{Id: mainHand}
	return &proto.Player{
		Class:         proto.Class_ClassWarrior,
		Race:          proto.Race_RaceOrc,
		Equipment:     equipment,
		TalentsString: "30305013-050520035150310051",
		Rotation:      rotation,
		Spec: &proto.Player_Warrior{Warrior: &proto.Warrior{Options: &proto.Warrior_Options{
			StartingRage: 50,
			Shout:        proto.WarriorShout_WarriorShoutBattle,
		}}},
	}
}

func runItemTestSim(t *testing.T, player *proto.Player, iterations int32) *proto.UnitMetrics {
	t.Helper()
	result := core.RunRaidSim(&proto.RaidSimRequest{
		Raid:       core.SinglePlayerRaidProto(player, &proto.PartyBuffs{}, &proto.RaidBuffs{}, &proto.Debuffs{}),
		Encounter:  core.MakeSingleTargetEncounter(0),
		SimOptions: &proto.SimOptions{Iterations: iterations, RandomSeed: 101, Ruleset: proto.Ruleset_RulesetForever},
	})
	if result.Error != nil {
		t.Fatal(result.Error.Message)
	}
	return result.RaidMetrics.Parties[0].Players[0]
}

// Ironfoe (11684): Fury of Forgewright (1301046) is a 6% chance on any melee hit, off-hand
// swings included, to grant 2 extra attacks (15494). Era's was 0.8 PPM off Ironfoe's own hits,
// about 3% of main-hand swings and under 2% of all of them.
func TestIronfoeProcsOffAnyMeleeHit(t *testing.T) {
	warrior := itemTestWarrior(11684, &proto.APLRotation{})
	// Hand of Justice's extra attacks would count as Ironfoe's below.
	warrior.Equipment.Items[proto.ItemSlot_ItemSlotTrinket2] = &proto.ItemSpec{}
	player := runItemTestSim(t, warrior, 50)

	// Every proc swings twice more; those swings are the main hand's extra attack split (tag 3).
	var landed, procs float64
	for _, action := range player.Actions {
		if action.Id.GetOtherId() != proto.OtherAction_OtherActionAttack {
			continue
		}
		for _, target := range action.Targets {
			landed += float64(target.Hits + target.Crits + target.Glances + target.Blocks)
			if action.Id.Tag == 3 {
				procs += float64(target.Casts) / 2
			}
		}
	}
	if landed == 0 || procs == 0 {
		t.Fatalf("Ironfoe never proced (%v procs over %v landed swings)", procs, landed)
	}
	// The two extra attacks land inside the 100 ms proc cooldown, so they cannot proc again and
	// the rate over all landed swings sits a little under 6%.
	if rate := procs / landed; rate < 0.045 || rate > 0.07 {
		t.Errorf("Ironfoe proced on %.2f%% of landed swings, want about 6%% (%v procs, %v landed)", rate*100, procs, landed)
	}
}

// Dragon's Call (10847): the Emerald Dragon Whelp spits Acid Spit (9591), 374 to 503 Nature
// before its spell damage. It used to never spit, because the summon never told it when it
// would despawn, and swung its extra melee instead.
func TestDragonsCallWhelpSpitsAcid(t *testing.T) {
	player := runItemTestSim(t, itemTestWarrior(10847, core.GetAplRotation("../ui/warrior/apls", "dps_reck").Rotation), 20)

	if len(player.Pets) == 0 {
		t.Fatal("no Emerald Dragon Whelp")
	}
	for _, pet := range player.Pets {
		for _, action := range pet.Actions {
			if action.Id.GetSpellId() != 9591 {
				continue
			}
			for _, target := range action.Targets {
				if target.Hits+target.Crits > 0 && target.Damage/float64(target.Hits+target.Crits) >= 374 {
					return
				}
			}
			t.Fatalf("Acid Spit cast but dealt too little: %v", action.Targets)
		}
	}
	t.Fatal("the whelp never cast Acid Spit")
}

// Forever's consumables that differ from Era's (client 1.60.1.69977, ElliotWood/Forever #421).
func TestForeverConsumableStats(t *testing.T) {
	// What the consumes add: the stats after the consumes phase, less those without any.
	consumesStats := func(consumes *proto.Consumes) stats.Stats {
		player := itemTestWarrior(13286, &proto.APLRotation{})
		player.Consumes = consumes
		result := core.ComputeStats(&proto.ComputeStatsRequest{
			Raid:      core.SinglePlayerRaidProto(player, &proto.PartyBuffs{}, &proto.RaidBuffs{}, &proto.Debuffs{}),
			Encounter: core.MakeSingleTargetEncounter(0),
		})
		if result.ErrorResult != "" {
			t.Fatal(result.ErrorResult)
		}
		return stats.FromFloatArray(result.RaidStats.Parties[0].Players[0].ConsumesStats.Stats)
	}

	for _, check := range []struct {
		name     string
		consumes *proto.Consumes
		want     map[stats.Stat]float64
	}{
		{"Grilled Squid", &proto.Consumes{Food: proto.Food_FoodGrilledSquid},
			map[stats.Stat]float64{stats.MeleeCrit: 1 * core.CritRatingPerCritChance, stats.Agility: 0}},
		{"Nightfin Soup", &proto.Consumes{Food: proto.Food_FoodNightfinSoup},
			map[stats.Stat]float64{stats.SpellDamage: 22, stats.MP5: 0}},
		{"Runn Tum Tuber Surprise", &proto.Consumes{Food: proto.Food_FoodRunnTumTuberSurprise},
			map[stats.Stat]float64{stats.Intellect: 15}},
		{"item 21546, the Elixir of Holy Power", &proto.Consumes{FirePowerBuff: proto.FirePowerBuff_ElixirOfGreaterFirepower},
			map[stats.Stat]float64{stats.HolyPower: 40, stats.FirePower: 0}},
		{"Elixir of Nature Power", &proto.Consumes{NaturePowerBuff: proto.NaturePowerBuff_ElixirOfNaturePower},
			map[stats.Stat]float64{stats.NaturePower: 40}},
		{"Greater Mageblood Elixir", &proto.Consumes{ManaRegenElixir: proto.ManaRegenElixir_GreaterMagebloodElixir},
			map[stats.Stat]float64{stats.MP5: 20}},
	} {
		with, without := consumesStats(check.consumes), consumesStats(&proto.Consumes{})
		for stat, want := range check.want {
			if got := with[stat] - without[stat]; got != want {
				t.Errorf("%s: adds %v %s, want %v", check.name, got, stat.StatName(), want)
			}
		}
	}
}
