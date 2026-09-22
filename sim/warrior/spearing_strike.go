package warrior

import (
	"github.com/wowsims/classic/sim/core"
	"github.com/wowsims/classic/sim/core/proto"
)

// Spearing Strike (1310222) is a new Arms talent in the beta client: 15 Rage, 20 sec cooldown,
// 40% normalized weapon damage, and 2 x 40% more against Giants and Dragonkin. The dismount and
// the extra damage to mounted targets have nothing to hit in a raid.
func (warrior *Warrior) registerSpearingStrikeSpell() {
	if !warrior.Talents.SpearingStrike {
		return
	}

	// Id, cost, cooldown, school, defense type and coefficient come from the client table. The 40% sits
	// on a weapon percent effect the table does not file under Direct, so it stays ours.
	row := spellData.SpearingStrike.ByRank(1)

	warrior.RegisterSpell(AnyStance, core.SpellConfig{
		ActionID:    core.ActionID{SpellID: row.SpellID},
		SpellSchool: row.SpellSchool,
		DefenseType: row.DefenseType,
		ProcMask:    core.ProcMaskMeleeMHSpecial,
		Flags:       core.SpellFlagMeleeMetrics | core.SpellFlagAPL | SpellFlagOffensive,

		RageCost: core.RageCostOptions{
			Cost:   float64(row.Cost),
			Refund: 0.8,
		},
		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD: core.GCDDefault,
			},
			IgnoreHaste: true,
			CD: core.Cooldown{
				Timer:    warrior.NewTimer(),
				Duration: row.Cooldown,
			},
		},

		CritDamageBonus: warrior.impale(),

		DamageMultiplier: 1,
		ThreatMultiplier: 1,
		BonusCoefficient: row.Direct.BonusCoefficient(),

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			weaponDamage := 0.4
			if target.MobType == proto.MobType_MobTypeGiant || target.MobType == proto.MobType_MobTypeDragonkin {
				weaponDamage += 0.4 * 2
			}
			baseDamage := weaponDamage * spell.Unit.MHNormalizedWeaponDamage(sim, spell.MeleeAttackPower(target))
			result := spell.CalcAndDealDamage(sim, target, baseDamage, spell.OutcomeMeleeWeaponSpecialHitAndCrit)

			if !result.Landed() {
				spell.IssueRefund(sim)
			}
		},
	})
}
