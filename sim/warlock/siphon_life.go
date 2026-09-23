package warlock

import (
	"strconv"

	"github.com/wowsims/classic/sim/common/shared"
	"github.com/wowsims/classic/sim/core"
)

const SiphonLifeRanks = 4

func (warlock *Warlock) getSiphonLifeBaseConfig(rank int) core.SpellConfig {
	// Beta client 1.60.1: spell ID, cost, school, tick, tick count and coefficient from the client table
	row := spellData.SiphonLife.ByRank(int32(rank))
	periodic := row.Periodic.(shared.SpellDataPeriodic)
	baseDamage := periodic.Tick
	level := [SiphonLifeRanks + 1]int{0, 0, 38, 48, 58}[rank]

	actionID := core.ActionID{SpellID: row.SpellID}
	healthMetrics := warlock.NewHealthMetrics(actionID)

	return core.SpellConfig{
		SpellCode:      SpellCode_WarlockSiphonLife,
		ClassSpellMask: SpellMaskSiphonLife,
		ActionID:       actionID,
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskSpellDamage,
		Flags:          core.SpellFlagAPL | core.SpellFlagResetAttackSwing | core.SpellFlagBinary | WarlockFlagAffliction | core.SpellFlagNoPeriodicCrit, // client 18265-18881: no Periodic Can Crit
		RequiredLevel:  level,
		Rank:           rank,

		ManaCost: core.ManaCostOptions{
			FlatCost: float64(row.Cost),
		},
		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD: core.GCDDefault,
			},
		},

		CritDamageBonus: 0,

		DamageMultiplierAdditive: 1,
		DamageMultiplier:         1,
		ThreatMultiplier:         1,

		Dot: core.DotConfig{
			Aura: core.Aura{
				Label: "SiphonLife-" + warlock.Label + strconv.Itoa(rank),
			},
			NumberOfTicks:       periodic.NumberOfTicks,
			TickLength:          periodic.TickLength,
			AffectedByCastSpeed: false,
			BonusCoefficient:    roundCoef(periodic.Coef),

			OnSnapshot: func(sim *core.Simulation, target *core.Unit, dot *core.Dot, isRollover bool) {
				dot.Snapshot(target, baseDamage, isRollover)

				if !isRollover {
					// Siphon Life heals so it snapshots target modifiers
					dot.SnapshotAttackerMultiplier *= dot.Spell.TargetDamageMultiplier(dot.Spell.Unit.AttackTables[target.UnitIndex][dot.Spell.CastType], true)
				}
			},
			OnTick: func(sim *core.Simulation, target *core.Unit, dot *core.Dot) {
				// TODO: interaction with bonus damage taken?
				// Remove target modifiers for the tick only
				dot.Spell.Flags |= core.SpellFlagIgnoreTargetModifiers

				result := dot.CalcAndDealPeriodicSnapshotDamage(sim, target, dot.OutcomeTick)

				// revert flag changes
				dot.Spell.Flags ^= core.SpellFlagIgnoreTargetModifiers

				health := result.Damage
				warlock.GainHealth(sim, health, healthMetrics)
			},
		},

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			result := spell.CalcOutcome(sim, target, spell.OutcomeMagicHitNoHitCounter)
			if result.Landed() {
				dot := spell.Dot(target)
				dot.Apply(sim)
			}
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

func (warlock *Warlock) registerSiphonLifeSpell() {
	if !warlock.Talents.SiphonLife {
		return
	}

	warlock.SiphonLife = make([]*core.Spell, 0)
	for rank := 1; rank <= SiphonLifeRanks; rank++ {
		config := warlock.getSiphonLifeBaseConfig(rank)

		if config.RequiredLevel <= int(warlock.Level) {
			warlock.SiphonLife = append(warlock.SiphonLife, warlock.GetOrRegisterSpell(config))
		}
	}
}
