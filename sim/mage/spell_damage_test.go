package mage

import (
	"testing"

	"github.com/wowsims/classic/sim/common/shared"
)

// Our damage is kept beside the client table (see frostbolt.go); it must not drift from it. The
// table has one value per rank, the centre of the range the game rolls, so ours must contain it.
func TestDamageContainsClientValue(t *testing.T) {
	ranged := map[string]struct {
		table shared.SpellDataTable
		ours  [][]float64
	}{
		"Frostbolt":       {spellData.Frostbolt, FrostboltBaseDamage[:]},
		"Fireball":        {spellData.Fireball, FireballBaseDamage[:]},
		"Scorch":          {spellData.Scorch, ScorchBaseDamage[:]},
		"FireBlast":       {spellData.FireBlast, FireBlastBaseDamage[:]},
		"Pyroblast":       {spellData.Pyroblast, PyroblastBaseDamage[:]},
		"ArcaneExplosion": {spellData.ArcaneExplosion, ArcaneExplosionBaseDamage[:]},
		"BlastWave":       {spellData.BlastWave, BlastWaveBaseDamage[:]},
		"Flamestrike":     {spellData.Flamestrike, FlamestrikeBaseDamage[:]},
	}
	for name, spell := range ranged {
		for rank := 1; rank < len(spell.ours); rank++ {
			low, high := spell.table.ByRank(int32(rank)).Direct.Range()
			if r := spell.ours[rank]; low < r[0] || high > r[1] {
				t.Errorf("%s rank %d: table %v-%v outside our %v-%v", name, rank, low, high, r[0], r[1])
			}
		}
	}

	// The table truncates the missile's level-scaled value, so it may sit up to 1 below ours.
	for rank := 1; rank <= ArcaneMissilesRanks; rank++ {
		table, _ := spellData.ArcaneMissilesTriggered.ByRank(int32(rank)).Direct.Range()
		if d := ArcaneMissilesBaseTickDamage[rank] - table; d < 0 || d > 1 {
			t.Errorf("Arcane Missiles rank %d: table %v, ours %v", rank, table, ArcaneMissilesBaseTickDamage[rank])
		}
	}
}
