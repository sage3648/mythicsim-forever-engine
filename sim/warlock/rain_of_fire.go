package warlock

import (
	"strconv"

	"github.com/wowsims/classic/sim/common/shared"
	"github.com/wowsims/classic/sim/core"
)

const RainOfFireRanks = 4

func (warlock *Warlock) getRainOfFireBaseConfig(rank int) core.SpellConfig {
	// Beta client 1.60.1: each tick is now its own damage spell (1282380, 1282383, 1282384, 1282385)
	// carrying the per tick damage and a 0.083 coefficient. The 0.03 on the channel's dummy effect is
	// not the damage coefficient.
	//
	// Spell ID, cost, tick, ticks and the coefficient come from the client table. The table tick
	// (41/93/151/221) is each rank scaled to its cap, the server's value for every bracket's top rank.
	row := spellData.RainOfFire.ByRank(int32(rank))
	periodic := row.Periodic.(shared.SpellDataPeriodic)
	baseDamage := periodic.Tick
	level := [RainOfFireRanks + 1]int{0, 20, 34, 46, 58}[rank]

	flags := core.SpellFlagAPL | core.SpellFlagResetAttackSwing | WarlockFlagDestruction | core.SpellFlagChanneled

	config := core.SpellConfig{
		ActionID:       core.ActionID{SpellID: row.SpellID},
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskSpellDamage,
		ClassSpellMask: SpellMaskRainOfFire,
		Flags:          flags,
		RequiredLevel:  level,
		Rank:           rank,

		DamageMultiplier: 1,
		ThreatMultiplier: 1,

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
				Label: "RainOfFire-" + warlock.Label + strconv.Itoa(rank),
			},
			NumberOfTicks:    periodic.NumberOfTicks,
			TickLength:       periodic.TickLength,
			BonusCoefficient: roundCoef(periodic.Coef),

			OnSnapshot: func(sim *core.Simulation, target *core.Unit, dot *core.Dot, isRollover bool) {
				dot.Snapshot(target, baseDamage, isRollover)
			},
			OnTick: func(sim *core.Simulation, target *core.Unit, dot *core.Dot) {
				for _, aoeTarget := range sim.Encounter.TargetUnits {
					dot.CalcAndDealPeriodicSnapshotDamage(sim, aoeTarget, dot.OutcomeTick)
				}

			},
		},

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			spell.AOEDot().Apply(sim)
		},
	}

	return config
}

func (warlock *Warlock) registerRainOfFireSpell() {
	warlock.RainOfFire = make([]*core.Spell, 0)
	for rank := 1; rank <= RainOfFireRanks; rank++ {
		config := warlock.getRainOfFireBaseConfig(rank)

		if config.RequiredLevel <= int(warlock.Level) {
			warlock.RainOfFire = append(warlock.RainOfFire, warlock.GetOrRegisterSpell(config))
		}
	}
}
