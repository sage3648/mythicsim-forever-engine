package hunter

import (
	"github.com/wowsims/classic/sim/core"
)

const RaptorStrikeRanks = 8

var RaptorStrikeSpellIdMeleeSpecialist = [RaptorStrikeRanks + 1]int32{0, 415335, 415336, 415337, 415338, 415340, 415341, 415342, 415343}

// Spell ID, cost, cooldown, flat damage, coefficient, school and defense type come from the client
// table (see aimed_shot.go).
var RaptorStrikeLevel = [RaptorStrikeRanks + 1]int{0, 1, 8, 16, 24, 32, 40, 48, 56}

// Returns true if the regular melee swing should be used, false otherwise.
func (hunter *Hunter) TryRaptorStrike(sim *core.Simulation, mhSwingSpell *core.Spell) *core.Spell {
	if hunter.curQueuedAutoSpell != nil && hunter.curQueuedAutoSpell.CanCast(sim, hunter.CurrentTarget) {
		return hunter.curQueuedAutoSpell
	}
	return mhSwingSpell
}

func (hunter *Hunter) getRaptorStrikeConfig(rank int) core.SpellConfig {
	row := spellData.RaptorStrike.ByRank(int32(rank))
	level := RaptorStrikeLevel[rank]

	hunter.RaptorStrikeHit = hunter.newRaptorStrikeHitSpell(rank)

	spellConfig := core.SpellConfig{
		SpellCode:      SpellCode_HunterRaptorStrike,
		ClassSpellMask: SpellMaskRaptorStrike,
		ActionID:       core.ActionID{SpellID: row.SpellID},
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskMeleeMHSpecial | core.ProcMaskMeleeMHAuto,
		Flags:          core.SpellFlagMeleeMetrics | SpellFlagStrike,
		Rank:           rank,
		RequiredLevel:  level,

		ManaCost: core.ManaCostOptions{
			FlatCost: float64(row.Cost),
		},

		Cast: core.CastConfig{
			CD: core.Cooldown{
				Timer:    hunter.NewTimer(),
				Duration: row.Cooldown,
			},
		},
		ExtraCastCondition: func(sim *core.Simulation, target *core.Unit) bool {
			return hunter.DistanceFromTarget <= core.MaxMeleeAttackDistance
		},

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			hunter.RaptorStrikeHit.Cast(sim, target)

			if hunter.curQueueAura != nil {
				hunter.curQueueAura.Deactivate(sim)
			}
		},
	}

	return spellConfig
}

func (hunter *Hunter) newRaptorStrikeHitSpell(rank int) *core.Spell {
	row := spellData.RaptorStrike.ByRank(int32(rank))
	baseDamage, _ := row.Direct.Range()

	return hunter.RegisterSpell(core.SpellConfig{
		SpellCode:      SpellCode_HunterRaptorStrikeHit,
		ClassSpellMask: SpellMaskRaptorStrikeHit,
		ActionID:       core.ActionID{SpellID: row.SpellID}.WithTag(1),
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskMeleeMHSpecial,
		Flags:          core.SpellFlagMeleeMetrics | core.SpellFlagNoOnCastComplete,

		BonusCritRating:  float64(hunter.Talents.SavageStrikes) * 2 * core.CritRatingPerCritChance,
		CritDamageBonus:  hunter.mortalShots(),
		DamageMultiplier: 1,
		BonusCoefficient: roundCoef(row.Direct.BonusCoefficient()),

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			damage := baseDamage + hunter.MHWeaponDamage(sim, spell.MeleeAttackPower(target))
			spell.CalcAndDealDamage(sim, target, damage, spell.OutcomeMeleeWeaponSpecialHitAndCrit)
		},
	})
}

func (hunter *Hunter) makeQueueSpellsAndAura() *core.Spell {
	queueAura := hunter.RegisterAura(core.Aura{
		Label:    "Raptor Strike Queued",
		ActionID: hunter.RaptorStrike.ActionID,
		Duration: core.NeverExpires,
		OnGain: func(aura *core.Aura, sim *core.Simulation) {
			if hunter.curQueueAura != nil {
				hunter.curQueueAura.Deactivate(sim)
			}
			hunter.PseudoStats.DisableDWMissPenalty = true
			hunter.curQueueAura = aura
			hunter.curQueuedAutoSpell = hunter.RaptorStrike
		},
		OnExpire: func(aura *core.Aura, sim *core.Simulation) {
			hunter.PseudoStats.DisableDWMissPenalty = false
			hunter.curQueueAura = nil
			hunter.curQueuedAutoSpell = nil
		},
	})

	queueSpell := hunter.RegisterSpell(core.SpellConfig{
		SpellCode:      SpellCode_HunterRaptorStrike,
		ClassSpellMask: SpellMaskRaptorStrike,
		ActionID:       hunter.RaptorStrike.WithTag(3),
		Flags:          core.SpellFlagMeleeMetrics | core.SpellFlagAPL,

		ExtraCastCondition: func(sim *core.Simulation, target *core.Unit) bool {
			return hunter.curQueueAura != queueAura &&
				hunter.CurrentMana() >= hunter.RaptorStrike.Cost.GetCurrentCost() &&
				!hunter.IsCasting(sim) &&
				hunter.DistanceFromTarget <= core.MaxMeleeAttackDistance &&
				hunter.RaptorStrike.IsReady(sim)
		},

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			queueAura.Activate(sim)
		},
	})
	queueSpell.CdSpell = hunter.RaptorStrike

	return queueSpell
}

func (hunter *Hunter) registerRaptorStrikeSpell() {
	rank := map[int32]int{
		25: 4,
		40: 6,
		50: 7,
		60: 8,
	}[hunter.Level]

	config := hunter.getRaptorStrikeConfig(rank)
	hunter.RaptorStrike = hunter.GetOrRegisterSpell(config)
	hunter.makeQueueSpellsAndAura()
}
