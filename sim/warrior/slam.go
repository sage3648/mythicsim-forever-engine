package warrior

import (
	"time"

	"github.com/wowsims/classic/sim/common/shared"
	"github.com/wowsims/classic/sim/core"
)

// Forever beta client 1.60.1.69893: cost, cast time, cooldown, school, defense type, flat damage and
// coefficient come from the client table; the id stays ours (see registerHeroicStrikeSpell).
func (warrior *Warrior) registerSlamSpell() {
	requiredLevel := 54
	spellID := int32(11605)
	row := spellData.Slam.BySpellID(spellID)
	flatDamageBonus := shared.SpellDataMin(row.Direct)

	// Improved Slam now takes the same amount off the global cooldown as it does the cast time,
	// and stops Slam from resetting the swing timer entirely.
	castReduction := time.Millisecond * 250 * time.Duration(warrior.Talents.ImprovedSlam)

	warrior.Slam = warrior.RegisterSpell(AnyStance, core.SpellConfig{
		SpellCode:      SpellCode_WarriorSlam,
		ClassSpellMask: SpellMaskSlam,
		ActionID:       core.ActionID{SpellID: spellID},
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskMeleeMHSpecial,
		Flags:          core.SpellFlagMeleeMetrics | core.SpellFlagAPL | SpellFlagOffensive,

		RequiredLevel: requiredLevel,

		RageCost: core.RageCostOptions{
			Cost:   float64(row.Cost),
			Refund: 0.8,
		},
		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD:      max(core.GCDDefault-castReduction, core.GCDMin),
				CastTime: row.CastTime - castReduction,
			},
			// Every rank of Slam carries a 15 sec cooldown in the beta client, which Classic's has not.
			CD: core.Cooldown{
				Timer:    warrior.NewTimer(),
				Duration: row.Cooldown,
			},
			ModifyCast: func(sim *core.Simulation, spell *core.Spell, cast *core.Cast) {
				if warrior.Talents.ImprovedSlam == 0 && spell.CastTime() > 0 {
					warrior.AutoAttacks.StopMeleeUntil(sim, sim.CurrentTime+cast.CastTime, true)
				}
			},
		},

		CritDamageBonus: warrior.impale(),

		DamageMultiplier: 1,
		ThreatMultiplier: 1,
		FlatThreatBonus:  140, // Should this be 54 or the old 140 value from before SoD?
		BonusCoefficient: row.Direct.BonusCoefficient(),

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			baseDamage := flatDamageBonus + spell.Unit.MHWeaponDamage(sim, spell.MeleeAttackPower(target))

			result := spell.CalcAndDealDamage(sim, target, baseDamage, spell.OutcomeMeleeWeaponSpecialHitAndCrit)
			if !result.Landed() {
				spell.IssueRefund(sim)
			}
		},
	})
}
