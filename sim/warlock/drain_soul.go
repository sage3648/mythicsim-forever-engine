package warlock

import (
	"strconv"

	"github.com/wowsims/classic/sim/common/shared"
	"github.com/wowsims/classic/sim/core"
)

const DrainSoulRanks = 4

func (warlock *Warlock) getDrainSoulBaseConfig(rank int) core.SpellConfig {
	// Beta client 1.60.1: spell ID, cost, school, tick, tick count and coefficient from the client table
	row := spellData.DrainSoul.ByRank(int32(rank))
	periodic := row.Periodic.(shared.SpellDataPeriodic)
	baseDamage := periodic.Tick
	level := [DrainSoulRanks + 1]int{0, 10, 24, 38, 52}[rank]

	return core.SpellConfig{
		SpellCode:      SpellCode_WarlockDrainSoul,
		ClassSpellMask: SpellMaskDrainSoul,
		ActionID:       core.ActionID{SpellID: row.SpellID},
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskSpellDamage,
		Flags:          core.SpellFlagAPL | core.SpellFlagChanneled | core.SpellFlagResetAttackSwing | WarlockFlagAffliction,

		RequiredLevel: level,
		Rank:          rank,

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

		Dot: core.DotConfig{
			Aura: core.Aura{
				Label: "DrainSoul-" + warlock.Label + strconv.Itoa(rank),
			},
			NumberOfTicks:    periodic.NumberOfTicks,
			TickLength:       periodic.TickLength,
			BonusCoefficient: roundCoef(periodic.Coef),

			OnSnapshot: func(sim *core.Simulation, target *core.Unit, dot *core.Dot, isRollover bool) {
				dot.Snapshot(target, baseDamage, isRollover)
			},
			OnTick: func(sim *core.Simulation, target *core.Unit, dot *core.Dot) {
				result := dot.CalcSnapshotDamage(sim, target, dot.OutcomeTick)
				result.Damage *= warlock.improvedDrainsMultiplier(sim, target)
				dot.Spell.DealPeriodicDamage(sim, result)
			},
		},

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			result := spell.CalcOutcome(sim, target, spell.OutcomeMagicHitNoHitCounter)
			if result.Landed() {

				dot := spell.Dot(target)
				dot.Apply(sim)
			}
			spell.DealOutcome(sim, result)
		},
		ExpectedTickDamage: func(sim *core.Simulation, target *core.Unit, spell *core.Spell, useSnapshot bool) *core.SpellResult {
			if useSnapshot {
				dot := spell.Dot(target)
				return dot.CalcSnapshotDamage(sim, target, spell.OutcomeExpectedMagicAlwaysHit)
			} else {
				return spell.CalcPeriodicDamage(sim, target, baseDamage, spell.OutcomeExpectedMagicAlwaysHit)
			}
		},
	}
}

func (warlock *Warlock) registerDrainSoulSpell() {
	warlock.DrainSoul = make([]*core.Spell, 0)
	for rank := 1; rank <= DrainSoulRanks; rank++ {
		config := warlock.getDrainSoulBaseConfig(rank)

		if config.RequiredLevel <= int(warlock.Level) {
			warlock.DrainSoul = append(warlock.DrainSoul, warlock.GetOrRegisterSpell(config))
		}
	}
}
