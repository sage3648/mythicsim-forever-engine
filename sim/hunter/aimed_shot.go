package hunter

import (
	"math"
	"time"

	"github.com/wowsims/classic/sim/core"
	"github.com/wowsims/classic/sim/core/proto"
)

// Spell ID, cost, cast time, cooldown, flat damage, coefficient, school, defense type and dot ticks of
// the hunter's spells come from the client table (spell_data_auto_gen.go, vendored from
// wowsims/forever) wherever it agrees with what the sim had. Missile speeds do not: every shot keeps
// the sim's 24 yd/sec. Damage ranges do not either: the table keeps only the truncated centre (see
// sim/mage/frostbolt.go); spell_damage_test.go checks every range kept here still contains it.

// The client stores .139 as a float32; the table widens it. Rounding back to the stated value keeps
// the sim's numbers where they were (sim/mage/frostbolt.go). Every coefficient and cost share read
// from the table goes through this.
func roundCoef(coef float64) float64 {
	return math.Round(coef*1e6) / 1e6
}

func (hunter *Hunter) getAimedShotConfig(rank int, timer *core.Timer) core.SpellConfig {
	row := spellData.AimedShot.ByRank(int32(rank))
	baseDamage, _ := row.Direct.Range()
	level := [7]int{0, 0, 28, 36, 44, 52, 60}[rank]

	return core.SpellConfig{
		SpellCode:      SpellCode_HunterAimedShot,
		ClassSpellMask: SpellMaskAimedShot,
		ActionID:       core.ActionID{SpellID: row.SpellID},
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskRangedSpecial,
		Flags:          core.SpellFlagMeleeMetrics | core.SpellFlagAPL | SpellFlagShot,
		CastType:       proto.CastType_CastTypeRanged,
		Rank:           rank,
		RequiredLevel:  level,
		MissileSpeed:   24,

		ManaCost: core.ManaCostOptions{
			FlatCost: float64(row.Cost),
		},
		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD: core.GCDDefault,
				// The client casts 2 sec, down from Classic's 3; the sim keeps its 0.5 sec shot wind-up on top.
				CastTime: row.CastTime + time.Millisecond*500,
			},
			CD: core.Cooldown{
				Timer:    timer,
				Duration: row.Cooldown,
			},
			ModifyCast: func(sim *core.Simulation, spell *core.Spell, cast *core.Cast) {
				cast.CastTime = spell.CastTime()
				hunter.Unit.AutoAttacks.CancelAutoSwing(sim)
			},
			IgnoreHaste: true, // Hunter GCD is locked at 1.5s
			CastTime: func(spell *core.Spell) time.Duration {
				return time.Duration(float64(spell.DefaultCast.CastTime) / hunter.RangedSwingSpeed())
			},
		},
		ExtraCastCondition: func(sim *core.Simulation, target *core.Unit) bool {
			return hunter.DistanceFromTarget >= core.MinRangedAttackDistance
		},

		CritDamageBonus: hunter.mortalShots(),

		DamageMultiplier: 1 + []float64{0, .03, .07, .10}[hunter.Talents.Barrage],
		ThreatMultiplier: 1,
		BonusCoefficient: roundCoef(row.Direct.BonusCoefficient()),

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			baseDamage := hunter.AutoAttacks.Ranged().CalculateNormalizedWeaponDamage(sim, spell.RangedAttackPower(target, false)) +
				hunter.AmmoDamageBonus +
				baseDamage

			result := spell.CalcDamage(sim, target, baseDamage, spell.OutcomeRangedHitAndCrit)
			hunter.Unit.AutoAttacks.EnableAutoSwing(sim)
			spell.WaitTravelTime(sim, func(s *core.Simulation) {
				spell.DealDamage(sim, result)
			})
		},
	}
}

// Aimed Shot is no longer a talent. The beta client (1.60.1.69893) keeps every rank on the hunter
// with a 2 sec cast, down from 3, and a much smaller flat bonus (rank 6 600 -> 166); mana costs
// and the 6 sec cooldown are Classic's.
func (hunter *Hunter) registerAimedShotSpell(timer *core.Timer) {
	maxRank := 6

	for i := 1; i <= maxRank; i++ {
		config := hunter.getAimedShotConfig(i, timer)

		if config.RequiredLevel <= int(hunter.Level) {
			hunter.AimedShot = hunter.GetOrRegisterSpell(config)
		}
	}
}
