package core

import (
	"math"
	"testing"
)

// The table from issue #252, checked against the rule rather than against itself. The
// tolerance is half a tenth because the logs record rage in tenths and so round.
func TestForeverWhiteHitRage(t *testing.T) {
	for _, c := range []struct {
		name      string
		speed     float64
		twoHand   bool
		measured  float64
		tolerance float64
	}{
		{"2.1s one-hand", 2.1, false, 7.25, 0.06},
		{"2.5s one-hand", 2.5, false, 8.65, 0.06},
		{"3.2s two-hand", 3.2, true, 14.4, 0.06},
		{"3.3s two-hand", 3.3, true, 14.9, 0.06},
		{"3.5s two-hand", 3.5, true, 15.7, 0.06},
	} {
		got := ForeverWhiteHitRage(&Weapon{SwingSpeed: c.speed, TwoHand: c.twoHand})
		if math.Abs(got-c.measured) > c.tolerance {
			t.Errorf("%s: rule gives %.2f, the beta logs measured %.2f", c.name, got, c.measured)
		}
	}

	// Damage cannot reach it at all, which is the entire point of the change.
	if ForeverWhiteHitRage(&Weapon{SwingSpeed: 2.6, BaseDamageMin: 10, BaseDamageMax: 20}) !=
		ForeverWhiteHitRage(&Weapon{SwingSpeed: 2.6, BaseDamageMin: 1000, BaseDamageMax: 2000}) {
		t.Error("rage moved when only the weapon's damage changed")
	}

	// A two-hander of the same speed is worth more, not the same.
	slow1H := ForeverWhiteHitRage(&Weapon{SwingSpeed: 3.3})
	slow2H := ForeverWhiteHitRage(&Weapon{SwingSpeed: 3.3, TwoHand: true})
	if slow2H <= slow1H {
		t.Errorf("two-hand %.2f should beat one-hand %.2f at the same speed", slow2H, slow1H)
	}

	if ForeverWhiteHitRage(nil) != 0 || ForeverWhiteHitRage(&Weapon{}) != 0 {
		t.Error("a weapon that does not swing should not pay rage")
	}
}

func TestForeverDamageTakenRage(t *testing.T) {
	// 1000 pre-armor damage into 5000 health: 1000 x 10 / 5000 = 2.
	if got := ForeverDamageTakenRage(1000, 5000); math.Abs(got-2) > 1e-9 {
		t.Errorf("got %.3f rage, want 2", got)
	}
	if ForeverDamageTakenRage(1000, 0) != 0 {
		t.Error("no health should not divide by zero")
	}
}

// Classic's damage-based rage and Forever's flat rage cross over somewhere, and where they
// cross is the whole story for a geared level 60: below it the sim was undershooting, above
// it the sim has been handing out rage the server does not.
func TestForeverRageCrossesClassicAtLevel60(t *testing.T) {
	const level = 60
	conversion := GetRageConversion(level)
	weapon := &Weapon{SwingSpeed: 2.6}
	flat := ForeverWhiteHitRage(weapon)
	breakEven := flat * conversion / 7.5

	if breakEven < 200 || breakEven > 400 {
		t.Errorf("break-even swing is %.0f damage, which is not the right order of magnitude", breakEven)
	}
	// A level 60 in raid gear swings for well over this, so Classic's rule pays more there.
	if classic := 800 * 7.5 / conversion; classic <= flat {
		t.Errorf("an 800 damage swing gives %.1f rage on Classic and %.1f flat; the sim was not over-generating after all", classic, flat)
	}
}
