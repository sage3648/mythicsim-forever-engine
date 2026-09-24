package paladin_test

import (
	"os"
	"testing"

	_ "github.com/wowsims/classic/sim/common" // weapon enchant effects, Crusader among them
	"github.com/wowsims/classic/sim/core"
	"github.com/wowsims/classic/sim/core/proto"
	"github.com/wowsims/classic/sim/core/simsignals"
	"github.com/wowsims/classic/sim/paladin/protection"
	"google.golang.org/protobuf/encoding/protojson"
)

// This lives outside the spec packages so the item effects it needs don't reach their goldens.
func init() {
	protection.RegisterProtectionPaladin()
}

// Neither seal's damage suppresses weapon procs (wowsims/forever fc548dc29), so a Crusader weapon
// procs off the Fury and Righteousness hits alone, with no white swing involved.
func TestSealHitsRollWeaponProcs(t *testing.T) {
	raw, err := os.ReadFile("protection/testdata/seal_request.json")
	if err != nil {
		t.Fatal(err)
	}
	for _, seal := range []struct {
		name   string
		procID int32
	}{{"Fury", 20418}, {"Righteousness", 25713}} {
		req := &proto.RaidSimRequest{}
		if err := protojson.Unmarshal(raw, req); err != nil {
			t.Fatal(err)
		}
		for _, item := range req.Raid.Parties[0].Players[0].Equipment.Items {
			if item.Id == 12584 { // Grand Marshal's Longsword, the main hand
				item.Enchant = 1900 // Crusader
			}
		}
		sim := core.NewSim(req, simsignals.CreateSignals())
		sim.Reset()
		character := sim.Raid.Parties[0].Players[0].GetCharacter()
		proc := character.GetSpell(core.ActionID{SpellID: seal.procID})
		crusader := character.GetAuraByID(core.ActionID{SpellID: 20007, Tag: 1})
		if proc == nil || crusader == nil {
			t.Fatalf("%s: missing the seal's hit or the main-hand Crusader aura", seal.name)
		}
		for i := 0; i < 500 && !crusader.IsActive(); i++ {
			proc.Cast(sim, character.CurrentTarget)
		}
		if !crusader.IsActive() {
			t.Errorf("Crusader never procced off 500 %s hits", seal.name)
		}
	}
}
