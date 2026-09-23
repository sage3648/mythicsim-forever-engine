package paladin

import (
	"testing"

	"github.com/wowsims/classic/sim/core"
	"github.com/wowsims/classic/sim/core/proto"
)

func TestSealOfCommandRequiresTalent(t *testing.T) {
	character := core.NewCharacter(&core.Party{}, 0, &proto.Player{
		Class: proto.Class_ClassPaladin, Race: proto.Race_RaceHuman,
		Equipment: &proto.EquipmentSpec{},
		Spec:      &proto.Player_RetributionPaladin{RetributionPaladin: &proto.RetributionPaladin{}},
	})
	p := &Paladin{Character: character, Talents: &proto.PaladinTalents{}}
	p.registerSealOfCommand()

	if len(p.aurasSoC) != 0 || len(p.spellsJoC) != 0 {
		t.Fatalf("untalented paladin registered %d Seal of Command auras and %d judgements", len(p.aurasSoC), len(p.spellsJoC))
	}
	for _, rank := range sealOfCommandRanks {
		if p.GetSpell(core.ActionID{SpellID: rank.spellID}) != nil {
			t.Fatalf("untalented paladin knows Seal of Command %d", rank.spellID)
		}
	}
}
