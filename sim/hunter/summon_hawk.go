package hunter

import (
	"strconv"
	"time"

	"github.com/wowsims/classic/sim/core"
)

// Summon Hawk shares its cooldown with Arcane Shot, and Ferocity and Unleashed Fury buff hawks
// the same way they buff pets.
//
// The beta client (1293241, 1293525-1293527) gives the dive bomb, 32/47/85/108 plus 5% of ranged
// attack power, the mana cost, the 6 sec cooldown and the 18 sec hawk (1293248), and client
// 1.60.1.69977 caps the hawks out at once at the rank's third effect, 2. The hawk that stays is a
// guardian whose swings the client does not describe, so each hawk's assault is modelled as the
// rank's dive bomb base damage every 3 sec. A cast past the cap replaces the hawk closest to
// leaving, which the client does not specify.
func (hunter *Hunter) registerSummonHawkSpell(timer *core.Timer) {
	if !hunter.Talents.SummonHawk {
		return
	}

	rank := 1
	switch {
	case hunter.Level >= 60:
		rank = 4
	case hunter.Level >= 48:
		rank = 3
	case hunter.Level >= 36:
		rank = 2
	}
	// Spell ID, cost, cooldown, dive bomb and school from the client table (see aimed_shot.go). Its
	// ranged defense type and 35 yd/sec missile are not used: ours rolls a melee hit that lands at once.
	row := spellData.SummonHawk.ByRank(int32(rank))
	baseDamage, _ := row.Direct.Range()
	maxHawks := int(row.Effects[2].Value)
	hawkDuration := spellData.SummonHawkTriggered.ByRank(1).Duration // 1293248
	const swingInterval = time.Second * 3

	bonusCrit := 2 * float64(hunter.Talents.Ferocity) * core.CritRatingPerCritChance
	damageMultiplier := 1 + 0.03*float64(hunter.Talents.UnleashedFury)

	// One spell per hawk, so each keeps its own assault on the target; their ticks are filed under
	// the rank's id with the hawk's number as the tag.
	hawks := make([]*core.Spell, maxHawks)
	for i := range hawks {
		hawks[i] = hunter.RegisterSpell(core.SpellConfig{
			ClassSpellMask: SpellMaskSummonHawk,
			ActionID:       core.ActionID{SpellID: row.SpellID, Tag: int32(i + 1)},
			SpellSchool:    row.SpellSchool,
			DefenseType:    core.DefenseTypeMelee,
			ProcMask:       core.ProcMaskEmpty,
			Flags:          core.SpellFlagMeleeMetrics | core.SpellFlagNoOnCastComplete,

			BonusCritRating:  bonusCrit,
			DamageMultiplier: damageMultiplier,
			ThreatMultiplier: 1,

			Dot: core.DotConfig{
				Aura: core.Aura{
					Label: "Summon Hawk " + strconv.Itoa(i+1) + hunter.Label,
				},
				NumberOfTicks: int32(hawkDuration / swingInterval),
				TickLength:    swingInterval,

				OnSnapshot: func(sim *core.Simulation, target *core.Unit, dot *core.Dot, isRollover bool) {
					dot.Snapshot(target, baseDamage, isRollover)
				},
				OnTick: func(sim *core.Simulation, target *core.Unit, dot *core.Dot) {
					dot.CalcAndDealPeriodicSnapshotDamage(sim, target, dot.OutcomeTick)
				},
			},
		})
	}
	hunter.summonHawks = hawks

	hunter.SummonHawk = hunter.RegisterSpell(core.SpellConfig{
		SpellCode:      SpellCode_HunterSummonHawk,
		ClassSpellMask: SpellMaskSummonHawk,
		ActionID:       core.ActionID{SpellID: row.SpellID},
		Rank:           rank,
		SpellSchool:    row.SpellSchool,
		DefenseType:    core.DefenseTypeMelee,
		ProcMask:       core.ProcMaskEmpty,
		Flags:          core.SpellFlagMeleeMetrics | core.SpellFlagAPL,

		ManaCost: core.ManaCostOptions{
			FlatCost: float64(row.Cost),
		},
		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD: core.GCDDefault,
			},
			IgnoreHaste: true, // Hunter GCD is locked at 1.5s
			CD: core.Cooldown{
				Timer:    timer,
				Duration: row.Cooldown,
			},
		},

		BonusCritRating:  bonusCrit,
		DamageMultiplier: damageMultiplier,
		ThreatMultiplier: 1,

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			damage := baseDamage + 0.05*spell.RangedAttackPower(target, false)
			result := spell.CalcAndDealDamage(sim, target, damage, spell.OutcomeMeleeSpecialHitAndCrit)
			if !result.Landed() {
				return
			}

			// A free slot, or else the hawk with the least time left.
			hawk := hawks[0].Dot(target)
			for _, h := range hawks[1:] {
				if dot := h.Dot(target); hawk.IsActive() && (!dot.IsActive() || dot.RemainingDuration(sim) < hawk.RemainingDuration(sim)) {
					hawk = dot
				}
			}
			hawk.Apply(sim)
		},
	})
}
