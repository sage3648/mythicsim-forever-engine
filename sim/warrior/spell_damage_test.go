package warrior

import "testing"

// Revenge's and Shield Slam's damage ranges are kept beside the client table (see revenge.go and
// shield_slam.go); they must not drift from it. The table holds the centre of the range the game
// rolls, so ours must centre on it.
func TestDamageRangesCentreOnClientValue(t *testing.T) {
	check := func(name string, low, high float64, spellID int32, table float64) {
		if mid := (low + high) / 2; mid != table {
			t.Errorf("%s %d: table %v, our centre %v", name, spellID, table, mid)
		}
	}
	for rank := 1; rank <= RevengeRanks; rank++ {
		spellID := RevengeSpellId[rank]
		low, _ := spellData.Revenge.BySpellID(spellID).Direct.Range()
		check("Revenge", RevengeBaseDamage[rank][0], RevengeBaseDamage[rank][1], spellID, low)
	}
	low, _ := spellData.ShieldSlam.BySpellID(23925).Direct.Range()
	check("Shield Slam", shieldSlamDamage[0], shieldSlamDamage[1], 23925, low)
}
