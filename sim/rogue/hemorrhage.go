package rogue

import (
	"strconv"
	"time"

	"github.com/wowsims/classic/sim/core"
)

// Hemorrhage no longer weakens the target for the whole raid, it makes the rogue's own
// Rupture hit harder.
const HemorrhageRuptureMultiplier = 1.15

func (rogue *Rogue) registerHemorrhageSpell() {
	if !rogue.Talents.Hemorrhage {
		return
	}

	// Forever beta client 1.60.1.69893: id, cost, debuff duration, school, defense type and coefficient
	// come from the client table.
	row := spellData.Hemorrhage.ByRank(1)
	actionID := core.ActionID{SpellID: row.SpellID}

	rogue.HemorrhageAuras = rogue.NewEnemyAuraArray(func(target *core.Unit) *core.Aura {
		return target.GetOrRegisterAura(core.Aura{
			Label:    "Hemorrhage-" + strconv.Itoa(int(rogue.Index)),
			ActionID: actionID,
			Duration: row.Duration,
		})
	})

	rogue.Hemorrhage = rogue.RegisterSpell(core.SpellConfig{
		SpellCode:      SpellCode_RogueHemorrhage,
		ClassSpellMask: SpellMaskHemorrhage,
		ActionID:       actionID,
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskMeleeMHSpecial,
		Flags:          rogue.builderFlags(),

		EnergyCost: core.EnergyCostOptions{
			Cost:   float64(row.Cost),
			Refund: 0.8,
		},
		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD: time.Second,
			},
			IgnoreHaste: true,
		},

		CritDamageBonus: rogue.lethality(),

		DamageMultiplier: core.TernaryFloat64(rogue.HasDagger(core.MainHand), 1.45, 1),
		ThreatMultiplier: 1,
		BonusCoefficient: row.Direct.BonusCoefficient(),

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			rogue.BreakStealth(sim)
			// The beta client moved Hemorrhage from plain to normalized weapon damage.
			baseDamage := spell.Unit.MHNormalizedWeaponDamage(sim, spell.MeleeAttackPower(target)) * rogue.quietusMultiplier(sim)

			result := spell.CalcAndDealDamage(sim, target, baseDamage, spell.OutcomeMeleeWeaponSpecialHitAndCrit)

			if result.Landed() {
				rogue.AddComboPoints(sim, 1, target, spell.ComboPointMetrics())
				rogue.HemorrhageAuras.Get(target).Activate(sim)
			} else {
				spell.IssueRefund(sim)
			}
		},
	})
}

// Hemorrhage is optional, so the debuff has to be looked up defensively.
func (rogue *Rogue) isHemorrhaging(target *core.Unit) bool {
	return rogue.HemorrhageAuras != nil && rogue.HemorrhageAuras.Get(target).IsActive()
}
