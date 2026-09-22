package warrior

import (
	"github.com/wowsims/classic/sim/common/shared"
	"github.com/wowsims/classic/sim/core"
)

func (warrior *Warrior) registerBloodthirstSpell(cdTimer *core.Timer) {
	if !warrior.Talents.Bloodthirst {
		return
	}

	// Rank 4 (23894) in the beta client. Cost, cooldown, school, defense type, flat damage and
	// coefficient come from the client table; the id stays ours (see registerHeroicStrikeSpell). The 35%
	// of attack power sits on a dummy effect, so it stays ours.
	spellID := int32(23894)
	row := spellData.Bloodthirst.BySpellID(spellID)
	flatDamage := shared.SpellDataMin(row.Direct)

	warrior.Bloodthirst = warrior.RegisterSpell(AnyStance, core.SpellConfig{
		SpellCode:      SpellCode_WarriorBloodthirst,
		ClassSpellMask: SpellMaskBloodthirst,
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

		CritDamageBonus: warrior.impale(),

		DamageMultiplier: 1,
		ThreatMultiplier: 1,
		BonusCoefficient: row.Direct.BonusCoefficient(),

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			// 48 plus 35% of attack power. The talent tooltip's 30 is rank 1's (23881).
			baseDamage := 0.35*spell.MeleeAttackPower(target) + flatDamage
			result := spell.CalcAndDealDamage(sim, target, baseDamage, spell.OutcomeMeleeSpecialHitAndCrit)
			if !result.Landed() {
				spell.IssueRefund(sim)
			}
		},
	})
}
