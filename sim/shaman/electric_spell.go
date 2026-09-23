package shaman

import (
	"github.com/wowsims/classic/sim/common/shared"
	"github.com/wowsims/classic/sim/core"
)

// Totem Item IDs
const (
	StormfuryTotem           = 31031
	TotemOfAncestralGuidance = 32330
	TotemOfStorms            = 23199
	TotemOfTheVoid           = 28248
	TotemOfHex               = 40267
	VentureCoLightningRod    = 38361
	ThunderfallTotem         = 45255
)

// Shared precomputation logic for LB and CL. Cost, cast time, missile speed, school, defense type and
// coefficient come from the client table row; the caller builds the action id from it, where
// spell_sources_test.go can read it.
func (shaman *Shaman) newElectricSpellConfig(actionID core.ActionID, row shared.SpellData) core.SpellConfig {
	spell := core.SpellConfig{
		ActionID:     actionID,
		SpellSchool:  row.SpellSchool,
		DefenseType:  row.DefenseType,
		ProcMask:     core.ProcMaskSpellDamage,
		Flags:        SpellFlagShaman | SpellFlagLightning | core.SpellFlagAPL,
		MetricSplits: 6,
		MissileSpeed: row.MissileSpeed,

		ManaCost: core.ManaCostOptions{
			FlatCost:   float64(row.Cost),
			Multiplier: 100 - 2*shaman.Talents.Convection,
		},

		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				CastTime: row.CastTime - shaman.elementalAlacrityReduction(),
				GCD:      core.GCDDefault,
			},
			ModifyCast: func(sim *core.Simulation, spell *core.Spell, cast *core.Cast) {
				castTime := shaman.ApplyCastSpeedForSpell(cast.CastTime, spell)
				// Only a cast that runs past the next swing pushes it back; an instant (5 stack Maelstrom) one leaves the
				// swing timer alone, as on forever-next and upstream.
				if sim.CurrentTime+castTime > shaman.AutoAttacks.NextAttackAt() {
					shaman.AutoAttacks.StopMeleeUntil(sim, sim.CurrentTime+castTime, false)
				}
			},
		},

		BonusCritRating: core.TernaryFloat64(shaman.Talents.CallOfThunder, 3, 0) * core.SpellCritRatingPerCritChance,

		DamageMultiplier: 1,
		ThreatMultiplier: 1,
		BonusCoefficient: roundCoef(row.Direct.BonusCoefficient()),
	}

	return spell
}
