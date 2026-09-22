package rogue

import (
	"time"

	"github.com/wowsims/classic/sim/core"
)

func (rogue *Rogue) applyRiposte() {
	if !rogue.Talents.Riposte {
		return
	}

	// Forever beta client 1.60.1.69893: id, cooldown, school and defense type come from the client
	// table. Its cost reads 0 (ours 10) and its 6s duration is the disarm's, not the 5s window after a
	// parry, so both stay ours.
	row := spellData.Riposte.ByRank(1)

	var riposteReady *core.Aura

	riposte := rogue.GetOrRegisterSpell(core.SpellConfig{
		ActionID:    core.ActionID{SpellID: row.SpellID},
		SpellSchool: row.SpellSchool,
		DefenseType: row.DefenseType,
		ProcMask:    core.ProcMaskMeleeMHSpecial,
		Flags:       core.SpellFlagMeleeMetrics | core.SpellFlagAPL,

		EnergyCost: core.EnergyCostOptions{
			Cost: 10,
		},
		Cast: core.CastConfig{
			CD: core.Cooldown{
				Timer:    rogue.NewTimer(),
				Duration: row.Cooldown,
			},
		},
		ExtraCastCondition: func(sim *core.Simulation, target *core.Unit) bool {
			return riposteReady.IsActive()
		},

		DamageMultiplier: 1.5,
		ThreatMultiplier: 1,
		BonusCoefficient: 1,

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			riposteReady.Deactivate(sim)

			damage := rogue.MHWeaponDamage(sim, spell.MeleeAttackPower(target))
			spell.CalcAndDealDamage(sim, target, damage, spell.OutcomeMeleeWeaponSpecialHitAndCrit)
		},
	})

	riposteReady = rogue.RegisterAura(core.Aura{
		Label:    "Riposte Ready Aura",
		ActionID: riposte.ActionID,
		Duration: time.Second * 5,
	})

	rogue.RegisterAura(core.Aura{
		Label:    "Riposte Trigger",
		Duration: core.NeverExpires,
		OnReset: func(aura *core.Aura, sim *core.Simulation) {
			aura.Activate(sim)
		},
		OnSpellHitTaken: func(aura *core.Aura, sim *core.Simulation, spell *core.Spell, result *core.SpellResult) {
			if result.Outcome == core.OutcomeParry {
				riposteReady.Activate(sim)
			}
		},
	})
}
