package warrior

import (
	"time"

	"github.com/wowsims/classic/sim/common/shared"
	"github.com/wowsims/classic/sim/core"
)

func (warrior *Warrior) registerOverpowerSpell(cdTimer *core.Timer) {
	// Cost, cooldown, school, defense type, flat damage and coefficient come from the client table; the
	// id stays ours (see registerHeroicStrikeSpell).
	spellID := int32(11585)
	row := spellData.Overpower.BySpellID(spellID)
	bonusDamage := shared.SpellDataMin(row.Direct)

	warrior.RegisterAura(core.Aura{
		Label:    "Overpower Trigger",
		Duration: core.NeverExpires,
		OnReset: func(aura *core.Aura, sim *core.Simulation) {
			aura.Activate(sim)
		},
		OnSpellHitDealt: func(aura *core.Aura, sim *core.Simulation, spell *core.Spell, result *core.SpellResult) {
			if result.DidDodge() {
				warrior.OverpowerAura.Activate(sim)
			}
		},
	})

	warrior.OverpowerAura = warrior.RegisterAura(core.Aura{
		Label:    "Overpower Aura",
		ActionID: core.ActionID{SpellID: spellID},
		Duration: time.Second * 5,
	})

	warrior.Overpower = warrior.RegisterSpell(BattleStance, core.SpellConfig{
		SpellCode:      SpellCode_WarriorOverpower,
		ClassSpellMask: SpellMaskOverpower,
		ActionID:       core.ActionID{SpellID: spellID},
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskMeleeMHSpecial,
		Flags:          core.SpellFlagMeleeMetrics | core.SpellFlagAPL | SpellFlagOffensive,

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
				Timer:    cdTimer,
				Duration: row.Cooldown,
			},
		},
		ExtraCastCondition: func(sim *core.Simulation, target *core.Unit) bool {
			return warrior.OverpowerAura.IsActive() || warrior.BloodthrillAura.IsActive()
		},

		BonusCritRating: 25 * core.CritRatingPerCritChance * float64(warrior.Talents.ImprovedOverpower),

		CritDamageBonus: warrior.impale(),

		DamageMultiplier: 1,
		ThreatMultiplier: 0.75,
		BonusCoefficient: row.Direct.BonusCoefficient(),

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			baseDamage := bonusDamage + spell.Unit.MHNormalizedWeaponDamage(sim, spell.MeleeAttackPower(target))
			result := spell.CalcAndDealDamage(sim, target, baseDamage, spell.OutcomeMeleeSpecialNoBlockDodgeParry)

			warrior.OverpowerAura.Deactivate(sim)
			if warrior.BloodthrillAura.IsActive() {
				warrior.BloodthrillAura.Deactivate(sim)
			}
			if !result.Landed() {
				spell.IssueRefund(sim)
			}
		},
	})
}
