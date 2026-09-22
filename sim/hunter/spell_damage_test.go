package hunter

import (
	"testing"

	"github.com/wowsims/classic/sim/common/shared"
)

// Our damage ranges are kept beside the client table (see aimed_shot.go); they must not drift from
// it. The table has one value per rank, the centre of the range the game rolls, so ours must contain it.
func TestDamageContainsClientValue(t *testing.T) {
	check := func(name string, rank int, value shared.SpellDataValue, ours []float64) {
		low, high := value.Range()
		if low < ours[0] || high > ours[1] {
			t.Errorf("%s rank %d: table %v-%v outside our %v-%v", name, rank, low, high, ours[0], ours[1])
		}
	}

	for rank := 1; rank < len(ExplosiveTrapBaseDamage); rank++ {
		check("Explosive Trap", rank, spellData.ExplosiveTrapEffect.ByRank(int32(rank)).Direct, ExplosiveTrapBaseDamage[rank])
	}

	pets := map[string]struct {
		table  shared.SpellDataTable
		ids    map[int32]int32
		damage map[int32][]float64
	}{
		"Claw":             {spellData.ClawTriggered, PetClawSpellID, PetClawDamage},
		"Bite":             {spellData.BiteTriggered, PetBiteSpellID, PetBiteDamage},
		"Lightning Breath": {spellData.LightningBreathTriggered, PetLightningBreathSpellID, PetLightningBreathDamage},
		"Screech":          {spellData.DemoralizingScreech, PetScreechSpellID, PetScreechDamage},
	}
	for name, pet := range pets {
		for level, id := range pet.ids {
			if ours, ok := pet.damage[level]; ok {
				row := pet.table.BySpellID(id)
				check(name, int(row.Rank), row.Direct, ours)
			}
		}
	}
}
