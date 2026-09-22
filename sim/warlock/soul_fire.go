package warlock

import (
	"time"

	"github.com/wowsims/classic/sim/core"
)

const SoulFireRanks = 2

// Beta client 1.60.1 values. Everything but the damage comes from the client table (see shadowbolt.go).
var SoulFireBaseDamage = [SoulFireRanks + 1][]float64{{0, 0}, {344, 430}, {390, 487}}

func (warlock *Warlock) getSoulFireBaseConfig(rank int) core.SpellConfig {
	row := spellData.SoulFire.ByRank(int32(rank))
	baseDamage := SoulFireBaseDamage[rank]
	level := [SoulFireRanks + 1]int{0, 48, 56}[rank]

	config := core.SpellConfig{
		SpellCode:      SpellCode_WarlockSoulFire,
		ClassSpellMask: SpellMaskSoulFire,
		ActionID:       core.ActionID{SpellID: row.SpellID},
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskSpellDamage,
		Flags:          core.SpellFlagAPL | core.SpellFlagResetAttackSwing | WarlockFlagDestruction,
		RequiredLevel:  level,
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
			results := spell.CalcDamage(sim, target, damage, spell.OutcomeMagicHitAndCrit)
			spell.WaitTravelTime(sim, func(s *core.Simulation) {
				spell.DealDamage(sim, results)
			})
		},
	}

	// Decimation: 45% per point in the beta client, so 2/2 leaves Soul Fire a six second cooldown
	cooldownReduction := 0.45 * float64(warlock.Talents.Decimation)

	config.Cast.CD = core.Cooldown{
		Timer:    warlock.NewTimer(),
		Duration: time.Duration(float64(row.Cooldown) * (1 - cooldownReduction)),
	}

	return config
}

func (warlock *Warlock) registerSoulFireSpell() {
	warlock.SoulFire = make([]*core.Spell, 0)
	for rank := 1; rank <= SoulFireRanks; rank++ {
		config := warlock.getSoulFireBaseConfig(rank)

		if config.RequiredLevel <= int(warlock.Level) {
			warlock.SoulFire = append(warlock.SoulFire, warlock.GetOrRegisterSpell(config))
		}
	}
}
