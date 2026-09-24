package warlock

import (
	"math"
	"testing"
)

// The branded hit is 1293696's formula: ((level - 26) * 1.5) + 14 to + 17, plus 7.8% of Shadow
// spell power.
func TestDemonicBrandHit(t *testing.T) {
	for _, c := range []struct {
		level     int32
		power     float64
		low, high float64
	}{
		{60, 0, 65, 68},
		{60, 500, 104, 107},
		{40, 0, 35, 38},
	} {
		low, high := demonicBrandHit(c.level, c.power)
		if math.Abs(low-c.low) > 1e-9 || math.Abs(high-c.high) > 1e-9 {
			t.Errorf("level %d, %v shadow spell power: %v to %v, want %v to %v", c.level, c.power, low, high, c.low, c.high)
		}
	}
}
