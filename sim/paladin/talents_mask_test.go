package paladin_test

import (
	"os"
	"testing"

	"github.com/wowsims/classic/sim/core"
	"github.com/wowsims/classic/sim/core/proto"
	"github.com/wowsims/classic/sim/core/stats"
	"github.com/wowsims/classic/sim/paladin"
	"google.golang.org/protobuf/encoding/protojson"
)

// The protection fixture's paladin with the given talents, built but not simmed.
func fixturePaladin(t *testing.T, talents string) *core.Character {
	t.Helper()
	raw, err := os.ReadFile("protection/testdata/seal_request.json")
	if err != nil {
		t.Fatal(err)
	}
	req := &proto.RaidSimRequest{}
	if err := protojson.Unmarshal(raw, req); err != nil {
		t.Fatal(err)
	}
	req.Raid.Parties[0].Players[0].TalentsString = talents
	env, _, _ := core.NewEnvironment(req.Raid, req.Encounter, proto.Ruleset_RulesetForever, false)
	return env.Raid.Parties[0].Players[0].GetCharacter()
}

// Divine Precision (1310904) is a miss chance mod on its class mask, not Holy school hit.
func TestDivinePrecisionOnClientMaskOnly(t *testing.T) {
	// 3/3 Divine Precision and Holy Shock, Holy Shield for its proc.
	character := fixturePaladin(t, "000000000000031-0000000000000001")
	if hit := character.PseudoStats.SchoolBonusHitChance[stats.SchoolIndexHoly]; hit != 0 {
		t.Errorf("Holy school hit %v, want 0", hit)
	}
	seen := int64(0)
	for _, spell := range character.Spellbook {
		want := 0.0
		if spell.Matches(paladin.SpellMaskDivinePrecision) {
			want = 18
			seen |= spell.ClassSpellMask
		}
		if spell.SpellSchool.Matches(core.SpellSchoolHoly) && spell.BonusHitRating != want {
			t.Errorf("%v (mask %d): bonus hit %v, want %v", spell.ActionID, spell.ClassSpellMask, spell.BonusHitRating, want)
		}
	}
	for _, mask := range []int64{paladin.SpellMaskExorcism, paladin.SpellMaskHolyShock, paladin.SpellMaskHolyStrike, paladin.SpellMaskConsecration, paladin.SpellMaskHolyWrath} {
		if seen&mask == 0 {
			t.Errorf("no spell with mask %d took Divine Precision", mask)
		}
	}
}
