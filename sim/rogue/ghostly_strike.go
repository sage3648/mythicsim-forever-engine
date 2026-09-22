package rogue

import (
	"time"

	"github.com/wowsims/classic/sim/core/stats"

	"github.com/wowsims/classic/sim/core"
)

func (rogue *Rogue) registerGhostlyStrikeSpell() {
	if !rogue.Talents.GhostlyStrike {
		return
	}

	// Forever beta client 1.60.1.69893: id, cooldown, buff duration, school and defense type come from
	// the client table. Its cost reads 0 where the client charges 40, so the cost stays ours.
	row := spellData.GhostlyStrike.ByRank(1)

	ghostlyStrikeAura := rogue.RegisterAura(core.Aura{
		Label:    "Ghostly Strike Buff",
		ActionID: core.ActionID{SpellID: row.SpellID},
		Duration: row.Duration,
		OnGain: func(aura *core.Aura, sim *core.Simulation) {
			rogue.AddStatDynamic(sim, stats.Dodge, 15*core.DodgeRatingPerDodgeChance)
		},
		OnExpire: func(aura *core.Aura, sim *core.Simulation) {
			rogue.AddStatDynamic(sim, stats.Dodge, -15*core.DodgeRatingPerDodgeChance)
		},
	})

	rogue.GhostlyStrike = rogue.RegisterSpell(core.SpellConfig{
		SpellCode:      SpellCode_RogueGhostlyStrike,
		ClassSpellMask: SpellMaskGhostlyStrike,
		ActionID:       ghostlyStrikeAura.ActionID,
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskMeleeMHSpecial,
		Flags:          rogue.builderFlags(),
		EnergyCost: core.EnergyCostOptions{
			Cost:   40.0,
			Refund: 0.8,
		},

		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD: time.Second,
			},
			CD: core.Cooldown{
				Timer:    rogue.NewTimer(),
				Duration: row.Cooldown,
			},
			IgnoreHaste: true,
		},

		CritDamageBonus: rogue.lethality(),

		DamageMultiplier: core.TernaryFloat64(rogue.HasDagger(core.MainHand), 1.8, 1.25),
		ThreatMultiplier: 1,
		BonusCoefficient: 1,

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			rogue.BreakStealth(sim)
			baseDamage := spell.Unit.MHWeaponDamage(sim, spell.MeleeAttackPower(target)) * rogue.quietusMultiplier(sim)

			result := spell.CalcAndDealDamage(sim, target, baseDamage, spell.OutcomeMeleeWeaponSpecialHitAndCrit)

			ghostlyStrikeAura.Activate(sim)

			if result.Landed() {
				rogue.AddComboPoints(sim, 1, target, spell.ComboPointMetrics())
			} else {
				spell.IssueRefund(sim)
			}
		},
	})
}
