package druid

import (
	"time"

	"github.com/wowsims/classic/sim/core"
)

func (druid *Druid) registerFaerieFireSpell() {
	// Rank 4's cost comes from the client table (see wrath.go). Its id stays ours: reading it through the table files
	// the never-registered ranks 1-3 in sim/spell_sources_test.go. The table's Magic defense type is not used either.
	// Forever has no Faerie Fire (Feral): 16857 and 17390-17392 are gone from the client. 9907 itself is castable
	// in Cat, Bear and Moonkin form (SpellShapeshift mask 0x40000091), at its mana cost and 1.5 s GCD.
	formMask := Humanoid | Moonkin | Cat | Bear

	druid.FaerieFireAuras = druid.NewEnemyAuraArray(func(target *core.Unit) *core.Aura {
		return core.FaerieFireAura(target)
	})

	druid.FaerieFire = druid.RegisterSpell(formMask, core.SpellConfig{
		SpellCode:      SpellCode_DruidFaerieFire,
		ClassSpellMask: SpellMaskFaerieFire,
		ActionID:       core.ActionID{SpellID: 9907},
		SpellSchool:    core.SpellSchoolNature,
		ProcMask:       core.ProcMaskSpellDamage,
		Flags:          core.SpellFlagAPL | core.SpellFlagResetAttackSwing,

		ManaCost: core.ManaCostOptions{
			FlatCost: float64(spellData.FaerieFire.ByRank(4).Cost),
		},
		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD: core.GCDDefault,
			},
		},

		ThreatMultiplier: 1,
		FlatThreatBonus:  2. * 54,
		DamageMultiplier: 1,

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			result := spell.CalcAndDealOutcome(sim, target, spell.OutcomeMagicHit)
			if result.Landed() {
				druid.FaerieFireAuras.Get(target).Activate(sim)
			}

			if druid.InForm(Humanoid | Moonkin) {
				druid.AutoAttacks.StopMeleeUntil(sim, sim.CurrentTime, false)
			}
		},

		RelatedAuras: []core.AuraArray{druid.FaerieFireAuras},
	})
}

func (druid *Druid) ShouldFaerieFire(sim *core.Simulation, target *core.Unit) bool {
	if druid.FaerieFire == nil {
		return false
	}

	if !druid.FaerieFire.IsReady(sim) {
		return false
	}

	debuff := druid.FaerieFireAuras.Get(target)
	return !debuff.IsActive() || debuff.RemainingDuration(sim) < time.Second*4
}
