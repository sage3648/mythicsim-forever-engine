package dpsrogue

import (
	"testing"

	"github.com/wowsims/classic/sim/core"
	"github.com/wowsims/classic/sim/core/proto"
	"github.com/wowsims/classic/sim/rogue"
)

// Cold Blood's client mask (14177) is Sinister Strike, Backstab, Ambush, Eviscerate and Mutilate's
// hits. Ghostly Strike and Hemorrhage, builders like the rest, are not on it.
func TestColdBloodOnClientMaskOnly(t *testing.T) {
	onMask := rogue.SpellMaskSinisterStrike | rogue.SpellMaskBackstab | rogue.SpellMaskAmbush |
		rogue.SpellMaskEviscerate | rogue.SpellMaskMutilate

	for _, talents := range []string{SubtletyHemorrhageTalents, AssassinationMutilateTalents} {
		player := &proto.Player{
			Class:         proto.Class_ClassRogue,
			Race:          proto.Race_RaceHuman,
			Equipment:     core.GetGearSet("../../../ui/rogue/gear_sets", "combat_backstab_prebis").GearSet,
			TalentsString: talents,
			Rotation:      core.GetAplRotation("../../../ui/rogue/apls", "combat_sinister_strike").Rotation,
			Consumes:      Phase1Consumes.Consumes,
			Spec:          DefaultRogue,
		}
		raid := core.SinglePlayerRaidProto(player, nil, nil, nil)
		env, _, _ := core.NewEnvironment(raid, core.MakeSingleTargetEncounter(0), proto.Ruleset_RulesetForever, false)
		character := env.Raid.Parties[0].Players[0].GetCharacter()

		for _, spell := range character.Spellbook {
			if spell.ClassSpellMask == 0 {
				continue
			}
			if want := spell.Matches(onMask); spell.Flags.Matches(rogue.SpellFlagColdBlooded) != want {
				t.Errorf("%v (mask %d): Cold Blood applies %v, want %v", spell.ActionID, spell.ClassSpellMask, !want, want)
			}
		}
	}
}
