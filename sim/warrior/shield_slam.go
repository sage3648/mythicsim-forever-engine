package warrior

import (
	"github.com/wowsims/classic/sim/core"
)

// Rank 4 in the beta client: 640-670 plus Block Value once. The 421-439 this used to carry is rank 1's,
// and the second Block Value and 15% of attack power were Season of Discovery's. The client table holds
// only the centre of the range (spell_damage_test.go checks it), so the range stays ours.
var shieldSlamDamage = [2]float64{640, 670}

func (warrior *Warrior) registerShieldSlamSpell() {
	if !warrior.Talents.ShieldSlam {
		return
	}

	// Cost, cooldown, school, defense type and coefficient come from the client table; the id and threat
	// stay ours (see registerHeroicStrikeSpell), and so does the damage (see shieldSlamDamage).
	spellID := int32(23925)
	row := spellData.ShieldSlam.BySpellID(spellID)
	damageLow, damageHigh := shieldSlamDamage[0], shieldSlamDamage[1]
	threat := 254.0

	warrior.ShieldSlam = warrior.RegisterSpell(AnyStance, core.SpellConfig{
		SpellCode:      SpellCode_WarriorShieldSlam,
		ClassSpellMask: SpellMaskShieldSlam,
		ActionID:       core.ActionID{SpellID: spellID},
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskMeleeMHSpecial, // TODO really?
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
				Timer:    warrior.NewTimer(),
				Duration: row.Cooldown,
			},
		},
		ExtraCastCondition: func(sim *core.Simulation, target *core.Unit) bool {
			return warrior.PseudoStats.CanBlock
		},

		CritDamageBonus: warrior.impale(),

		DamageMultiplier: 1,
		ThreatMultiplier: 1,
		FlatThreatBonus:  threat * 2,
		BonusCoefficient: row.Direct.BonusCoefficient(),

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			damage := sim.Roll(damageLow, damageHigh) + warrior.BlockValue()
			result := spell.CalcAndDealDamage(sim, target, damage, spell.OutcomeMeleeSpecialHitAndCrit)

			if !result.Landed() {
				spell.IssueRefund(sim)
			}
		},
	})
}
