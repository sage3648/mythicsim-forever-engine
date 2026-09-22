package warrior

import (
	"github.com/wowsims/classic/sim/core"
)

func (warrior *Warrior) registerDemoralizingShoutSpell() {
	rank := int32(5)
	actionId := core.DemoralizingShoutSpellId[rank]
	// Cost and school come from the client table; the id stays ours (see registerHeroicStrikeSpell). The
	// table's Magic defense type is not applied, as before.
	row := spellData.DemoralizingShout.BySpellID(actionId)

	warrior.DemoralizingShoutAuras = warrior.NewEnemyAuraArray(func(target *core.Unit) *core.Aura {
		// Improved Demoralizing Shout is gone from the Forever tree and is baseline at full strength:
		// the beta client's rank 5 reduces attack power by 196, Classic's 140 plus 40%, for 45 sec.
		// Forever's Booming Voice only widens the radius.
		return core.DemoralizingShoutAura(target)
	})

	warrior.DemoralizingShout = warrior.RegisterSpell(AnyStance, core.SpellConfig{
		ActionID:    core.ActionID{SpellID: actionId},
		SpellSchool: row.SpellSchool,
		ProcMask:    core.ProcMaskEmpty,
		Flags:       core.SpellFlagAPL | SpellFlagOffensive,

		RageCost: core.RageCostOptions{
			Cost: float64(row.Cost),
		},
		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD: core.GCDDefault,
			},
			IgnoreHaste: true,
		},

		ThreatMultiplier: 0.4,
		FlatThreatBonus:  0.4 * 2 * float64(core.DemoralizingShoutLevel[rank]),

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			for _, aoeTarget := range sim.Encounter.TargetUnits {
				result := spell.CalcAndDealOutcome(sim, aoeTarget, spell.OutcomeMagicHit)
				if result.Landed() {
					warrior.DemoralizingShoutAuras.Get(aoeTarget).Activate(sim)
				}
			}
		},

		RelatedAuras: []core.AuraArray{warrior.DemoralizingShoutAuras},
	})
}
