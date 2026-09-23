package hunter

import (
	"time"

	"github.com/wowsims/classic/sim/core"
	"github.com/wowsims/classic/sim/core/proto"
)

// Sniper Shot from the beta client (1310687, 1310785, 1310786): a 4 sec cast on a 15 sec cooldown
// for 365 mana at every rank, adding 160/225/295 to a normalized weapon shot.
func (hunter *Hunter) registerSniperShotSpell() {
	if !hunter.Talents.SniperShot {
		return
	}

	rank := 1
	if hunter.Level >= 58 {
		rank = 3
	} else if hunter.Level >= 48 {
		rank = 2
	}
	// Everything comes from the client table (see aimed_shot.go).
	row := spellData.SniperShot.ByRank(int32(rank))
	flatDamageBonus, _ := row.Direct.Range()

	hunter.SniperShot = hunter.RegisterSpell(core.SpellConfig{
		SpellCode:      SpellCode_HunterSniperShot,
		ClassSpellMask: SpellMaskSniperShot,
		ActionID:       core.ActionID{SpellID: row.SpellID},
		Rank:           rank,
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskRangedSpecial,
		Flags:          core.SpellFlagMeleeMetrics | core.SpellFlagAPL | SpellFlagShot,
		CastType:       proto.CastType_CastTypeRanged,
		MissileSpeed:   row.MissileSpeed,

		ManaCost: core.ManaCostOptions{
			FlatCost: float64(row.Cost),
		},
		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD: core.GCDDefault,
				// The client's 4 sec, no extra wind-up (the client has none, as for Multi-Shot).
				CastTime: row.CastTime,
			},
			CD: core.Cooldown{
				Timer:    hunter.NewTimer(),
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

		DamageMultiplier: 1,
		ThreatMultiplier: 1,
		BonusCoefficient: roundCoef(row.Direct.BonusCoefficient()),

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			baseDamage := hunter.AutoAttacks.Ranged().CalculateNormalizedWeaponDamage(sim, spell.RangedAttackPower(target, false)) +
				hunter.AmmoDamageBonus +
				flatDamageBonus

			result := spell.CalcDamage(sim, target, baseDamage, spell.OutcomeRangedHitAndCrit)
			hunter.Unit.AutoAttacks.EnableAutoSwing(sim)
			spell.WaitTravelTime(sim, func(s *core.Simulation) {
				spell.DealDamage(sim, result)
			})
		},
	})
}
