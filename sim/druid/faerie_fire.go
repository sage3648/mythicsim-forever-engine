package druid

import (
	"time"

	"github.com/wowsims/classic/sim/core"
)

func (druid *Druid) registerFaerieFireSpell() {
	spellCode := SpellCode_DruidFaerieFire
	classMask := SpellMaskFaerieFire
	// Rank 4's cost comes from the client table (see wrath.go). Its id stays ours: reading it through the table files
	// the never-registered ranks 1-3 in sim/spell_sources_test.go. The table's Magic defense type is not used either.
	actionID := core.ActionID{SpellID: 9907}
	manaCostOptions := core.ManaCostOptions{
		FlatCost: float64(spellData.FaerieFire.ByRank(4).Cost),
	}
	gcd := core.GCDDefault
	ignoreHaste := false
	cd := core.Cooldown{}
	flatThreatBonus := 2. * 54
	flags := core.SpellFlagNone
	formMask := Humanoid | Moonkin

	druid.FaerieFireAuras = druid.NewEnemyAuraArray(func(target *core.Unit) *core.Aura {
		return core.FaerieFireAura(target)
	})

	// TODO: the beta client 1.60.1.69893 has no Faerie Fire (Feral): 16857 and 17390-17392 are gone from the
	// spellbook and 17392 from the spell tables. The cat and the bear keep it because their rotations are built
	// around it.
	if druid.InForm(Cat | Bear) {
		spellCode = SpellCode_DruidFaerieFireFeral
		classMask = SpellMaskFaerieFireFeral
		actionID = core.ActionID{SpellID: 17392}
		manaCostOptions = core.ManaCostOptions{}
		gcd = time.Second
		ignoreHaste = true
		formMask = Cat | Bear
		cd = core.Cooldown{
			Timer:    druid.NewTimer(),
			Duration: time.Second * 6,
		}
		druid.FaerieFireAuras = druid.NewEnemyAuraArray(func(target *core.Unit) *core.Aura {
			return core.FaerieFireFeralAura(target)
		})
	}
	flags |= core.SpellFlagAPL | core.SpellFlagResetAttackSwing

	druid.FaerieFire = druid.RegisterSpell(formMask, core.SpellConfig{
		SpellCode:      spellCode,
		ClassSpellMask: classMask,
		ActionID:       actionID,
		SpellSchool:    core.SpellSchoolNature,
		ProcMask:       core.ProcMaskSpellDamage,
		Flags:          flags,

		ManaCost: manaCostOptions,
		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD: gcd,
			},
			IgnoreHaste: ignoreHaste,
			CD:          cd,
		},

		ThreatMultiplier: 1,
		FlatThreatBonus:  flatThreatBonus,
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
