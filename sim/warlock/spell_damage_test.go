package warlock

import (
	"testing"

	"github.com/wowsims/classic/sim/common/shared"
)

// Our damage is kept beside the client table (see shadowbolt.go); it must not drift from it. The
// table has one value per rank, the centre of the range the game rolls, so ours must contain it.
func TestDamageContainsClientValue(t *testing.T) {
	ranged := map[string]struct {
		table shared.SpellDataTable
		ours  [][]float64
	}{
		"ShadowBolt":  {spellData.ShadowBolt, ShadowBoltBaseDamage[:]},
		"SearingPain": {spellData.SearingPain, SearingPainBaseDamage[:]},
		"SoulFire":    {spellData.SoulFire, SoulFireBaseDamage[:]},
		"Incinerate":  {spellData.Incinerate, IncinerateBaseDamage},
		"Conflagrate": {spellData.Conflagrate, ConflagrateBaseDamage[:]},
		"Shadowburn":  {spellData.Shadowburn, ShadowburnBaseDamage[:]},
	}
	for name, spell := range ranged {
		for rank := 1; rank < len(spell.ours); rank++ {
			low, high := spell.table.ByRank(int32(rank)).Direct.Range()
			if r := spell.ours[rank]; low < r[0] || high > r[1] {
				t.Errorf("%s rank %d: table %v-%v outside our %v-%v", name, rank, low, high, r[0], r[1])
			}
		}
	}

	// The table truncates Immolate's level-scaled hit, so it may sit up to 1 below ours.
	for rank := 1; rank <= ImmolateRanks; rank++ {
		table, _ := spellData.Immolate.ByRank(int32(rank)).Direct.Range()
		if d := ImmolateBaseDamage[rank] - table; d < 0 || d > 1 {
			t.Errorf("Immolate rank %d: table %v, ours %v", rank, table, ImmolateBaseDamage[rank])
		}
	}
}
