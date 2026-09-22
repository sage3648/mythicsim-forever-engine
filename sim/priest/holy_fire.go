package priest

import (
	"fmt"
	"time"

	"github.com/wowsims/classic/sim/common/shared"
	"github.com/wowsims/classic/sim/core"
)

const HolyFireRanks = 8

// Forever beta client 1.60.1.69893. The client's rank 5 dot (13 a tick) is larger than rank 6's (10 a tick); taken as it is.
// Everything but the direct damage comes from the client table (see shadow_word_pain.go).
var HolyFireBaseDamage = [HolyFireRanks + 1][]float64{{0}, {56, 71}, {64, 79}, {80, 98}, {90, 112}, {103, 127}, {133, 166}, {163, 206}, {184, 232}}
var HolyFireLevel = [HolyFireRanks + 1]int{0, 20, 24, 30, 36, 42, 48, 54, 60}

func (priest *Priest) registerHolyFire() {
	priest.HolyFire = make([]*core.Spell, HolyFireRanks+1)

	for rank := 1; rank <= HolyFireRanks; rank++ {
		config := priest.getHolyFireConfig(rank)

		if config.RequiredLevel <= int(priest.Level) {
			priest.HolyFire[rank] = priest.GetOrRegisterSpell(config)
		}
	}
}

func (priest *Priest) getHolyFireConfig(rank int) core.SpellConfig {
	row := spellData.HolyFire.ByRank(int32(rank))
	periodic := row.Periodic.(shared.SpellDataPeriodic)
	baseDamageLow := HolyFireBaseDamage[rank][0]
	baseDamageHigh := HolyFireBaseDamage[rank][1]
	level := HolyFireLevel[rank]

	return core.SpellConfig{
		SpellCode:      SpellCode_PriestHolyFire,
		ClassSpellMask: SpellMaskHolyFire,
		ActionID:       core.ActionID{SpellID: row.SpellID},
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskSpellDamage,
		Flags:          SpellFlagPriest | core.SpellFlagAPL,

		RequiredLevel: level,
		Rank:          rank,

		ManaCost: core.ManaCostOptions{
			FlatCost: float64(row.Cost),
		},

		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD:      core.GCDDefault,
				CastTime: row.CastTime - time.Millisecond*100*time.Duration(priest.Talents.DivineFury),
			},
		},

		BonusCoefficient: roundCoef(row.Direct.BonusCoefficient()),

		DamageMultiplier: 1,
		ThreatMultiplier: 1,

		Dot: core.DotConfig{
			Aura: core.Aura{
				Label: fmt.Sprintf("Holy Fire (Rank %d)", rank),
			},

			NumberOfTicks:    periodic.NumberOfTicks,
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
			baseDamage := sim.Roll(baseDamageLow, baseDamageHigh)
			result := spell.CalcDamage(sim, target, baseDamage, spell.OutcomeMagicHitAndCrit)
			if result.Landed() {
				spell.Dot(target).Apply(sim)
			}
			spell.DealDamage(sim, result)
		},
	}
}
