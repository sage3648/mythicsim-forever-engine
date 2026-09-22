package hunter

import (
	"fmt"
	"time"

	"github.com/wowsims/classic/sim/common/shared"
	"github.com/wowsims/classic/sim/core"
)

func (hunter *Hunter) registerVolleySpell() {
	ranks := 3

	for i := ranks; i >= 0; i-- {
		config := hunter.getVolleyConfig(i)

		if config.RequiredLevel <= int(hunter.Level) {
			hunter.Volley = hunter.GetOrRegisterSpell(config)
			break
		}
	}
}

func (hunter *Hunter) getVolleyConfig(rank int) core.SpellConfig {
	level := [4]int{0, 40, 50, 58}[rank]
	// Forever fires a separate damage spell each tick (1279721, 1279719, 1279715). Spell ID, cost,
	// school, tick, tick count and length come from the client table (see aimed_shot.go).
	row := spellData.Volley.ByRank(int32(rank))
	periodic := row.Periodic.(shared.SpellDataPeriodic)

	return core.SpellConfig{
		SpellCode:      SpellCode_HunterVolley,
		ClassSpellMask: SpellMaskVolley,
		ActionID:       core.ActionID{SpellID: row.SpellID},
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskSpellDamage,
		Flags:          core.SpellFlagChanneled | core.SpellFlagAPL,

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

		Dot: core.DotConfig{
			IsAOE: true,
			Aura: core.Aura{
				Label: fmt.Sprintf("Volley (Rank %d)", rank),
			},
			NumberOfTicks: periodic.NumberOfTicks,
			TickLength:    periodic.TickLength,
			// The tick spell has no coefficient and the channel's dummy effect carries .03, the
			// same placeholder Blizzard and Rain of Fire carry, so Classic's .056 a tick stands.
			BonusCoefficient: .056,
			OnSnapshot: func(sim *core.Simulation, target *core.Unit, dot *core.Dot, isRollover bool) {
				dot.Snapshot(target, periodic.Tick, isRollover)
			},
			OnTick: func(sim *core.Simulation, target *core.Unit, dot *core.Dot) {
				for _, aoeTarget := range sim.Encounter.TargetUnits {
					dot.CalcAndDealPeriodicSnapshotDamage(sim, aoeTarget, dot.OutcomeTick)
				}
			},
		},

		CritDamageBonus:  hunter.mortalShots(),
		DamageMultiplier: 1 + []float64{0, .03, .07, .10}[hunter.Talents.Barrage],
		ThreatMultiplier: 1,

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			hunter.Unit.AutoAttacks.DelayRangedUntil(sim, sim.CurrentTime+(time.Second*6))
			spell.AOEDot().Apply(sim)
		},
	}
}
