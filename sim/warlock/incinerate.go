package warlock

import (
	"github.com/wowsims/classic/sim/core"
)

// Beta client 1.60.1. Everything but the damage comes from the client table
// (see shadowbolt.go).
var IncinerateBaseDamage = [][]float64{{0}, {100, 114}, {146, 168}, {201, 233}}

func (warlock *Warlock) registerIncinerateSpell() {
	if !warlock.Talents.Incinerate {
		return
	}

	// Beta client 1.60.1: three ranks, learned at 40 (412758), 50 (1293812) and 60 (1293813), 2.5 sec
	// cast, 0.714 coefficient and 25% more damage on a target with Immolate. The highest rank the
	// character's level allows is the one registered.
	if warlock.Level < 40 {
		return
	}

	rank := map[int32]int{40: 1, 50: 2, 60: 3}[warlock.Level]
	if rank == 0 {
		return
	}
	row := spellData.Incinerate.ByRank(int32(rank))
	baseDamage := IncinerateBaseDamage[rank]

	warlock.Incinerate = warlock.RegisterSpell(core.SpellConfig{
		SpellCode:      SpellCode_WarlockIncinerate,
		ClassSpellMask: SpellMaskIncinerate,
		ActionID:       core.ActionID{SpellID: row.SpellID},
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskSpellDamage,
		Flags:          core.SpellFlagAPL | core.SpellFlagResetAttackSwing | WarlockFlagDestruction,
		RequiredLevel:  int(warlock.Level),
		Rank:           rank,
		MissileSpeed:   row.MissileSpeed,

		ManaCost: core.ManaCostOptions{
			FlatCost: float64(row.Cost),
		},
		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD:      core.GCDDefault,
				CastTime: row.CastTime,
			},
		},

		DamageMultiplier: 1,
		ThreatMultiplier: 1,
		BonusCoefficient: roundCoef(row.Direct.BonusCoefficient()),

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			damage := sim.Roll(baseDamage[0], baseDamage[1])
			if warlock.getActiveImmolateSpell(target) != nil {
				damage *= 1.25
			}

			result := spell.CalcDamage(sim, target, damage, spell.OutcomeMagicHitAndCrit)
			spell.WaitTravelTime(sim, func(sim *core.Simulation) {
				spell.DealDamage(sim, result)
			})
		},
	})
}
