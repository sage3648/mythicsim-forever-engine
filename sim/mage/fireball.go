package mage

import (
	"fmt"
	"time"

	"github.com/wowsims/classic/sim/common/shared"
	"github.com/wowsims/classic/sim/core"
)

const FireballRanks = 12

// Spell ID, cost, cast time, coefficient, missile speed and the dot's total come from the client
// table (see frostbolt.go for why the direct damage does not).
//
// Beta client 1.60.1.69893. Damage is the client's base plus its per level growth up to the rank's
// max level (capped at 60), the same way the Classic numbers were read. Forever lowered every rank
// from 2 up and dropped the downranking penalty from the coefficients. The dot is the client's per
// tick damage times its tick count; the sim spreads that total over 4 ticks at every rank.
var FireballBaseDamage = [FireballRanks + 1][]float64{{0}, {16, 25}, {32, 47}, {48, 65}, {66, 91}, {98, 130}, {138, 183}, {171, 222}, {212, 275}, {271, 348}, {338, 431}, {397, 505}, {425, 541}}
var FireballLevel = [FireballRanks + 1]int{0, 1, 6, 12, 18, 24, 30, 36, 42, 48, 54, 60, 60}

func (mage *Mage) registerFireballSpell() {
	mage.Fireball = make([]*core.Spell, FireballRanks+1)

	maxRank := core.TernaryInt(core.IncludeAQ, FireballRanks, FireballRanks-1)
	for rank := 1; rank <= maxRank; rank++ {
		config := mage.newFireballSpellConfig(rank)

		if config.RequiredLevel <= int(mage.Level) {
			mage.Fireball[rank] = mage.GetOrRegisterSpell(config)
		}
	}
}

func (mage *Mage) newFireballSpellConfig(rank int) core.SpellConfig {
	numTicks := int32(4)
	tickLength := time.Second * 2

	row := spellData.Fireball.ByRank(int32(rank))
	periodic := row.Periodic.(shared.SpellDataPeriodic)
	baseDamageLow := FireballBaseDamage[rank][0]
	baseDamageHigh := FireballBaseDamage[rank][1]
	// The client ticks ranks 1-3 2, 3 and 3 times; the sim keeps 4 ticks at every rank, same total.
	baseDotDamage := periodic.Tick * float64(periodic.NumberOfTicks) / float64(numTicks)
	level := FireballLevel[rank]

	actionID := core.ActionID{SpellID: row.SpellID}

	return core.SpellConfig{
		ActionID:       actionID,
		SpellCode:      SpellCode_MageFireball,
		ClassSpellMask: SpellMaskFireball,
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskSpellDamage,
		Flags:          core.SpellFlagAPL | SpellFlagMage,
		MissileSpeed:   row.MissileSpeed,

		RequiredLevel: level,
		Rank:          rank,

		ManaCost: core.ManaCostOptions{
			FlatCost: float64(row.Cost),
		},
		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD:      core.GCDDefault,
				CastTime: row.CastTime - time.Millisecond*100*time.Duration(mage.Talents.ImprovedFireball),
			},
		},

		Dot: core.DotConfig{
			Aura: core.Aura{
				Label:    fmt.Sprintf("Fireball (Rank %d)", rank),
				ActionID: actionID.WithTag(1),
			},
			NumberOfTicks: numTicks,
			TickLength:    tickLength,
			OnSnapshot: func(sim *core.Simulation, target *core.Unit, dot *core.Dot, isRollover bool) {
				dot.Snapshot(target, baseDotDamage, isRollover)
			},
			OnTick: func(sim *core.Simulation, target *core.Unit, dot *core.Dot) {
				dot.CalcAndDealPeriodicSnapshotDamage(sim, target, dot.OutcomeTick)
			},
		},

		DamageMultiplier: 1,
		ThreatMultiplier: 1,
		BonusCoefficient: roundCoef(row.Direct.BonusCoefficient()),

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
}
