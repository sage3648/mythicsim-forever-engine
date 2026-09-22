package mage

import (
	"github.com/wowsims/classic/sim/core"
)

// Damage bonus against a target the mage counts as frozen, i.e. one held by Fingers of Frost.
const IceLanceFrozenMultiplier = 4.0

func (mage *Mage) registerIceLanceSpell() {
	if !mage.Talents.IceLance {
		return
	}

	// Beta client 1.60.1.69893: rank 6 (1240047), learned at 56 and grown to its level 60 value. The
	// demo's 28 to 33 is the client's rank 1.
	// TODO: the client's damage effect carries no spell power coefficient at all, like the few other
	// spells whose coefficient moved off the effect row, so .143 is still a guess.
	// Cost, missile speed and school come from the client table; the damage is ours (the table has
	// the centre, 148).
	row := spellData.IceLance.ByRank(6)
	baseDamage := []float64{136, 161}
	spellCoeff := .143

	mage.IceLance = mage.RegisterSpell(core.SpellConfig{
		SpellCode:      SpellCode_MageIceLance,
		ClassSpellMask: SpellMaskIceLance,
		ActionID:       core.ActionID{SpellID: 30455},
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskSpellDamage,
		Flags:          SpellFlagMage | core.SpellFlagBinary | core.SpellFlagAPL,
		MissileSpeed:   row.MissileSpeed,

		RequiredLevel: 60,
		Rank:          1,

		ManaCost: core.ManaCostOptions{
			FlatCost: float64(row.Cost),
		},
		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD: core.GCDDefault,
			},
		},

		DamageMultiplier: 1,
		ThreatMultiplier: 1,
		BonusCoefficient: spellCoeff,

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			damage := sim.Roll(baseDamage[0], baseDamage[1])
			result := spell.CalcDamage(sim, target, damage, spell.OutcomeMagicHitAndCrit)

			// The tooltip reads as a bonus on the whole hit rather than on the base roll, so spell
			// power gets multiplied too.
			if mage.IsTargetFrozen() {
				result.Damage *= IceLanceFrozenMultiplier
			}

			spell.WaitTravelTime(sim, func(sim *core.Simulation) {
				spell.DealDamage(sim, result)
			})
		},
	})
}
