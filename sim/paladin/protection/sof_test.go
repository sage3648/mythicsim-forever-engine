package protection

import (
	"math"
	"os"
	"strings"
	"testing"

	"github.com/wowsims/classic/sim/core"
	"github.com/wowsims/classic/sim/core/proto"
	"google.golang.org/protobuf/encoding/protojson"
)

func TestSealOfFurySimulation(t *testing.T) {
	raw, err := os.ReadFile("testdata/seal_request.json")
	if err != nil {
		t.Fatal(err)
	}
	for _, tc := range []struct {
		name                 string
		seal                 proto.PaladinSeal
		shield, talent, tank bool
	}{
		{"fury_tank", proto.PaladinSeal_Fury, true, true, true},
		{"fury_without_talent", proto.PaladinSeal_Fury, true, false, true},
		{"fury_without_shield", proto.PaladinSeal_Fury, false, true, true},
		{"fury_without_incoming_damage", proto.PaladinSeal_Fury, true, true, false},
		{"righteousness", proto.PaladinSeal_Righteousness, true, true, true},
	} {
		t.Run(tc.name, func(t *testing.T) {
			req := &proto.RaidSimRequest{}
			if err := protojson.Unmarshal(raw, req); err != nil {
				t.Fatal(err)
			}
			p := req.Raid.Parties[0].Players[0]
			p.GetProtectionPaladin().Options.PrimarySeal = tc.seal
			if !tc.shield {
				for i, item := range p.Equipment.Items {
					if item.Id == 18825 {
						p.Equipment.Items[i] = &proto.ItemSpec{}
					}
				}
			}
			trees := strings.Split(p.TalentsString, "-")
			talents := []byte(trees[1])
			if tc.talent {
				talents[5] = '1'
			} else {
				talents[5] = '0'
			}
			trees[1] = string(talents)
			p.TalentsString = strings.Join(trees, "-")
			if !tc.tank {
				req.Raid.Tanks = nil
			}
			result := core.RunRaidSim(req)
			if result.Error != nil {
				t.Fatal(result.Error)
			}
			if result.IterationsDone != 10 {
				t.Fatalf("iterations: %d", result.IterationsDone)
			}
			metrics := result.RaidMetrics.Parties[0].Players[0]
			var fury, righteous, judgement, shielding, mana float64
			for _, action := range metrics.Actions {
				for _, target := range action.Targets {
					switch action.Id.GetSpellId() {
					case 20418:
						fury += target.Damage
					case 25713:
						righteous += target.Damage
					case 20414:
						judgement += target.Damage
					case 1310927:
						shielding += target.Shielding
					}
				}
			}
			for _, resource := range metrics.Resources {
				if resource.Id.GetSpellId() == 1314104 {
					mana += resource.Gain
				}
			}
			if tc.seal == proto.PaladinSeal_Fury {
				if fury <= 0 || judgement <= 0 || righteous != 0 {
					t.Fatalf("wrong seal actions: fury=%v judgement=%v righteousness=%v", fury, judgement, righteous)
				}
				if tc.shield && math.Abs(shielding-0.5*fury) > 1e-6 {
					t.Fatalf("shielding %v is not half of actual Fury damage %v", shielding, fury)
				}
				if (shielding > 0) != tc.shield {
					t.Fatalf("shielding=%v shield=%v", shielding, tc.shield)
				}
				if (mana > 0) != (tc.shield && tc.talent && tc.tank) {
					t.Fatalf("mana=%v", mana)
				}
			} else if righteous <= 0 || fury != 0 || shielding != 0 || mana != 0 {
				t.Fatal("Fury leaked into Righteousness")
			}
		})
	}
}

// A banked Fury proc must survive the upstream change from spell pointers to callbacks.
func TestFuryTwistOfLightEcho(t *testing.T) {
	raw, err := os.ReadFile("testdata/seal_request.json")
	if err != nil {
		t.Fatal(err)
	}
	for _, talented := range []bool{false, true} {
		req := &proto.RaidSimRequest{}
		if err := protojson.Unmarshal(raw, req); err != nil {
			t.Fatal(err)
		}
		p := req.Raid.Parties[0].Players[0]
		p.TalentsString = "--00000000000000000"
		if talented {
			p.TalentsString += "1"
		}
		req.Raid.Tanks = nil
		req.Encounter.Duration = 20
		req.Encounter.DurationVariation = 0
		p.Rotation = &proto.APLRotation{}
		// Change seals before combat, so any Fury damage must come from the banked echo.
		if err := protojson.Unmarshal([]byte(`{"type":"TypeAPL","prepullActions":[
   {"action":{"castSpell":{"spellId":{"spellId":20423}}},"doAtValue":{"const":{"val":"-3s"}}},
   {"action":{"castSpell":{"spellId":{"spellId":20293}}},"doAtValue":{"const":{"val":"-1.5s"}}}
  ]}`), p.Rotation); err != nil {
			t.Fatal(err)
		}
		result := core.RunRaidSim(req)
		if result.Error != nil {
			t.Fatal(result.Error)
		}
		var hits int32
		for _, action := range result.RaidMetrics.Parties[0].Players[0].Actions {
			if action.Id.GetSpellId() == 20418 {
				for _, target := range action.Targets {
					hits += target.Hits + target.Crits
				}
			}
		}
		want := int32(0)
		if talented {
			want = req.SimOptions.Iterations
		}
		if hits != want {
			t.Fatalf("Twist talented=%v: Fury hits=%d, want %d", talented, hits, want)
		}
	}
}

func TestFuryMatchesClientSealCombatRules(t *testing.T) {
	raw, err := os.ReadFile("testdata/seal_request.json")
	if err != nil {
		t.Fatal(err)
	}
	req := &proto.RaidSimRequest{}
	if err := protojson.Unmarshal(raw, req); err != nil {
		t.Fatal(err)
	}
	env, _, _ := core.NewEnvironment(req.Raid, req.Encounter, proto.Ruleset_RulesetForever, false)
	character := env.Raid.Parties[0].Players[0].GetCharacter()
	judgement := character.GetSpell(core.ActionID{SpellID: 20414})
	if judgement == nil || judgement.DefenseType != core.DefenseTypeMelee {
		t.Fatal("Judgement of Fury must use the client melee defense type")
	}
	fury := character.GetSpell(core.ActionID{SpellID: 20418})
	righteousness := character.GetSpell(core.ActionID{SpellID: 25713})
	if fury == nil || righteousness == nil || fury.DamageMultiplier != righteousness.DamageMultiplier {
		t.Fatal("Fury and Righteousness Holy procs must receive the same damage modifiers")
	}
}
