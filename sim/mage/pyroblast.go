package mage

import (
	"fmt"

	"github.com/wowsims/classic/sim/common/shared"
	"github.com/wowsims/classic/sim/core"
)

const PyroblastRanks = 8

// Spell ID, cost, cast time, missile speed and the whole dot (tick, ticks,
// coefficient) come from the client table (see
// frostbolt.go for why the damage does not).
//
// Beta client 1.60.1.69893. Both halves are lower than Classic at every rank. The 76 periodic damage
// the demo showed, once read as rank 1, is the client's rank 3.
var PyroblastBaseDamage = [PyroblastRanks + 1][]float64{{0}, {101, 131}, {126, 163}, {179, 228}, {230, 289}, {291, 364}, {368, 456}, {448, 555}, {520, 646}}
var PyroblastLevel = [PyroblastRanks + 1]int{0, 20, 24, 30, 36, 42, 48, 54, 60}

func (mage *Mage) registerPyroblastSpell() {
	if !mage.Talents.Pyroblast {
		return
	}

	mage.Pyroblast = make([]*core.Spell, PyroblastRanks+1)

	for rank := 1; rank <= PyroblastRanks; rank++ {
		config := mage.newPyroblastSpellConfig(rank)

		if config.RequiredLevel <= int(mage.Level) {
			mage.Pyroblast[rank] = mage.GetOrRegisterSpell(config)
		}
	}
}

func (mage *Mage) newPyroblastSpellConfig(rank int) core.SpellConfig {

	row := spellData.Pyroblast.ByRank(int32(rank))
	periodic := row.Periodic.(shared.SpellDataPeriodic)
	baseDamageLow := PyroblastBaseDamage[rank][0]
	baseDamageHigh := PyroblastBaseDamage[rank][1]
	level := PyroblastLevel[rank]

	actionID := core.ActionID{SpellID: row.SpellID}

	spellConfig := core.SpellConfig{
		ActionID:       actionID,
		SpellCode:      SpellCode_MagePyroblast,
		ClassSpellMask: SpellMaskPyroblast,
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskSpellDamage,
		Flags:          SpellFlagMage | core.SpellFlagAPL,
		MissileSpeed:   row.MissileSpeed,

		RequiredLevel: level,
		Rank:          rank,

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

		Dot: core.DotConfig{
			Aura: core.Aura{
				Label:    fmt.Sprintf("Pyroblast (Rank %d)", rank),
				ActionID: actionID.WithTag(1),
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

			spell.WaitTravelTime(sim, func(sim *core.Simulation) {
				spell.DealDamage(sim, result)

				if result.Landed() {
					spell.Dot(target).Apply(sim)
				}
			})
		},
	}

	return spellConfig
}
