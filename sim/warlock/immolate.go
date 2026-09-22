package warlock

import (
	"strconv"
	"time"

	"github.com/wowsims/classic/sim/common/shared"
	"github.com/wowsims/classic/sim/core"
)

const ImmolateRanks = 8

// Beta client 1.60.1 values; ranks 1 and 2 lost their downranking penalty. Spell ID, cost, cast time,
// both coefficients and the dot (tick, 5 x 3 sec) come from the client table. The direct damage does
// not: the table truncates it, one below ours at ranks 1, 4 and 6.
var ImmolateBaseDamage = [ImmolateRanks + 1]float64{0, 11, 21, 38, 64, 80, 116, 146, 158}

func (warlock *Warlock) getImmolateConfig(rank int) core.SpellConfig {
	row := spellData.Immolate.ByRank(int32(rank))
	periodic := row.Periodic.(shared.SpellDataPeriodic)
	baseDamage := ImmolateBaseDamage[rank]
	dotDamage := periodic.Tick
	level := [ImmolateRanks + 1]int{0, 1, 10, 20, 30, 40, 50, 60, 60}[rank]

	return core.SpellConfig{
		SpellCode:      SpellCode_WarlockImmolate,
		ClassSpellMask: SpellMaskImmolate,
		ActionID:       core.ActionID{SpellID: row.SpellID},
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskSpellDamage,
		Flags:          core.SpellFlagAPL | core.SpellFlagResetAttackSwing | core.SpellFlagBinary | WarlockFlagDestruction,

		Rank:          rank,
		RequiredLevel: level,

		ManaCost: core.ManaCostOptions{
			FlatCost: float64(row.Cost),
		},
		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD:      core.GCDDefault,
				CastTime: row.CastTime,
			},
			ModifyCast: func(sim *core.Simulation, spell *core.Spell, cast *core.Cast) {
				cast.CastTime = spell.CastTime()
			},
			CastTime: func(spell *core.Spell) time.Duration {
				durationDecrease := time.Duration(0)
				return spell.DefaultCast.CastTime - durationDecrease
			},
		},

		DamageMultiplier: 1,
		ThreatMultiplier: 1,
		BonusCoefficient: roundCoef(row.Direct.BonusCoefficient()),

		Dot: core.DotConfig{
			Aura: core.Aura{
				Label: "Immolate-" + warlock.Label + strconv.Itoa(rank),
			},

			NumberOfTicks:    periodic.NumberOfTicks,
			TickLength:       periodic.TickLength,
			BonusCoefficient: roundCoef(periodic.Coef),

			OnSnapshot: func(sim *core.Simulation, target *core.Unit, dot *core.Dot, isRollover bool) {
				dot.Snapshot(target, dotDamage, isRollover)
			},
			OnTick: func(sim *core.Simulation, target *core.Unit, dot *core.Dot) {
				var result *core.SpellResult
				result = dot.CalcSnapshotDamage(sim, target, dot.OutcomeTick)
				dot.Spell.DealPeriodicDamage(sim, result)
			},
		},

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			// Aftermath: 10% initial damage per point (beta client curve). Its Daze half, 20% per point to
			// slow by a flat 50% for 5 sec, changes no damage and is not modelled.
			oldMultiplier := spell.DamageMultiplier
			spell.DamageMultiplier *= 1 + 0.1*float64(warlock.Talents.Aftermath)
			result := spell.CalcDamage(sim, target, baseDamage, spell.OutcomeMagicHitAndCrit)
			spell.DamageMultiplier = oldMultiplier

			if result.Landed() {
				dot := spell.Dot(target)
				dot.Apply(sim)
			}

			spell.DealDamage(sim, result)
		},
		ExpectedTickDamage: func(sim *core.Simulation, target *core.Unit, spell *core.Spell, useSnapshot bool) *core.SpellResult {
			if useSnapshot {
				dot := spell.Dot(target)
				return dot.CalcSnapshotDamage(sim, target, dot.Spell.OutcomeExpectedMagicAlwaysHit)
			} else {
				return spell.CalcPeriodicDamage(sim, target, baseDamage, spell.OutcomeExpectedMagicAlwaysHit)
			}
		},
	}
}

func (warlock *Warlock) getActiveImmolateSpell(target *core.Unit) *core.Spell {
	for _, immolateSpell := range warlock.Immolate {
		if immolateSpell.Dot(target).IsActive() {
			return immolateSpell
		}
	}
	return nil
}

func (warlock *Warlock) registerImmolateSpell() {
	warlock.Immolate = make([]*core.Spell, 0)

	maxRank := core.TernaryInt(core.IncludeAQ, ImmolateRanks, ImmolateRanks-1)
	for rank := 1; rank <= maxRank; rank++ {
		config := warlock.getImmolateConfig(rank)

		if config.RequiredLevel <= int(warlock.Level) {
			warlock.Immolate = append(warlock.Immolate, warlock.GetOrRegisterSpell(config))
		}
	}
}
