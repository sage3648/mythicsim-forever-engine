package shared

import (
	"math"
	"testing"
)

// Attack power coefficients are hand-supplied, so the guards around them are the only thing standing
// between a typo and a spell that silently scales off nothing.
func apCoefTestTable() SpellDataTable {
	return SpellDataTable{
		{Rank: 1, SpellID: 100, Direct: SpellDataRange{Min: 10, Max: 20, Coef: 0.1}},
		{Rank: 2, SpellID: 200, Direct: SpellDataRange{Min: 30, Max: 40, Coef: 0.2}},
	}
}

func TestPerRankAPCoefficients(t *testing.T) {
	src := apCoefTestTable()
	out := WithSpellDataAPCoefs(src, map[int32]float64{1: 0.05, 2: 0.15})

	if got := SpellDataAPCoef(out[0].Direct); got != 0.05 {
		t.Errorf("rank 1 AP coef = %v, want 0.05", got)
	}
	if got := SpellDataAPCoef(out[1].Direct); got != 0.15 {
		t.Errorf("rank 2 AP coef = %v, want 0.15", got)
	}
	if got := SpellDataAPCoef(src[0].Direct); got != 0 {
		t.Errorf("source table was mutated: %v", got)
	}
	if got := SpellDataCoef(out[1].Direct); got != 0.2 {
		t.Errorf("SP coef lost: %v", got)
	}
}

func TestAPCoefficientMissingRankPanics(t *testing.T) {
	defer func() {
		if recover() == nil {
			t.Error("expected a panic for a rank with no coefficient")
		}
	}()
	WithSpellDataAPCoefs(apCoefTestTable(), map[int32]float64{1: 0.05})
}

func TestAPCoefficientUnknownRankPanics(t *testing.T) {
	defer func() {
		if recover() == nil {
			t.Error("expected a panic for a coefficient naming a rank that does not exist")
		}
	}()
	WithSpellDataAPCoefs(apCoefTestTable(), map[int32]float64{1: 0.05, 2: 0.15, 7: 0.25})
}

// Improved Righteous Fury's shape: one talent, two effects, and the role fields can only hold one.
func effectTestRank() SpellData {
	return SpellData{
		Rank: 3, SpellID: 20470,
		Effects: []SpellDataEffect{
			{Index: 0, Effect: E_APPLY_AURA, Aura: A_ADD_PCT_MODIFIER, Misc: 8, Value: 50},
			{Index: 1, Effect: E_APPLY_AURA, Aura: A_ADD_FLAT_MODIFIER, Misc: 12, Value: -6},
		},
		Direct: SpellDataFlat{Value: 50, Coef: 1},
	}
}

func TestEffectPicksByAura(t *testing.T) {
	rank := effectTestRank()
	if got := rank.Effect(A_ADD_PCT_MODIFIER, 8).Value; got != 50 {
		t.Errorf("threat effect: want 50, got %v", got)
	}
	if got := rank.Effect(A_ADD_FLAT_MODIFIER, 12).Value; got != -6 {
		t.Errorf("damage-taken effect: want -6, got %v", got)
	}
}

func TestEffectMissingPanics(t *testing.T) {
	defer func() {
		if recover() == nil {
			t.Error("expected a panic for an aura the rank does not carry")
		}
	}()
	effectTestRank().Effect(A_MOD_DAMAGE_PERCENT_DONE, 0)
}

// 186 ranked spells in this build carry two effects with the same aura and misc value. Returning the
// first is how a caller silently reads the wrong one.
func TestEffectAmbiguousPanics(t *testing.T) {
	rank := SpellData{
		Rank: 1, SpellID: 1,
		Effects: []SpellDataEffect{
			{Index: 0, Effect: E_APPLY_AURA, Aura: A_MOD_DAMAGE_PERCENT_DONE, Misc: 0, Value: 10},
			{Index: 1, Effect: E_APPLY_AURA, Aura: A_MOD_DAMAGE_PERCENT_DONE, Misc: 0, Value: 20},
		},
	}
	defer func() {
		if recover() == nil {
			t.Error("expected a panic for two effects sharing an aura and misc value")
		}
	}()
	rank.Effect(A_MOD_DAMAGE_PERCENT_DONE, 0)
}

func talentLadder() SpellDataTable {
	return SpellDataTable{
		{Rank: 1, SpellID: 20468, Effects: []SpellDataEffect{
			{Index: 0, Effect: E_APPLY_AURA, Aura: A_ADD_PCT_MODIFIER, Misc: SPELLMOD_ALL_EFFECTS, Value: 16},
			{Index: 1, Effect: E_APPLY_AURA, Aura: A_ADD_FLAT_MODIFIER, Misc: SPELLMOD_EFFECT2, Value: -2}}},
		{Rank: 2, SpellID: 20469, Effects: []SpellDataEffect{
			{Index: 0, Effect: E_APPLY_AURA, Aura: A_ADD_PCT_MODIFIER, Misc: SPELLMOD_ALL_EFFECTS, Value: 33},
			{Index: 1, Effect: E_APPLY_AURA, Aura: A_ADD_FLAT_MODIFIER, Misc: SPELLMOD_EFFECT2, Value: -4}}},
		{Rank: 3, SpellID: 20470, Effects: []SpellDataEffect{
			{Index: 0, Effect: E_APPLY_AURA, Aura: A_ADD_PCT_MODIFIER, Misc: SPELLMOD_ALL_EFFECTS, Value: 50},
			{Index: 1, Effect: E_APPLY_AURA, Aura: A_ADD_FLAT_MODIFIER, Misc: SPELLMOD_EFFECT2, Value: -6}}},
	}
}

// An untaken talent is rank 0, which ByRank would panic on.
func TestLadderUntakenTalentIsZero(t *testing.T) {
	table := talentLadder()
	if got := table.Effect(A_ADD_PCT_MODIFIER, SPELLMOD_ALL_EFFECTS).ValueAt(0); got != 0 {
		t.Errorf("rank 0 value: want 0, got %v", got)
	}
	if got := table.Effect(A_ADD_FLAT_MODIFIER, SPELLMOD_EFFECT2).MultiplierAt(0); got != 1 {
		t.Errorf("rank 0 multiplier: want 1, got %v", got)
	}
}

// The ladder is 16/33/50, not 16/32/48, which is what a per-point literal would give.
func TestLadderIsNotPerPointTimesRank(t *testing.T) {
	threat := talentLadder().Effect(A_ADD_PCT_MODIFIER, SPELLMOD_ALL_EFFECTS)
	for rank, want := range map[int32]float64{1: 1.16, 2: 1.33, 3: 1.50} {
		if got := threat.MultiplierAt(rank); math.Abs(got-want) > 1e-9 {
			t.Errorf("rank %d: want %v, got %v", rank, want, got)
		}
	}
}

// The client states the reduction negative, so the caller never writes the minus.
func TestLadderMultiplierTakesItsSignFromTheData(t *testing.T) {
	if got := talentLadder().Effect(A_ADD_FLAT_MODIFIER, SPELLMOD_EFFECT2).MultiplierAt(3); math.Abs(got-0.94) > 1e-9 {
		t.Errorf("want 0.94, got %v", got)
	}
}

func TestLadderUnnamedEffectOnMultiEffectTalentPanics(t *testing.T) {
	defer func() {
		if recover() == nil {
			t.Error("expected a panic for an unnamed read of a two-effect talent")
		}
	}()
	talentLadder().ValueAt(3)
}

// Client values the helper has to reproduce: Blizzard's top tick per bracket (1279978 at 40, 1279979
// at 50, 1279949 at 60), a rank past its cap, and Feint's negative threat (1966 at 25, 8637 at 50).
func TestLevelScaled(t *testing.T) {
	for _, c := range []struct {
		base, perLevel              float64
		spellLevel, maxLevel, level int32
		want                        float64
	}{
		{62, 0.2, 36, 41, 40, 62},
		{87, 0.3, 44, 49, 50, 88},
		{146, 0.4, 60, 65, 60, 146},
		{42, 0.2, 28, 33, 60, 43},
		{-750, -5, 16, 26, 25, -795},
		{-1950, -5, 40, 50, 50, -2000},
	} {
		if got := LevelScaled(c.base, c.perLevel, c.spellLevel, c.maxLevel, c.level); got != c.want {
			t.Errorf("LevelScaled(%v, %v, %d, %d, %d) = %v, want %v", c.base, c.perLevel, c.spellLevel, c.maxLevel, c.level, got, c.want)
		}
	}
}
