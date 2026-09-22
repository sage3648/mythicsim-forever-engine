package warlock

import (
	"github.com/wowsims/classic/sim/core"
)

const SearingPainRanks = 6

// Beta client 1.60.1 values; rank 1 lost its downranking penalty. Everything but the damage comes from
// the client table (see shadowbolt.go).
var SearingPainBaseDamage = [SearingPainRanks + 1][]float64{{0}, {24, 29}, {34, 40}, {45, 53}, {62, 73}, {84, 98}, {107, 126}}

func (warlock *Warlock) getSearingPainBaseConfig(rank int) core.SpellConfig {
	row := spellData.SearingPain.ByRank(int32(rank))
	baseDamage := SearingPainBaseDamage[rank]
	level := [SearingPainRanks + 1]int{0, 18, 26, 36, 42, 50, 58}[rank]

	return core.SpellConfig{
		SpellCode:      SpellCode_WarlockSearingPain,
		ClassSpellMask: SpellMaskSearingPain,
		ActionID:       core.ActionID{SpellID: row.SpellID},
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskSpellDamage,
		Flags:          core.SpellFlagAPL | core.SpellFlagResetAttackSwing | WarlockFlagDestruction,
		RequiredLevel:  level,
		Rank:           rank,

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
		ThreatMultiplier: 2,
		BonusCoefficient: roundCoef(row.Direct.BonusCoefficient()),

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			damage := sim.Roll(baseDamage[0], baseDamage[1])
			spell.CalcAndDealDamage(sim, target, damage, spell.OutcomeMagicHitAndCrit)
		},
	}
}

func (warlock *Warlock) registerSearingPainSpell() {
	warlock.SearingPain = make([]*core.Spell, 0)
	for rank := 1; rank <= SearingPainRanks; rank++ {
		config := warlock.getSearingPainBaseConfig(rank)

		if config.RequiredLevel <= int(warlock.Level) {
			warlock.SearingPain = append(warlock.SearingPain, warlock.GetOrRegisterSpell(config))
		}
	}
}
