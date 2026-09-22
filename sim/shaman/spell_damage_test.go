package shaman

import (
	"testing"

	"github.com/wowsims/classic/sim/common/shared"
)

// Our damage is kept beside the client table (see lightning_bolt.go); it must not drift from it. The
// table has one value per rank, the centre of the range the game rolls, so ours must contain it.
func TestDamageContainsClientValue(t *testing.T) {
	ranged := map[string]struct {
		table shared.SpellDataTable
		ours  [][]float64
	}{
		"LightningBolt":  {spellData.LightningBolt, LightningBoltBaseDamage[:]},
		"ChainLightning": {spellData.ChainLightning, ChainLightningBaseDamage[:]},
		"EarthShock":     {spellData.EarthShock, EarthShockBaseDamage[:]},
		"FrostShock":     {spellData.FrostShock, FrostShockBaseDamage[:]},
		"LavaBurst":      {spellData.LavaBurst, LavaBurstBaseDamage[:]},
		"SearingTotem":   {spellData.SearingTotemTriggered, SearingTotemBaseDamage[:]},
	}
	for name, spell := range ranged {
		for rank := 1; rank < len(spell.ours); rank++ {
			low, high := spell.table.ByRank(int32(rank)).Direct.Range()
			if r := spell.ours[rank]; low < r[0] || high > r[1] {
				t.Errorf("%s rank %d: table %v-%v outside our %v-%v", name, rank, low, high, r[0], r[1])
			}
		}
	}

	// The table truncates Flame Shock's direct hit, so it may sit up to 1 below ours (rank 5: 136, ours 137).
	for rank := 1; rank <= FlameShockRanks; rank++ {
		table, _ := spellData.FlameShock.ByRank(int32(rank)).Direct.Range()
		if d := FlameShockBaseDamage[rank] - table; d < 0 || d > 1 {
			t.Errorf("FlameShock rank %d: table %v, ours %v", rank, table, FlameShockBaseDamage[rank])
		}
	}
}
