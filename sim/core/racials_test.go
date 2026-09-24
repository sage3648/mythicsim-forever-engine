package core

import (
	"math"
	"testing"
	"time"

	"github.com/wowsims/forever/sim/core/proto"
	"github.com/wowsims/forever/sim/core/simsignals"
	"github.com/wowsims/forever/sim/core/stats"
)

// racialWarriorSim builds a finalized (fake) warrior of the given race, so a test can read what
// its racials registered. mutate, if set, edits the player proto first.
func racialWarriorSim(race proto.Race, mutate func(*proto.Player)) (*Simulation, *Character) {
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
		Encounter: &proto.Encounter{
			Targets: []*proto.Target{
				{Name: "humanoid", Level: 63, MobType: proto.MobType_MobTypeHumanoid},
				{Name: "beast", Level: 63, MobType: proto.MobType_MobTypeBeast},
				{Name: "elemental", Level: 63, MobType: proto.MobType_MobTypeElemental},
			},
			Duration: 180,
		},
	}, simsignals.CreateSignals())
	sim.Reset()
	return sim, sim.Raid.Parties[0].Players[0].GetCharacter()
}

func racialWarrior(race proto.Race, mutate func(*proto.Player)) *Character {
	_, character := racialWarriorSim(race, mutate)
	return character
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

// The checks below hold the Forever racials to client 1.60.1.69977. Players on the MythicSim
// Discord caught the first two on the previous engine line from the race comparison page:
// Windshaper was given a copy of Blood Fury that no Skyborne has, and Blood Fury cost a
// global cooldown.

// Both Skyborne halves share one racial skill line (2980): Wind Blessed, Elemental Insight,
// Read Ley Line, Walk on Air and Skysight. Neither has a damage cooldown, their base stats are
// the class baseline, and they differ only in faction.
func TestSkyborneHalvesShareTheirRacials(t *testing.T) {
	human := racialWarrior(proto.Race_RaceHuman, nil)
	for race, faction := range map[proto.Race]proto.Faction{
		proto.Race_RaceSkyborneHighOrder:  proto.Faction_Alliance,
		proto.Race_RaceSkyborneWindshaper: proto.Faction_Horde,
	} {
		skyborne := racialWarrior(race, nil)
		if got := skyborne.GetFaction(); got != faction {
			t.Errorf("%v is %v, want %v", race, got, faction)
		}
		if got, want := skyborne.GetBaseStats(), human.GetBaseStats(); got != want {
			t.Errorf("%v base stats %v, want the class baseline %v", race, got, want)
		}
		if skyborne.GetSpell(ActionID{SpellID: 460530}) != nil {
			t.Errorf("%v has the Windshaper cooldown, which no client spell backs", race)
		}
		if len(skyborne.GetMajorCooldowns()) != 0 {
			t.Errorf("%v has %d racial cooldowns, want none", race, len(skyborne.GetMajorCooldowns()))
		}

		// Wind Blessed: 1% melee, ranged and cast speed.
		for name, multiplier := range map[string]float64{
			"melee":  skyborne.PseudoStats.MeleeSpeedMultiplier,
			"ranged": skyborne.PseudoStats.RangedSpeedMultiplier,
			"cast":   skyborne.PseudoStats.CastSpeedMultiplier,
		} {
			if multiplier != 1.01 {
				t.Errorf("%v %s speed multiplier %.4f, want 1.01", race, name, multiplier)
			}
		}

		// Elemental Insight: 5% against Elementals and nothing else.
		for i, want := range []float64{1, 1, 1.05} {
			target := skyborne.Env.Encounter.AllTargetUnits[i]
			if got := skyborne.AttackTables[target.UnitIndex].DamageDealtMultiplier; got != want {
				t.Errorf("%v against %s: damage multiplier %.3f, want %.3f", race, target.Label, got, want)
			}
		}
	}
}

// Blood Fury (20572) has no start recovery category or time in the client, so it is off the
// global cooldown, and its three effects are percentage modifiers on melee attack power,
// ranged attack power and spell power.
func TestForeverBloodFury(t *testing.T) {
	sim, orc := racialWarriorSim(proto.Race_RaceOrc, func(player *proto.Player) {
		player.BonusStats = &proto.UnitStats{Stats: make([]float64, stats.ProtoStatsLen)}
		player.BonusStats.Stats[stats.AttackPower] = 1000
		player.BonusStats.Stats[stats.RangedAttackPower] = 500
		player.BonusStats.Stats[stats.SpellDamage] = 200
		player.BonusStats.Stats[stats.FireDamage] = 50
	})

	bloodFury := orc.GetSpell(ActionID{SpellID: 20572})
	if bloodFury == nil {
		t.Fatal("an orc warrior has no Blood Fury")
	}
	if bloodFury.DefaultCast.GCD != 0 {
		t.Errorf("Blood Fury triggers a %v global cooldown", bloodFury.DefaultCast.GCD)
	}
	if bloodFury.CD.Duration != 2*time.Minute {
		t.Errorf("Blood Fury cooldown %v, want 2m", bloodFury.CD.Duration)
	}
	if bloodFury.Cost != nil {
		t.Error("Blood Fury has a resource cost")
	}
	if orc.GetSpell(ActionID{SpellID: 33697}) != nil {
		t.Error("the orc still has TBC's flat Blood Fury (33697)")
	}

	aura := orc.GetAuraByID(ActionID{SpellID: 20572})
	if aura.Duration != 15*time.Second {
		t.Errorf("Blood Fury lasts %v, want 15s", aura.Duration)
	}

	before := orc.GetStats()
	aura.Activate(sim)
	after := orc.GetStats()
	for _, stat := range []stats.Stat{stats.AttackPower, stats.RangedAttackPower, stats.SpellDamage, stats.FireDamage} {
		if !WithinToleranceFloat64(before[stat]*1.1, after[stat], 1e-6) {
			t.Errorf("Blood Fury took %s from %.2f to %.2f, want %.2f", stat.StatName(), before[stat], after[stat], before[stat]*1.1)
		}
	}
	aura.Deactivate(sim)
	if got := orc.GetStats(); got != before {
		t.Errorf("Blood Fury left stats behind: %v, want %v", got, before)
	}
}

// The warrior's Eureka! (1259813) lasts 15 seconds with three charges on a two minute
// cooldown, off the global cooldown.
func TestForeverEurekaDurationAndCharges(t *testing.T) {
	gnome := racialWarrior(proto.Race_RaceGnome, nil)
	eureka := gnome.GetAuraByID(ActionID{SpellID: 1259813})
	if eureka == nil {
		t.Fatal("a gnome warrior has no Eureka!")
	}
	if eureka.Duration != 15*time.Second {
		t.Errorf("Eureka! lasts %v, want 15s", eureka.Duration)
	}
	if eureka.MaxStacks != 3 {
		t.Errorf("Eureka! has %d charges, want 3", eureka.MaxStacks)
	}
	spell := gnome.GetSpell(ActionID{SpellID: 1259813})
	if spell.DefaultCast.GCD != 0 || spell.CD.Duration != 2*time.Minute {
		t.Errorf("Eureka! has a %v global cooldown and a %v cooldown, want none and 2m", spell.DefaultCast.GCD, spell.CD.Duration)
	}

	// Expansive Mind raises the resource pool now, not Intellect.
	if got, want := gnome.GetStat(stats.Intellect), gnome.GetBaseStats()[stats.Intellect]; got != want {
		t.Errorf("gnome intellect %.2f, want the unmultiplied base %.2f", got, want)
	}
}

// Berserking (20554) is one flat 10% for everyone, free and off the global cooldown.
func TestForeverBerserking(t *testing.T) {
	troll := racialWarrior(proto.Race_RaceTroll, nil)
	spell := troll.GetSpell(ActionID{SpellID: 26297, Tag: 2})
	if spell == nil {
		t.Fatal("a troll warrior has no Berserking")
	}
	if spell.Cost != nil {
		t.Error("Berserking has a resource cost")
	}
	if spell.DefaultCast.GCD != 0 || spell.CD.Duration != 3*time.Minute {
		t.Errorf("Berserking has a %v global cooldown and a %v cooldown, want none and 3m", spell.DefaultCast.GCD, spell.CD.Duration)
	}
	if len(troll.GetMajorCooldowns()) != 1 {
		t.Errorf("a troll warrior has %d racial cooldowns, want Berserking alone", len(troll.GetMajorCooldowns()))
	}
	if got := troll.GetAuraByID(ActionID{SpellID: 26297, Tag: 2}).Duration; got != 10*time.Second {
		t.Errorf("Berserking lasts %v, want 10s", got)
	}
}

// Elune's Light (460520): 10% critical strike with spells and attacks for 15 seconds.
func TestForeverElunesLight(t *testing.T) {
	sim, nightElf := racialWarriorSim(proto.Race_RaceNightElf, nil)
	aura := nightElf.GetAuraByID(ActionID{SpellID: 460520})
	if aura == nil {
		t.Fatal("a night elf warrior has no Elune's Light")
	}
	if aura.Duration != 15*time.Second {
		t.Errorf("Elune's Light lasts %v, want 15s", aura.Duration)
	}
	if got := nightElf.GetSpell(ActionID{SpellID: 460520}).CD.Duration; got != 3*time.Minute {
		t.Errorf("Elune's Light cooldown %v, want 3m", got)
	}

	before := nightElf.GetStats()
	aura.Activate(sim)
	after := nightElf.GetStats()
	for _, stat := range []stats.Stat{stats.PhysicalCritPercent, stats.SpellCritPercent} {
		if got := after[stat] - before[stat]; !WithinToleranceFloat64(10, got, 1e-9) {
			t.Errorf("Elune's Light adds %.2f %s, want 10", got, stat.StatName())
		}
	}
}

// Touch of the Grave replaces the undead's Shadow Resistance, and every other +10 resistance
// racial is gone too.
func TestForeverRacialsCarryNoResistances(t *testing.T) {
	for _, race := range []proto.Race{proto.Race_RaceDwarf, proto.Race_RaceGnome, proto.Race_RaceNightElf, proto.Race_RaceTauren, proto.Race_RaceUndead} {
		character := racialWarrior(race, nil)
		for _, stat := range []stats.Stat{stats.ArcaneResistance, stats.FireResistance, stats.FrostResistance, stats.NatureResistance, stats.ShadowResistance} {
			if got := character.GetStat(stat); got != 0 {
				t.Errorf("a naked %v has %.0f %s", race, got, stat.StatName())
			}
		}
	}

	undead := racialWarrior(proto.Race_RaceUndead, nil)
	if undead.GetSpell(ActionID{SpellID: 1260198}) == nil || undead.GetAura("Touch of the Grave") == nil {
		t.Error("an undead warrior has no Touch of the Grave")
	}
}

// Tauren Endurance carries a point of hit, both pools, alongside the health.
func TestForeverTaurenEndurance(t *testing.T) {
	tauren := racialWarrior(proto.Race_RaceTauren, nil)
	without := racialWarrior(proto.Race_RaceTauren, disableRacials)
	for _, stat := range []stats.Stat{stats.PhysicalHitPercent, stats.SpellHitPercent} {
		if got := tauren.GetStat(stat) - without.GetStat(stat); got != 1 {
			t.Errorf("Endurance adds %.2f %s, want 1", got, stat.StatName())
		}
	}
	if got, want := tauren.GetStat(stats.Health), without.GetStat(stats.Health)*1.05; !WithinToleranceFloat64(want, got, 1) {
		t.Errorf("tauren health %.0f, want %.0f", got, want)
	}
}

// The Human Spirit is Classic's 5%, not TBC's 10%.
func TestForeverHumanSpirit(t *testing.T) {
	human := racialWarrior(proto.Race_RaceHuman, nil)
	base := human.GetBaseStats()[stats.Spirit]
	if got := human.GetStat(stats.Spirit); got < math.Floor(base*1.05) || got > base*1.05 {
		t.Errorf("human spirit %.2f from a base of %.0f, want 5%% more", got, base)
	}
}

// Every race the picker offers a class has a base stat row to build from, and the Skyborne
// rows are the class baseline for both halves.
func TestEveryEligibleRaceClassHasBaseStats(t *testing.T) {
	for class, races := range ClassRaceCapabilities {
		for _, race := range races {
			if BaseStats[BaseStatsKey{Race: race, Class: class}][stats.Health] <= 0 {
				t.Errorf("%v %v is offered but has no base stats", race, class)
			}
		}
	}

	for _, class := range []proto.Class{proto.Class_ClassWarrior, proto.Class_ClassHunter, proto.Class_ClassRogue, proto.Class_ClassDruid} {
		highOrder := BaseStats[BaseStatsKey{Race: proto.Race_RaceSkyborneHighOrder, Class: class}]
		windshaper := BaseStats[BaseStatsKey{Race: proto.Race_RaceSkyborneWindshaper, Class: class}]
		if highOrder != windshaper {
			t.Errorf("Skyborne %v: High Order %v and Windshaper %v differ", class, highOrder, windshaper)
		}
	}
}

func TestGetFaction(t *testing.T) {
	for race, want := range map[proto.Race]proto.Faction{
		proto.Race_RaceHuman:              proto.Faction_Alliance,
		proto.Race_RaceDwarf:              proto.Faction_Alliance,
		proto.Race_RaceGnome:              proto.Faction_Alliance,
		proto.Race_RaceNightElf:           proto.Faction_Alliance,
		proto.Race_RaceDraenei:            proto.Faction_Alliance,
		proto.Race_RaceSkyborneHighOrder:  proto.Faction_Alliance,
		proto.Race_RaceOrc:                proto.Faction_Horde,
		proto.Race_RaceTroll:              proto.Faction_Horde,
		proto.Race_RaceTauren:             proto.Faction_Horde,
		proto.Race_RaceUndead:             proto.Faction_Horde,
		proto.Race_RaceBloodElf:           proto.Faction_Horde,
		proto.Race_RaceSkyborneWindshaper: proto.Faction_Horde,
		proto.Race_RaceUnknown:            proto.Faction_Unknown,
	} {
		if got := (&Character{Race: race}).GetFaction(); got != want {
			t.Errorf("%v is %v, want %v", race, got, want)
		}
	}
}
