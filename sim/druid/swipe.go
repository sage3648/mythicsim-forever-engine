package druid

import (
	"github.com/wowsims/classic/sim/core"
)

const SwipeRanks = 5

// The id, cost and damage come from the client table (see wrath.go). The client does not carry threat, so the
// multiplier below stays ours.
var SwipeLevel = [SwipeRanks + 1]int{0, 16, 24, 34, 44, 54}

// See https://www.wowhead.com/classic/spell=436895/s03-tuning-and-overrides-passive-druid
// Modifies Threat +101%:
const SwipeThreatMultiplier = 2.0

func (druid *Druid) registerSwipeBearSpell() {
	rank := map[int32]int{
		25: 2,
		40: 3,
		50: 4,
		60: 5,
	}[druid.Level]

	level := SwipeLevel[rank]
	row := spellData.Swipe.ByRank(int32(rank))
	baseDamage, _ := row.Direct.Range()

	rageCost := float64(row.Cost) - float64(druid.Talents.Ferocity)
	numHits := min(3, druid.Env.GetNumTargets())
	results := make([]*core.SpellResult, numHits)

	switch druid.Ranged().ID {
	case IdolOfBrutality:
		rageCost -= 3
	}

	druid.SwipeBear = druid.RegisterSpell(Bear, core.SpellConfig{
		SpellCode:      SpellCode_DruidSwipe,
		ClassSpellMask: SpellMaskSwipe,
		ActionID:       core.ActionID{SpellID: row.SpellID},
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskMeleeMHSpecial,
		Flags:          core.SpellFlagMeleeMetrics | core.SpellFlagAPL,

		Rank:          rank,
		RequiredLevel: level,

		RageCost: core.RageCostOptions{
			Cost: rageCost,
		},

		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD: core.GCDDefault,
			},
			IgnoreHaste: true,
		},

		DamageMultiplierAdditive: 1 + 0.05*float64(druid.Talents.SavageFury) + 0.1*float64(druid.Talents.FeralInstinct),
		DamageMultiplier:         1,
		ThreatMultiplier:         SwipeThreatMultiplier,

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			for idx := range results {
				results[idx] = spell.CalcDamage(sim, target, baseDamage, spell.OutcomeMeleeSpecialHitAndCrit)
				target = sim.Environment.NextTargetUnit(target)
			}

			for _, result := range results {
				spell.DealDamage(sim, result)
			}
		},
	})
}
