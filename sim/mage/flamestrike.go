package mage

import (
	"fmt"

	"github.com/wowsims/classic/sim/common/shared"
	"github.com/wowsims/classic/sim/core"
)

const FlamestrikeRanks = 6

var FlamestrikeBaseDamage = [FlamestrikeRanks + 1][]float64{{0}, {55, 71}, {96, 123}, {159, 197}, {220, 272}, {294, 362}, {381, 466}}

// Spell ID, cost, cast time, coefficient and the whole burn (tick, ticks,
// coefficient) come from the client table (see
// frostbolt.go for why the damage does not).
//
// Beta client 1.60.1.69893. The burn is now an area trigger casting a damage spell every 2 sec
// (1279983 ... 1279990), 4 times; the dot damage is 4 times that spell's base and the dot
// coefficient is that spell's, per tick, up from Classic's .02.
var FlamestrikeLevel = [FlamestrikeRanks + 1]int{0, 16, 24, 32, 40, 48, 56}

func (mage *Mage) registerFlamestrikeSpell() {
	mage.Flamestrike = make([]*core.Spell, FlamestrikeRanks+1)

	for rank := 1; rank <= FlamestrikeRanks; rank++ {
		config := mage.newFlamestrikeSpellConfig(rank)

		if config.RequiredLevel <= int(mage.Level) {
			mage.Flamestrike[rank] = mage.GetOrRegisterSpell(config)
		}
	}
}

func (mage *Mage) newFlamestrikeSpellConfig(rank int) core.SpellConfig {
	row := spellData.Flamestrike.ByRank(int32(rank))
	periodic := row.Periodic.(shared.SpellDataPeriodic)
	baseDamageLow := FlamestrikeBaseDamage[rank][0]
	baseDamageHigh := FlamestrikeBaseDamage[rank][1]
	level := FlamestrikeLevel[rank]

	return core.SpellConfig{
		ActionID:       core.ActionID{SpellID: row.SpellID},
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskSpellDamage,
		Flags:          SpellFlagMage | core.SpellFlagAPL,
		SpellCode:      SpellCode_MageFlamestrike,
		ClassSpellMask: SpellMaskFlamestrike,

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

		BonusCritRating: float64(5 * mage.Talents.ImprovedFlamestrike * core.CritRatingPerCritChance),

		DamageMultiplier: 1,
		ThreatMultiplier: 1,
		BonusCoefficient: roundCoef(row.Direct.BonusCoefficient()),

		Dot: core.DotConfig{
			IsAOE: true,
			Aura: core.Aura{
				Label: fmt.Sprintf("Flamestrike (Rank %d)", rank),
			},
			NumberOfTicks:    periodic.NumberOfTicks,
			TickLength:       periodic.TickLength,
			BonusCoefficient: roundCoef(periodic.Coef),
			OnSnapshot: func(sim *core.Simulation, target *core.Unit, dot *core.Dot, isRollover bool) {
				dot.Snapshot(target, periodic.Tick, isRollover)
			},
			OnTick: func(sim *core.Simulation, target *core.Unit, dot *core.Dot) {
				for _, aoeTarget := range sim.Encounter.TargetUnits {
					dot.CalcAndDealPeriodicSnapshotDamage(sim, aoeTarget, dot.OutcomeTick)
				}
			},
		},

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			for _, aoeTarget := range sim.Encounter.TargetUnits {
				baseDamage := sim.Roll(baseDamageLow, baseDamageHigh)
				spell.CalcAndDealDamage(sim, aoeTarget, baseDamage, spell.OutcomeMagicCrit)
			}
			spell.AOEDot().Apply(sim)
		},
	}
}
