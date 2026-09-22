package priest

import (
	"fmt"

	"github.com/wowsims/classic/sim/common/shared"
	"github.com/wowsims/classic/sim/core"
)

const DevouringPlagueRanks = 6

var DevouringPlagueLevel = [DevouringPlagueRanks + 1]int{0, 20, 28, 36, 44, 52, 60}

func (priest *Priest) registerDevouringPlagueSpell() {
	//TO DO: Implement race requirement
	priest.DevouringPlague = make([]*core.Spell, DevouringPlagueRanks+1)
	cdTimer := priest.NewTimer()

	for rank := 1; rank <= DevouringPlagueRanks; rank++ {
		config := priest.getDevouringPlagueConfig(rank, cdTimer)

		if config.RequiredLevel <= int(priest.Level) {
			priest.DevouringPlague[rank] = priest.GetOrRegisterSpell(config)
		}
	}
}

func (priest *Priest) getDevouringPlagueConfig(rank int, cdTimer *core.Timer) core.SpellConfig {
	// Forever beta client 1.60.1.69893. Spell ID, cost, cooldown (3 min in Classic), tick, tick count and
	// coefficient come from the client table.
	row := spellData.DevouringPlague.ByRank(int32(rank))
	periodic := row.Periodic.(shared.SpellDataPeriodic)
	level := DevouringPlagueLevel[rank]

	return core.SpellConfig{
		SpellCode:      SpellCode_PriestDevouringPlague,
		ClassSpellMask: SpellMaskDevouringPlague,
		ActionID:       core.ActionID{SpellID: row.SpellID},
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskSpellDamage,
		Flags:          SpellFlagPriest | core.SpellFlagAPL | core.SpellFlagDisease | core.SpellFlagPureDot,

		Rank:          rank,
		RequiredLevel: level,

		// Devouring Contagion, 25/50% in the beta client.
		ManaCost: core.ManaCostOptions{
			FlatCost:   float64(row.Cost),
			Multiplier: 100 - 25*priest.Talents.DevouringContagion,
		},
		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD: core.GCDDefault,
			},
			CD: core.Cooldown{
				Timer:    cdTimer,
				Duration: row.Cooldown,
			},
		},

		DamageMultiplier: 1,
		ThreatMultiplier: 1,

		Dot: core.DotConfig{
			Aura: core.Aura{
				Label: fmt.Sprintf("Devouring Plague (Rank %d)", rank),
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
			result := spell.CalcOutcome(sim, target, spell.OutcomeMagicHitNoHitCounter)
			if result.Landed() {
				priest.AddShadowWeavingStack(sim)
				spell.Dot(target).Apply(sim)
			}
			spell.DealOutcome(sim, result)
		},
	}
}
