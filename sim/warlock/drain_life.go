package warlock

import (
	"strconv"

	"github.com/wowsims/classic/sim/common/shared"
	"github.com/wowsims/classic/sim/core"
)

const DrainLifeRanks = 6

func (warlock *Warlock) getDrainLifeBaseConfig(rank int) core.SpellConfig {
	// Beta client 1.60.1: spell ID, cost, school, tick, tick count and coefficient from the client table
	row := spellData.DrainLife.ByRank(int32(rank))
	periodic := row.Periodic.(shared.SpellDataPeriodic)
	baseDamage := periodic.Tick
	level := [DrainLifeRanks + 1]int{0, 14, 22, 30, 38, 46, 54}[rank]

	actionID := core.ActionID{SpellID: row.SpellID}

	healingSpell := warlock.GetOrRegisterSpell(core.SpellConfig{
		ActionID:    actionID.WithTag(1),
		SpellSchool: core.SpellSchoolPhysical,
		ProcMask:    core.ProcMaskSpellHealing,
		Flags:       core.SpellFlagPassiveSpell | core.SpellFlagHelpful,

		DamageMultiplier: 1,
		ThreatMultiplier: 0,
	})

	spellConfig := core.SpellConfig{
		ActionID:       actionID,
		SpellSchool:    row.SpellSchool,
		SpellCode:      SpellCode_WarlockDrainLife,
		ClassSpellMask: SpellMaskDrainLife,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskSpellDamage,
		Flags:          core.SpellFlagAPL | core.SpellFlagResetAttackSwing | WarlockFlagAffliction | core.SpellFlagChanneled,

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

		DamageMultiplierAdditive: 1,
		DamageMultiplier:         1,
		ThreatMultiplier:         1,

		Dot: core.DotConfig{
			Aura: core.Aura{
				Label: "DrainLife-" + warlock.Label + strconv.Itoa(rank),
			},
			NumberOfTicks:    periodic.NumberOfTicks,
			TickLength:       periodic.TickLength,
			BonusCoefficient: roundCoef(periodic.Coef),

			OnSnapshot: func(sim *core.Simulation, target *core.Unit, dot *core.Dot, isRollover bool) {
				dot.Snapshot(target, baseDamage, isRollover)
				// Drain Life heals so it snapshots target modifiers
				// Update 2024-06-29: It no longer snapshots on PTR
				// dot.SnapshotAttackerMultiplier *= dot.Spell.TargetDamageMultiplier(dot.Spell.Unit.AttackTables[target.UnitIndex][dot.Spell.CastType], true)
			},
			OnTick: func(sim *core.Simulation, target *core.Unit, dot *core.Dot) {
				result := dot.CalcSnapshotDamage(sim, target, dot.OutcomeTick)
				result.Damage *= warlock.improvedDrainsMultiplier(sim, target)
				dot.Spell.DealPeriodicDamage(sim, result)

				health := result.Damage
				healingSpell.CalcAndDealHealing(sim, healingSpell.Unit, health, healingSpell.OutcomeHealing)
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

	return spellConfig
}

func (warlock *Warlock) registerDrainLifeSpell() {
	warlock.DrainLife = make([]*core.Spell, 0)
	for rank := 1; rank <= DrainLifeRanks; rank++ {
		config := warlock.getDrainLifeBaseConfig(rank)

		if config.RequiredLevel <= int(warlock.Level) {
			warlock.DrainLife = append(warlock.DrainLife, warlock.GetOrRegisterSpell(config))
		}
	}
}
