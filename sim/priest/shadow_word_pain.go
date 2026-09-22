package priest

import (
	"fmt"
	"math"

	"github.com/wowsims/classic/sim/common/shared"
	"github.com/wowsims/classic/sim/core"
)

// Spell ID, cost, cast time, cooldown, coefficient, school and dot ticks of the priest's spells come
// from the client table (spell_data_auto_gen.go, vendored from wowsims/forever). Damage ranges do not:
// the client rolls a spread the vendored generator drops, keeping only the truncated centre (see
// sim/mage/frostbolt.go). spell_damage_test.go checks every range kept here still contains it.
//
// Forever beta client 1.60.1.69893: less damage from rank 2 up, and .2 a tick at every rank.
const ShadowWordPainRanks = 8

// The client stores .429 as a float32; the table widens it. Rounding back to the stated value keeps
// the sim's numbers where they were (sim/mage/frostbolt.go). Every coefficient read from the table
// goes through this.
func roundCoef(coef float64) float64 {
	return math.Round(coef*1e6) / 1e6
}

var ShadowWordPainLevel = [ShadowWordPainRanks + 1]int{0, 4, 10, 18, 26, 34, 42, 50, 58}

//To Do: Check rollover code from runes

func (priest *Priest) registerShadowWordPainSpell() {
	priest.ShadowWordPain = make([]*core.Spell, ShadowWordPainRanks+1)

	for rank := 1; rank <= ShadowWordPainRanks; rank++ {
		config := priest.getShadowWordPainConfig(rank)

		if config.RequiredLevel <= int(priest.Level) {
			priest.ShadowWordPain[rank] = priest.GetOrRegisterSpell(config)
		}
	}
}

func (priest *Priest) getShadowWordPainConfig(rank int) core.SpellConfig {
	row := spellData.ShadowWordPain.ByRank(int32(rank))
	periodic := row.Periodic.(shared.SpellDataPeriodic)
	level := ShadowWordPainLevel[rank]

	return core.SpellConfig{
		SpellCode:      SpellCode_PriestShadowWordPain,
		ClassSpellMask: SpellMaskShadowWordPain,
		ActionID:       core.ActionID{SpellID: row.SpellID},
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskSpellDamage,
		Flags:          SpellFlagPriest | core.SpellFlagAPL | core.SpellFlagPureDot,

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
				Label: fmt.Sprintf("Shadow Word: Pain (Rank %d)", rank),
			},

			NumberOfTicks:    periodic.NumberOfTicks + (priest.Talents.ImprovedShadowWordPain),
			TickLength:       periodic.TickLength,
			BonusCoefficient: roundCoef(periodic.Coef),

			OnSnapshot: func(sim *core.Simulation, target *core.Unit, dot *core.Dot, isRollover bool) {
				dot.Snapshot(target, periodic.Tick, isRollover)
			},
			OnTick: func(sim *core.Simulation, target *core.Unit, dot *core.Dot) {
				dot.CalcAndDealPeriodicSnapshotDamage(sim, target, dot.OutcomeTick)
			},
		},

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			result := spell.CalcOutcome(sim, target, spell.OutcomeMagicHitNoHitCounter)

			if result.Landed() {
				priest.AddShadowWeavingStack(sim)
				spell.Dot(result.Target).Apply(sim)
			}
			spell.DealOutcome(sim, result)
		},
	}
}
