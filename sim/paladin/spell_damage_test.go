package paladin

import (
	"testing"

	"github.com/wowsims/classic/sim/common/shared"
)

// Our damage is kept beside the client table; it must not drift from it. The table has one value per
// rank, the centre of the range the game rolls at the rank's max level, so ours must contain it there.
func TestDamageContainsClientValue(t *testing.T) {
	// The growth to level 60 of a rank trained at level that stops growing at scaleLevel.
	grown := func(level, scaleLevel int32, scale float64) float64 {
		return float64(min(60, scaleLevel)-level) * scale
	}
	contains := func(name string, row shared.SpellData, low, high float64) {
		if value := shared.SpellDataMin(row.Direct); value < low || value > high {
			t.Errorf("%s %d: table %v outside our %v-%v", name, row.SpellID, value, low, high)
		}
	}

	for i, rank := range exorcismRanks {
		g := grown(rank.level, rank.scaleLevel, rank.scale)
		contains("Exorcism", spellData.Exorcism.ByRank(int32(i+1)), rank.minDamage+g, rank.maxDamage+g)
	}
	for i, rank := range hammerOfWrathRanks {
		contains("Hammer of Wrath", spellData.HammerOfWrath.ByRank(int32(i+1)), rank.minDamage, rank.maxDamage)
	}
	for _, rank := range holyWrathRanks {
		g := grown(rank.level, rank.scaleLevel, rank.scale)
		contains("Holy Wrath", spellData.HolyWrath.BySpellID(rank.spellID), rank.minDamage+g, rank.maxDamage+g)
	}
	for i, rank := range holyStrikeRanks {
		contains("Holy Strike", spellData.HolyStrike.ByRank(int32(i+1)), rank.minDamage, rank.maxDamage)
	}
	for _, rank := range sealOfRighteousnessRanks {
		g := grown(rank.level, rank.scaleLevel, rank.judge.scale)
		contains("Judgement of Righteousness", spellData.JudgementOfRighteousness.BySpellID(rank.judge.spellID), rank.judge.minDamage+g, rank.judge.maxDamage+g)
	}
	for _, rank := range sealOfCommandRanks {
		g := grown(rank.level, rank.scaleLevel, rank.judge.scale)
		contains("Judgement of Command", spellData.SealOfCommandTriggered.BySpellID(rank.judge.spellID), rank.judge.minDamage+g, rank.judge.maxDamage+g)
	}

	// Single values at level 60, which the table truncates, so it may sit up to 1 below ours.
	near := func(name string, rank int, table, ours float64) {
		if d := ours - table; d < 0 || d >= 1 {
			t.Errorf("%s rank %d: table %v, ours %v", name, rank, table, ours)
		}
	}
	for i, rank := range sealOfRighteousnessRanks {
		ours := rank.proc.value + grown(rank.level, rank.scaleLevel, rank.proc.scale)
		near("Seal of Righteousness", i+1, shared.SpellDataMin(spellData.SealOfRighteousness.ByRank(int32(i+1)).Direct), ours)
	}
	for i, rank := range sealOfTheCrusaderRanks {
		ours := rank.ap + grown(rank.level, rank.scaleLevel, rank.scale)
		near("Seal of the Crusader", i+1, spellData.SealOfTheCrusader.BySpellID(rank.spellID).Effects[0].Value, ours)
	}
}
