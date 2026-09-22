package druid

import (
	"time"

	"github.com/wowsims/classic/sim/core"
)

const StarfireRanks = 7

// Beta client 1.60.1.69893: about 30% less damage at every rank from 2 up (rank 7 496-584 -> 350-412). Costs and the
// 3.5 sec cast are Classic's. Everything but the damage comes from the client table (see wrath.go).
var StarfireBaseDamage = [StarfireRanks + 1][]float64{{0}, {79, 96}, {108, 130}, {140, 167}, {191, 226}, {257, 302}, {313, 370}, {350, 412}}
var StarfireLevel = [StarfireRanks + 1]int{0, 20, 26, 34, 42, 50, 58, 60}

func (druid *Druid) registerStarfireSpell() {
	druid.Starfire = make([]*DruidSpell, StarfireRanks+1)

	maxRank := core.TernaryInt(core.IncludeAQ, StarfireRanks, StarfireRanks-1)
	for rank := 1; rank <= maxRank; rank++ {
		config := druid.newStarfireSpellConfig(rank)

		if config.RequiredLevel <= int(druid.Level) {
			druid.Starfire[rank] = druid.RegisterSpell(Humanoid|Moonkin, config)
		}
	}
}

func (druid *Druid) newStarfireSpellConfig(rank int) core.SpellConfig {
	row := spellData.Starfire.ByRank(int32(rank))
	baseDamageLow := StarfireBaseDamage[rank][0]
	baseDamageHigh := StarfireBaseDamage[rank][1]
	level := StarfireLevel[rank]

	return core.SpellConfig{
		ActionID:       core.ActionID{SpellID: row.SpellID},
		SpellCode:      SpellCode_DruidStarfire,
		ClassSpellMask: SpellMaskStarfire,
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
				CastTime: row.CastTime - time.Millisecond*100*time.Duration(druid.Talents.ImprovedStarfire),
			},
		},

		DamageMultiplier: 1,
		ThreatMultiplier: 1,
		BonusCoefficient: roundCoef(row.Direct.BonusCoefficient()),

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			baseDamage := sim.Roll(baseDamageLow, baseDamageHigh)
			spell.CalcAndDealDamage(sim, target, baseDamage, spell.OutcomeMagicHitAndCrit)
		},
	}
}
