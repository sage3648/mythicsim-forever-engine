package druid

import (
	"testing"

	"github.com/wowsims/classic/sim/common/shared"
)

// Our damage is kept beside the client table (see wrath.go); it must not drift from it. The table has one
// value per rank, the centre of the range the game rolls, so ours must contain it.
func TestDamageContainsClientValue(t *testing.T) {
	fb := make([][]float64, len(ferociousBiteRanks)+1)
	fb[0] = []float64{0}
	for i, r := range ferociousBiteRanks {
		fb[i+1] = []float64{r.dmgBase, r.dmgBase + r.dmgRange}
	}

	ranged := map[string]struct {
		table shared.SpellDataTable
		ours  [][]float64
	}{
		"Wrath":         {spellData.Wrath, WrathBaseDamage[:]},
		"Starfire":      {spellData.Starfire, StarfireBaseDamage[:]},
		"Moonfire":      {spellData.Moonfire, MoonfireBaseDamage[:]},
		"FerociousBite": {spellData.FerociousBite, fb},
	}
	for name, spell := range ranged {
		for rank := 1; rank < len(spell.ours); rank++ {
			low, high := spell.table.ByRank(int32(rank)).Direct.Range()
			if r := spell.ours[rank]; low < r[0] || high > r[1] {
				t.Errorf("%s rank %d: table %v-%v outside our %v-%v", name, rank, low, high, r[0], r[1])
			}
		}
	}

	// The table truncates Hurricane's level-scaled tick at 60, so it may sit up to 1 below ours there.
	for i, r := range hurricaneRanks {
		ours := r.damage + float64(min(60, r.scaleLevel)-r.level)*r.scale
		table := spellData.Hurricane.ByRank(int32(i + 1)).Periodic.(shared.SpellDataPeriodic).Tick
		if d := ours - table; d < 0 || d >= 1 {
			t.Errorf("Hurricane rank %d: table %v, ours %v", i+1, table, ours)
		}
	}
}
