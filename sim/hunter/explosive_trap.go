package hunter

import (
	"strconv"
	"time"

	"github.com/wowsims/classic/sim/common/shared"
	"github.com/wowsims/classic/sim/core"
	"github.com/wowsims/classic/sim/core/stats"
)

var ExplosiveTrapBaseDamage = [4][]float64{{0}, {104, 135}, {145, 193}, {208, 265}}

func (hunter *Hunter) getExplosiveTrapConfig(rank int, timer *core.Timer) core.SpellConfig {
	// Classic and Forever ids; the 4095xx ids were Season of Discovery's and are not in the beta client.
	// Damage, cost and levels are unchanged in the beta client, only the shared cooldown moved.
	// Spell ID, cost and cooldown come from the trap's row of the client table, school, defense type
	// and the burn from its effect's (see aimed_shot.go); the blast's range stays ours.
	row := spellData.ExplosiveTrap.ByRank(int32(rank))
	effect := spellData.ExplosiveTrapEffect.ByRank(int32(rank))
	periodic := effect.Periodic.(shared.SpellDataPeriodic)
	minDamage := ExplosiveTrapBaseDamage[rank][0]
	maxDamage := ExplosiveTrapBaseDamage[rank][1]
	level := [4]int{0, 34, 44, 54}[rank]

	numHits := hunter.Env.GetNumTargets()

	return core.SpellConfig{
		SpellCode:      SpellCode_HunterExplosiveTrap,
		ClassSpellMask: SpellMaskExplosiveTrap,
		ActionID:       core.ActionID{SpellID: row.SpellID},
		SpellSchool:    effect.SpellSchool,
		DefenseType:    effect.DefenseType,
		ProcMask:       core.ProcMaskSpellDamage,
		Flags:          core.SpellFlagAPL | SpellFlagTrap,
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
			IsAOE: true,
			Aura: core.Aura{
				Label: "ExplosiveTrap" + hunter.Label + strconv.Itoa(rank),
				Tag:   "ExplosiveTrap",
			},
			NumberOfTicks: periodic.NumberOfTicks,
			TickLength:    periodic.TickLength,

			OnSnapshot: func(sim *core.Simulation, target *core.Unit, dot *core.Dot, isRollover bool) {
				dot.Snapshot(target, periodic.Tick, isRollover)
			},
			OnTick: func(sim *core.Simulation, target *core.Unit, dot *core.Dot) {
				for _, aoeTarget := range sim.Encounter.TargetUnits {
					// Explosive Trap DoT only does damage if the target does not have an immolation trap ticking on them
					if !aoeTarget.HasActiveAuraWithTag("ImmolationTrap") {
						dot.CalcAndDealPeriodicSnapshotDamage(sim, aoeTarget, dot.OutcomeTick)
					}
				}
			},
		},

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			if hunter.DistanceFromTarget > 5 {
				return
			}

			spell.WaitTravelTime(sim, func(s *core.Simulation) {
				curTarget := target
				// Traps gain no benefit from hit bonuses except for the Trap Mastery talent, since this is a unique interaction this is my workaround
				spellHit := spell.Unit.GetStat(stats.SpellHit) + target.PseudoStats.BonusSpellHitRatingTaken
				spell.Unit.AddStatDynamic(sim, stats.SpellHit, spellHit*-1)
				for hitIndex := int32(0); hitIndex < numHits; hitIndex++ {
					baseDamage := sim.Roll(minDamage, maxDamage)
					baseDamage *= sim.Encounter.AOECapMultiplier()
					spell.CalcAndDealDamage(sim, curTarget, baseDamage, spell.OutcomeMagicHitAndCrit)
					curTarget = sim.Environment.NextTargetUnit(curTarget)
				}
				spell.Unit.AddStatDynamic(sim, stats.SpellHit, spellHit)
				spell.AOEDot().ApplyOrReset(sim)
			})
		},
	}
}

func (hunter *Hunter) registerExplosiveTrapSpell(timer *core.Timer) {
	maxRank := 3
	for i := 1; i <= maxRank; i++ {
		config := hunter.getExplosiveTrapConfig(i, timer)

		if config.RequiredLevel <= int(hunter.Level) {
			hunter.ExplosiveTrap = hunter.GetOrRegisterSpell(config)
		}
	}
}
