package druid

import (
	"fmt"

	"github.com/wowsims/classic/sim/common/shared"
	"github.com/wowsims/classic/sim/core"
)

const MoonfireRanks = 10

// Beta client 1.60.1.69893: every rank carries the full .15 direct and .13 per tick coefficients, and the damage is
// lower from rank 2 up (rank 10 195-228 plus 96 a tick -> 128-151 plus 60 a tick). Everything but the direct damage
// comes from the client table (see wrath.go), the dot as the client's tick and tick count.
var MoonfireBaseDamage = [MoonfireRanks + 1][]float64{{0}, {9, 12}, {15, 19}, {25, 29}, {36, 43}, {50, 59}, {62, 72}, {75, 88}, {96, 113}, {112, 131}, {128, 151}}
var MoonfireLevel = [MoonfireRanks + 1]int{0, 4, 10, 16, 22, 28, 34, 40, 46, 52, 58}

func (druid *Druid) registerMoonfireSpell() {
	druid.Moonfire = make([]*DruidSpell, 0)

	for rank := 1; rank <= MoonfireRanks; rank++ {
		config := druid.getMoonfireBaseConfig(rank)

		if config.RequiredLevel <= int(druid.Level) {
			druid.Moonfire = append(druid.Moonfire, druid.RegisterSpell(Humanoid|Moonkin, config))
		}
	}
}

func (druid *Druid) getMoonfireBaseConfig(rank int) core.SpellConfig {
	row := spellData.Moonfire.ByRank(int32(rank))
	periodic := row.Periodic.(shared.SpellDataPeriodic)
	baseDamageLow := MoonfireBaseDamage[rank][0]
	baseDamageHigh := MoonfireBaseDamage[rank][1]
	level := MoonfireLevel[rank]

	return core.SpellConfig{
		ActionID:       core.ActionID{SpellID: row.SpellID},
		SpellCode:      SpellCode_DruidMoonfire,
		ClassSpellMask: SpellMaskMoonfire,
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskSpellDamage,
		Flags:          core.SpellFlagAPL | core.SpellFlagResetAttackSwing,

		RequiredLevel: level,
		Rank:          rank,

		ManaCost: core.ManaCostOptions{
			FlatCost: float64(row.Cost),
		},
		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD:      core.GCDDefault,
				CastTime: 0,
			},
		},
		Dot: core.DotConfig{
			Aura: core.Aura{
				Label:    fmt.Sprintf("Moonfire (Rank %d)", rank),
				ActionID: core.ActionID{SpellID: row.SpellID},
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

		BonusCoefficient: roundCoef(row.Direct.BonusCoefficient()),

		DamageMultiplier: 1,
		ThreatMultiplier: 1,

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			baseDamage := sim.Roll(baseDamageLow, baseDamageHigh)
			result := spell.CalcAndDealDamage(sim, target, baseDamage, spell.OutcomeMagicHitAndCrit)

			if result.Landed() {
				dot := spell.Dot(target)
				dot.Apply(sim)
			}
		},
	}
}
