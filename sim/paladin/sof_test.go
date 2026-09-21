package paladin

import (
	"math"
	"testing"
	"time"

	"github.com/wowsims/classic/sim/core"
	"github.com/wowsims/classic/sim/core/proto"
)

func furyTestPaladin(t *testing.T, talented bool) (*Paladin, *furyAbsorb, *core.Simulation) {
	t.Helper()
	character := core.NewCharacter(&core.Party{}, 0, &proto.Player{
		Class: proto.Class_ClassPaladin, Race: proto.Race_RaceHuman,
		Equipment: &proto.EquipmentSpec{},
		Spec:      &proto.Player_ProtectionPaladin{ProtectionPaladin: &proto.ProtectionPaladin{}},
	})
	p := &Paladin{Character: character, Talents: &proto.PaladinTalents{ImprovedSealOfFury: talented}}
	p.EnableManaBar()
	p.Env = &core.Environment{Ruleset: proto.Ruleset_RulesetForever, AllUnits: []*core.Unit{&p.Unit}}
	shield := p.registerFuryAbsorb()
	shield.shield.Spell.SpellMetrics = make([]core.SpellMetrics, 1)
	sim := &core.Simulation{Environment: p.Env}
	return p, shield, sim
}

func hitFuryShield(p *Paladin, sim *core.Simulation, amount float64, attackerLevel int32) float64 {
	result := &core.SpellResult{Target: &p.Unit, Damage: amount, Outcome: core.OutcomeHit}
	for _, modifier := range p.DynamicDamageTakenModifiers {
		modifier(sim, &core.Spell{Unit: &core.Unit{Level: attackerLevel}}, result)
	}
	return result.Damage
}

func TestFuryAbsorbConsumption(t *testing.T) {
	p, f, sim := furyTestPaladin(t, true)
	f.apply(sim, 100)
	if damage := hitFuryShield(p, sim, 40, 63); damage != 0 || f.remaining != 60 || p.CurrentMana() != 0 {
		t.Fatalf("partial absorb: damage=%v remaining=%v mana=%v", damage, f.remaining, p.CurrentMana())
	}
	if damage := hitFuryShield(p, sim, 90, 63); damage != 30 || f.shield.IsActive() || p.CurrentMana() != 87 {
		t.Fatalf("broken absorb: damage=%v active=%v mana=%v", damage, f.shield.IsActive(), p.CurrentMana())
	}
	if damage := hitFuryShield(p, sim, 50, 63); damage != 50 || p.CurrentMana() != 87 {
		t.Fatal("depleted shield absorbed again or returned mana twice")
	}
}

func TestFuryAbsorbReplacementExpiryAndReset(t *testing.T) {
	p, f, sim := furyTestPaladin(t, true)
	f.apply(sim, 100)
	hitFuryShield(p, sim, 20, 63)
	f.apply(sim, 30)
	if f.remaining != 30 || p.CurrentMana() != 0 {
		t.Fatal("replacement stacked or restored mana")
	}
	if f.shield.ExpiresAt() != 10*time.Second {
		t.Fatal("wrong client shield duration")
	}
	sim.CurrentTime = 10 * time.Second
	f.shield.Deactivate(sim)
	if f.remaining != 0 || p.CurrentMana() != 0 {
		t.Fatal("expiry retained absorb or restored mana")
	}
	f.apply(sim, 50)
	f.shield.Deactivate(sim)
	f.shield.OnReset(f.shield.Aura, sim)
	if damage := hitFuryShield(p, sim, 70, 63); damage != 70 || p.CurrentMana() != 0 {
		t.Fatal("shield leaked across iterations")
	}
}

func TestFuryManaTalentAndAttackerLevel(t *testing.T) {
	for _, tc := range []struct {
		talented bool
		level    int32
		mana     float64
	}{
		{false, 63, 0}, {true, 59, 60}, {true, 60, 60},
		{true, 61, 69}, {true, 62, 78}, {true, 63, 87}, {true, 70, 87},
	} {
		p, f, sim := furyTestPaladin(t, tc.talented)
		f.apply(sim, 20)
		hitFuryShield(p, sim, 0, tc.level)
		if f.remaining != 20 || p.CurrentMana() != 0 {
			t.Fatal("zero damage consumed shield")
		}
		hitFuryShield(p, sim, 20, tc.level)
		if math.Abs(p.CurrentMana()-tc.mana) > 1e-9 {
			t.Fatalf("%+v got mana %v", tc, p.CurrentMana())
		}
	}
}
