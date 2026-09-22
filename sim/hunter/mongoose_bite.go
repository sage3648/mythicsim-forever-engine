package hunter

import (
	"time"

	"github.com/wowsims/classic/sim/core"
)

func (hunter *Hunter) getMongooseBiteConfig(rank int) core.SpellConfig {
	row := spellData.MongooseBite.ByRank(int32(rank))
	baseDamage, _ := row.Direct.Range()
	level := [5]int{0, 16, 30, 44, 58}[rank]

	spellConfig := core.SpellConfig{
		SpellCode:      SpellCode_HunterMongooseBite,
		ClassSpellMask: SpellMaskMongooseBite,
		ActionID:       core.ActionID{SpellID: row.SpellID},
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskMeleeSpecial,
		Flags:          core.SpellFlagMeleeMetrics | core.SpellFlagAPL,
		Rank:           rank,
		RequiredLevel:  level,

		ManaCost: core.ManaCostOptions{
			FlatCost: float64(row.Cost),
		},

		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD: core.GCDDefault,
			},
			CD: core.Cooldown{
				Timer:    hunter.NewTimer(),
				Duration: row.Cooldown,
			},
		},

		ExtraCastCondition: func(sim *core.Simulation, target *core.Unit) bool {
			return hunter.DistanceFromTarget <= core.MaxMeleeAttackDistance && hunter.DefensiveState.IsActive()
		},

		BonusCritRating:  float64(hunter.Talents.SavageStrikes) * 2 * core.CritRatingPerCritChance,
		CritDamageBonus:  hunter.mortalShots(),
		DamageMultiplier: 1,
		ThreatMultiplier: 1,
		BonusCoefficient: roundCoef(row.Direct.BonusCoefficient()),

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			hunter.DefensiveState.Deactivate(sim)
			// Forever: normalized melee weapon damage plus a smaller flat amount, where Classic dealt the flat amount alone.
			damage := baseDamage + hunter.AutoAttacks.MH().CalculateNormalizedWeaponDamage(sim, spell.MeleeAttackPower(target))
			result := spell.CalcAndDealDamage(sim, target, damage, spell.OutcomeMeleeWeaponSpecialHitAndCrit)

			if hunter.LaceratingStrikes != nil && result.Landed() {
				hunter.procLaceratingStrikes(sim, result)
			}
		},
	}

	return spellConfig
}

func (hunter *Hunter) registerMongooseBiteSpell() {
	// Aura is only used as a pre-requisite for Mongoose Bite
	hunter.DefensiveState = hunter.RegisterAura(core.Aura{
		Label:    "Defensive State",
		ActionID: core.ActionID{SpellID: 5302},
		Duration: time.Second * 5,

		OnSpellHitTaken: func(aura *core.Aura, sim *core.Simulation, spell *core.Spell, result *core.SpellResult) {
			if result.DidDodge() {
				aura.Activate(sim)
			}
		},
	})

	rank := map[int32]int{
		25: 1,
		40: 2,
		50: 3,
		60: 4,
	}[hunter.Level]

	config := hunter.getMongooseBiteConfig(rank)
	hunter.MongooseBite = hunter.GetOrRegisterSpell(config)
}
