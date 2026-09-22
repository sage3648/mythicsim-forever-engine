package core

import "testing"

func TestSpellModFiltersAndLifecycle(t *testing.T) {
	unit := &Unit{}
	register := func(id int32, mask int64, school SpellSchool, flags SpellFlag) *Spell {
		return unit.RegisterSpell(SpellConfig{
			ActionID:         ActionID{SpellID: id},
			ClassSpellMask:   mask,
			SpellSchool:      school,
			ProcMask:         ProcMaskSpellDamage,
			Flags:            flags,
			DamageMultiplier: 1,
			ThreatMultiplier: 1,
		})
	}

	// Registered before the mods: picked up through OnSpellRegistered's replay of the spellbook.
	fire := register(1, 1<<1, SpellSchoolFire, SpellFlagAgentReserved1)

	unit.AddStaticMod(SpellModConfig{Kind: SpellMod_DamageDone_Flat, School: SpellSchoolFire, FloatValue: 0.1})
	unit.AddStaticMod(SpellModConfig{Kind: SpellMod_ThreatMultiplier_Pct, SpellFlag: SpellFlagAgentReserved1, FloatValue: -0.3})
	dynamic := unit.AddDynamicMod(SpellModConfig{Kind: SpellMod_BonusCrit_Percent, ClassMask: 1 << 2, FloatValue: 5})

	// Registered after: picked up as it registers.
	frost := register(2, 1<<2, SpellSchoolFrost, 0)

	if fire.DamageMultiplierAdditive != 1.1 || frost.DamageMultiplierAdditive != 1 {
		t.Fatalf("school filter: fire %v frost %v", fire.DamageMultiplierAdditive, frost.DamageMultiplierAdditive)
	}
	if fire.ThreatMultiplier != 0.7 || frost.ThreatMultiplier != 1 {
		t.Fatalf("flag filter: fire %v frost %v", fire.ThreatMultiplier, frost.ThreatMultiplier)
	}
	if frost.BonusCritRating != 0 {
		t.Fatalf("dynamic mod applied before Activate: %v", frost.BonusCritRating)
	}

	dynamic.Activate()
	dynamic.Activate() // idempotent
	if frost.BonusCritRating != 5 || fire.BonusCritRating != 0 {
		t.Fatalf("class mask filter: frost %v fire %v", frost.BonusCritRating, fire.BonusCritRating)
	}

	dynamic.UpdateFloatValue(8)
	if frost.BonusCritRating != 8 {
		t.Fatalf("UpdateFloatValue while active: %v", frost.BonusCritRating)
	}

	dynamic.Deactivate()
	dynamic.Deactivate() // idempotent
	if frost.BonusCritRating != 0 {
		t.Fatalf("Deactivate: %v", frost.BonusCritRating)
	}
}

func TestSpellModPowerCostPctAddRoundsToWholePercent(t *testing.T) {
	spell := &Spell{Cost: &SpellCost{Multiplier: 100}}
	mod := &SpellMod{floatValue: -0.05 * 3}
	spellModMap[SpellMod_PowerCost_Pct_Add].Apply(mod, spell)
	if spell.Cost.Multiplier != 85 {
		t.Fatalf("cost multiplier %v, want 85", spell.Cost.Multiplier)
	}
	spellModMap[SpellMod_PowerCost_Pct_Add].Remove(mod, spell)
	if spell.Cost.Multiplier != 100 {
		t.Fatalf("cost multiplier %v after remove, want 100", spell.Cost.Multiplier)
	}
}

func TestSpellModCustomNeedsApplyAndRemove(t *testing.T) {
	defer func() {
		if recover() == nil {
			t.Fatal("SpellMod_Custom without ApplyCustom/RemoveCustom should panic")
		}
	}()
	(&Unit{}).AddStaticMod(SpellModConfig{Kind: SpellMod_Custom})
}

func TestSpellModPeriodicDamageDoneFlat(t *testing.T) {
	spell := &Spell{PeriodicDamageMultiplierAdditive: 1}
	mod := &SpellMod{floatValue: 0.03}
	spellModMap[SpellMod_PeriodicDamageDone_Flat].Apply(mod, spell)
	if spell.PeriodicDamageMultiplierAdditive != 1.03 {
		t.Fatalf("periodic additive %v, want 1.03", spell.PeriodicDamageMultiplierAdditive)
	}
	spellModMap[SpellMod_PeriodicDamageDone_Flat].Remove(mod, spell)
	if spell.PeriodicDamageMultiplierAdditive != 1 {
		t.Fatalf("periodic additive %v after remove, want 1", spell.PeriodicDamageMultiplierAdditive)
	}
}

func TestSpellModBaseDamageDoneFlat(t *testing.T) {
	spell := &Spell{BaseDamageMultiplierAdditive: 1}
	mod := &SpellMod{floatValue: 0.1}
	spellModMap[SpellMod_BaseDamageDone_Flat].Apply(mod, spell)
	if spell.BaseDamageMultiplierAdditive != 1.1 {
		t.Fatalf("base additive %v, want 1.1", spell.BaseDamageMultiplierAdditive)
	}
	spellModMap[SpellMod_BaseDamageDone_Flat].Remove(mod, spell)
	if spell.BaseDamageMultiplierAdditive != 1 {
		t.Fatalf("base additive %v after remove, want 1", spell.BaseDamageMultiplierAdditive)
	}
}
