package enhancement

import (
	"testing"

	"github.com/wowsims/classic/sim/core"
	"github.com/wowsims/classic/sim/core/proto"
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
