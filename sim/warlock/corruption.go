package warlock

import (
	"strconv"
	"time"

	"github.com/wowsims/classic/sim/common/shared"
	"github.com/wowsims/classic/sim/core"
)

const CorruptionRanks = 7

func (warlock *Warlock) getCorruptionConfig(rank int) core.SpellConfig {
	// Beta client 1.60.1: 0.2 per tick at every rank, and the damage roughly halved. Spell ID, cost,
	// cast time, tick, tick count and coefficient come from the client table.
	row := spellData.Corruption.ByRank(int32(rank))
	periodic := row.Periodic.(shared.SpellDataPeriodic)
	ticks := periodic.NumberOfTicks
	baseDamage := periodic.Tick
	level := [CorruptionRanks + 1]int{0, 4, 14, 24, 34, 44, 54, 60}[rank]

	castTime := row.CastTime - 400*time.Millisecond*time.Duration(warlock.Talents.ImprovedCorruption)

	return core.SpellConfig{
		ActionID:       core.ActionID{SpellID: row.SpellID},
		SpellSchool:    row.SpellSchool,
		SpellCode:      SpellCode_WarlockCorruption,
		ClassSpellMask: SpellMaskCorruption,
		ProcMask:       core.ProcMaskSpellDamage,
		DefenseType:    row.DefenseType,
		Flags:          core.SpellFlagAPL | core.SpellFlagResetAttackSwing | core.SpellFlagPureDot | WarlockFlagAffliction,
		Rank:           rank,
		RequiredLevel:  level,

		ManaCost: core.ManaCostOptions{
			FlatCost: float64(row.Cost),
		},
		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				CastTime: castTime,
				GCD:      core.GCDDefault,
			},
		},

		CritDamageBonus: 0,

		DamageMultiplierAdditive: 1 + 0.02*float64(warlock.Talents.ImprovedCorruption),
		DamageMultiplier:         1,
		ThreatMultiplier:         1,

		Dot: core.DotConfig{
			Aura: core.Aura{
				Label: "Corruption-" + warlock.Label + strconv.Itoa(rank),
			},

			NumberOfTicks:    ticks,
			TickLength:       periodic.TickLength,
			BonusCoefficient: roundCoef(periodic.Coef),

			OnSnapshot: func(sim *core.Simulation, target *core.Unit, dot *core.Dot, isRollover bool) {
				dot.Snapshot(target, baseDamage, isRollover)
			},
			OnTick: func(sim *core.Simulation, target *core.Unit, dot *core.Dot) {
				dot.CalcAndDealPeriodicSnapshotDamage(sim, target, dot.OutcomeTick)
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
				return dot.CalcSnapshotDamage(sim, target, dot.Spell.OutcomeExpectedMagicAlwaysHit)
			} else {
				baseDamage := baseDamage / float64(ticks)
				return spell.CalcPeriodicDamage(sim, target, baseDamage, spell.OutcomeExpectedMagicAlwaysHit)
			}
		},
	}
}

func (warlock *Warlock) registerCorruptionSpell() {
	warlock.Corruption = make([]*core.Spell, 0)

	maxRank := core.TernaryInt(core.IncludeAQ, CorruptionRanks, CorruptionRanks-1)
	for rank := 1; rank <= maxRank; rank++ {
		config := warlock.getCorruptionConfig(rank)

		if config.RequiredLevel <= int(warlock.Level) {
			warlock.Corruption = append(warlock.Corruption, warlock.GetOrRegisterSpell(config))
		}
	}
}
