package enhancement

import (
	"math"
	"testing"

	"github.com/wowsims/classic/sim/core"
	"github.com/wowsims/classic/sim/core/proto"
	"github.com/wowsims/classic/sim/core/simsignals"
	"github.com/wowsims/classic/sim/shaman"
)

// An enhancement shaman on the launch gear with Flametongue on the main hand and Frostbrand on the
// off hand, built but not simmed.
func imbuedShaman(t *testing.T, talents string) *shaman.Shaman {
	t.Helper()
	player := &proto.Player{
		Class:         proto.Class_ClassShaman,
		Race:          proto.Race_RaceOrc,
		Equipment:     core.GetGearSet("../../../ui/enhancement_shaman/gear_sets", "launch").GearSet,
		TalentsString: talents,
		Consumes: &proto.Consumes{
			MainHandImbue: proto.WeaponImbue_FlametongueWeapon,
			OffHandImbue:  proto.WeaponImbue_FrostbrandWeapon,
		},
		Spec: PlayerOptionsSyncAuto,
	}
	raid := core.SinglePlayerRaidProto(player, nil, nil, nil)
	env, _, _ := core.NewEnvironment(raid, core.MakeSingleTargetEncounter(0), proto.Ruleset_RulesetForever, false)
	return env.Raid.Parties[0].Players[0].(shaman.ShamanAgent).GetShaman()
}

func spellsWithMask(sham *shaman.Shaman, mask int64) []*core.Spell {
	var spells []*core.Spell
	for _, spell := range sham.Spellbook {
		if spell.Matches(mask) {
			spells = append(spells, spell)
		}
	}
	return spells
}

// Elemental Fury's class mask (16089) names Flametongue Attack and Frostbrand Attack, so a shaman's
// own imbue hits crit for 2.0x at 5/5.
func TestElementalFuryReachesImbueAttacks(t *testing.T) {
	for _, tc := range []struct {
		talents string
		bonus   float64
	}{
		{"0000000", 1},
		{"0000000500000000", 2},
	} {
		sham := imbuedShaman(t, tc.talents)
		for _, mask := range []int64{shaman.SpellMaskFlametongueWeapon, shaman.SpellMaskFrostbrandWeapon} {
			spells := spellsWithMask(sham, mask)
			if len(spells) == 0 {
				t.Fatalf("no imbue attack with mask %d", mask)
			}
			for _, spell := range spells {
				if spell.CritDamageBonus != tc.bonus {
					t.Errorf("talents %q: %v crit damage bonus %v, want %v", tc.talents, spell.ActionID, spell.CritDamageBonus, tc.bonus)
				}
			}
		}
	}
}

// Lightning Shield's orbs roll spell hit and crit like the new engine line's, and Tidal Mastery
// (16194), whose class mask names Lightning Shield, adds its crit to them.
func TestLightningShieldOrbsCrit(t *testing.T) {
	player := &proto.Player{
		Class:     proto.Class_ClassShaman,
		Race:      proto.Race_RaceOrc,
		Equipment: core.GetGearSet("../../../ui/enhancement_shaman/gear_sets", "launch").GearSet,
		// 5/5 Tidal Mastery.
		TalentsString: "--0000000005",
		Spec:          PlayerOptionsSyncAuto,
	}
	sim := core.NewSim(&proto.RaidSimRequest{
		SimOptions: &proto.SimOptions{RandomSeed: 1, Ruleset: proto.Ruleset_RulesetForever},
		Raid:       core.SinglePlayerRaidProto(player, nil, nil, nil),
		Encounter:  core.MakeSingleTargetEncounter(0),
	}, simsignals.CreateSignals())
	sim.Reset()

	sham := sim.Raid.Parties[0].Players[0].(shaman.ShamanAgent).GetShaman()
	orb := sham.LightningShieldProcs[shaman.LightningShieldRanks]
	shield := sham.LightningShieldAuras[shaman.LightningShieldRanks]
	if orb.BonusCritRating != 5 {
		t.Errorf("orb bonus crit %v, want Tidal Mastery's 5", orb.BonusCritRating)
	}
	for i := 0; i < 300; i++ {
		if !shield.IsActive() {
			shield.Activate(sim)
			sham.ActiveShieldAura = shield
		}
		orb.Cast(sim, sham.CurrentTarget)
	}
	metrics := orb.SpellMetrics[sham.CurrentTarget.UnitIndex]
	if metrics.Crits == 0 || metrics.Misses == 0 {
		t.Errorf("300 orbs: %d crits and %d misses, want some of each", metrics.Crits, metrics.Misses)
	}
}

// Wushoolay's Charm of Spirits (24499) and Improved Lightning Shield (16261) are both percent damage
// modifiers on the orbs, so they add: 1 + 0.15 + 1 at 3/3, where Classic's charm doubled the total.
func TestWushoolaysCharmAddsToImprovedLightningShield(t *testing.T) {
	equipment := core.GetGearSet("../../../ui/enhancement_shaman/gear_sets", "launch").GearSet
	equipment.Items[proto.ItemSlot_ItemSlotTrinket1] = &proto.ItemSpec{Id: shaman.WushoolaysCharmOfSpirits}
	player := &proto.Player{
		Class:     proto.Class_ClassShaman,
		Race:      proto.Race_RaceOrc,
		Equipment: equipment,
		// 3/3 Improved Lightning Shield.
		TalentsString: "-0000003",
		Spec:          PlayerOptionsSyncAuto,
	}
	sim := core.NewSim(&proto.RaidSimRequest{
		SimOptions: &proto.SimOptions{RandomSeed: 1, Ruleset: proto.Ruleset_RulesetForever},
		Raid:       core.SinglePlayerRaidProto(player, nil, nil, nil),
		Encounter:  core.MakeSingleTargetEncounter(0),
	}, simsignals.CreateSignals())
	sim.Reset()

	sham := sim.Raid.Parties[0].Players[0].(shaman.ShamanAgent).GetShaman()
	orb := sham.LightningShieldProcs[shaman.LightningShieldRanks]
	charm := sham.GetAuraByID(core.ActionID{ItemID: shaman.WushoolaysCharmOfSpirits})
	if charm == nil {
		t.Fatal("no Wushoolay's Charm of Spirits aura")
	}
	if got := orb.DamageMultiplier * orb.DamageMultiplierAdditive; math.Abs(got-1.15) > 1e-9 {
		t.Errorf("orb multiplier %v with 3/3 Improved Lightning Shield, want 1.15", got)
	}
	charm.Activate(sim)
	if got := orb.DamageMultiplier * orb.DamageMultiplierAdditive; math.Abs(got-2.15) > 1e-9 {
		t.Errorf("orb multiplier %v with the charm up, want 2.15", got)
	}
	charm.Deactivate(sim)
	if got := orb.DamageMultiplier * orb.DamageMultiplierAdditive; math.Abs(got-1.15) > 1e-9 {
		t.Errorf("orb multiplier %v after the charm, want 1.15", got)
	}
}
