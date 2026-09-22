package rogue

import "testing"

// Eviscerate's damage is kept beside the client table (see eviscerate.go); it must not drift from it.
// The table holds the centre of the range the game rolls with no combo points, so ours must centre on it.
func TestEviscerateCentresOnClientValue(t *testing.T) {
	for spellID, ours := range eviscerateDamage {
		low, high := spellData.Eviscerate.BySpellID(spellID).Direct.Range()
		if mid := ours.flat + ours.variance/2; low != mid || high != mid {
			t.Errorf("Eviscerate %d: table %v-%v, our centre %v", spellID, low, high, mid)
		}
	}
}
