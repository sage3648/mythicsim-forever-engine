package priest

import (
	"testing"

	"github.com/wowsims/classic/sim/common/shared"
)

// Our damage is kept beside the client table (see shadow_word_pain.go); it must not drift from it. The
// table has one value per rank, the centre of the range the game rolls, so ours must contain it.
func TestDamageContainsClientValue(t *testing.T) {
	ranged := map[string]struct {
		table shared.SpellDataTable
		heal  bool
		ours  [][]float64
	}{
		"MindBlast":       {spellData.MindBlast, false, MindBlastBaseDamage[:]},
		"Smite":           {spellData.Smite, false, SmiteBaseDamage[:]},
		"HolyFire":        {spellData.HolyFire, false, HolyFireBaseDamage[:]},
		"HolyNova":        {spellData.HolyNova, false, HolyNovaBaseDamage[:]},
		"HolyNovaHealing": {spellData.HolyNovaTriggered, true, HolyNovaBaseHealing[:]},
	}
	for name, spell := range ranged {
		for rank := 1; rank < len(spell.ours); rank++ {
			row := spell.table.ByRank(int32(rank))
			value := row.Direct
			if spell.heal {
				value = row.Heal
			}
			low, high := value.Range()
			if r := spell.ours[rank]; low < r[0] || high > r[1] {
				t.Errorf("%s rank %d: table %v-%v outside our %v-%v", name, rank, low, high, r[0], r[1])
			}
		}
	}

	// The table truncates Shadow Word: Death's level-scaled hit at level 60, so it may sit up to 1
	// below ours there.
	for rank := 1; rank <= ShadowWordDeathRanks; rank++ {
		level := min(60, ShadowWordDeathMaxLevel[rank])
		ours := ShadowWordDeathBaseDamage[rank] + ShadowWordDeathPerLevel[rank]*float64(level-ShadowWordDeathLevel[rank])
		table, _ := spellData.ShadowWordDeath.ByRank(int32(rank)).Direct.Range()
		if d := ours - table; d < 0 || d >= 1 {
			t.Errorf("Shadow Word: Death rank %d: table %v, ours %v", rank, table, ours)
		}
	}
}
