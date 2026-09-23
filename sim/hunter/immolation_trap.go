package hunter

import (
	"strconv"
	"time"

	"github.com/wowsims/classic/sim/common/shared"
	"github.com/wowsims/classic/sim/core"
	"github.com/wowsims/classic/sim/core/stats"
)

func (hunter *Hunter) getImmolationTrapConfig(rank int, timer *core.Timer) core.SpellConfig {
	// Classic and Forever ids; the 4095xx ids were Season of Discovery's and are not in the beta client.
	// Damage, cost and levels are unchanged in the beta client, only the shared cooldown moved.
	// Spell ID, cost and cooldown come from the trap's row of the client table, school, defense type
	// and the burn from its effect's (see aimed_shot.go).
	row := spellData.ImmolationTrap.ByRank(int32(rank))
	effect := spellData.ImmolationTrapEffect.ByRank(int32(rank))
	periodic := effect.Periodic.(shared.SpellDataPeriodic)
	level := [6]int{0, 16, 26, 36, 46, 56}[rank]

	return core.SpellConfig{
		SpellCode:      SpellCode_HunterImmolationTrap,
		ClassSpellMask: SpellMaskImmolationTrap,
		ActionID:       core.ActionID{SpellID: row.SpellID},
		SpellSchool:    effect.SpellSchool,
		DefenseType:    effect.DefenseType,
		ProcMask:       core.ProcMaskSpellDamage,
		Flags:          core.SpellFlagAPL | core.SpellFlagPassiveSpell | SpellFlagTrap,
		Rank:           rank,
		RequiredLevel:  level,
		MissileSpeed:   row.MissileSpeed, // 0: the trap is a placed object (effect 104), no projectile

		ManaCost: core.ManaCostOptions{
			FlatCost: float64(row.Cost),
		},
		Cast: core.CastConfig{
			CD: core.Cooldown{
				Timer: timer,
				// Forever doubles the shared trap cooldown to 30 sec. Seen on every trap tooltip
				// from the demo streams (Savix, Xaryu and Soda, 12-13 September).
				Duration: core.TernaryDuration(hunter.Env.IsForever(), row.Cooldown, time.Second*15),
			},
			DefaultCast: core.Cast{
				GCD: core.GCDDefault,
			},
			IgnoreHaste: true, // Hunter GCD is locked at 1.5s
		},

		DamageMultiplier: 1,
		ThreatMultiplier: 1,

		Dot: core.DotConfig{
			Aura: core.Aura{
				Label: "ImmolationTrap" + hunter.Label + strconv.Itoa(rank),
				Tag:   "ImmolationTrap",
			},
			// 5 ticks 3 sec apart in both clients (13797, 14298-14301); the sim had Season of Discovery's 1.5 sec.
			NumberOfTicks: periodic.NumberOfTicks,
			TickLength:    periodic.TickLength,

			OnSnapshot: func(sim *core.Simulation, target *core.Unit, dot *core.Dot, isRollover bool) {
				dot.Snapshot(target, periodic.Tick, isRollover)
			},
			OnTick: func(sim *core.Simulation, target *core.Unit, dot *core.Dot) {
				dot.CalcAndDealPeriodicSnapshotDamage(sim, target, dot.OutcomeTick)
			},
		},

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			if hunter.DistanceFromTarget > 5 {
				return
			}
			// Traps gain no benefit from hit bonuses except for the Trap Mastery talent, since this is a unique interaction this is my workaround
			spellHit := spell.Unit.GetStat(stats.SpellHit) + target.PseudoStats.BonusSpellHitRatingTaken
			spell.Unit.AddStatDynamic(sim, stats.SpellHit, spellHit*-1)
			result := spell.CalcOutcome(sim, target, spell.OutcomeMagicHitNoHitCounter)
			spell.Unit.AddStatDynamic(sim, stats.SpellHit, spellHit)
			spell.WaitTravelTime(sim, func(s *core.Simulation) {
				spell.DealOutcome(sim, result)
				if result.Landed() {
					spell.Dot(target).Apply(sim)
				}
			})
		},
	}
}

func (hunter *Hunter) registerImmolationTrapSpell(timer *core.Timer) {
	maxRank := 5
	for i := 1; i <= maxRank; i++ {
		config := hunter.getImmolationTrapConfig(i, timer)

		if config.RequiredLevel <= int(hunter.Level) {
			hunter.ImmolationTrap = hunter.GetOrRegisterSpell(config)
		}
	}
}
