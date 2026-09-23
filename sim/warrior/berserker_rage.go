package warrior

import (
	"github.com/wowsims/classic/sim/core"
)

func (warrior *Warrior) registerBerserkerRageSpell() {
	if warrior.Level < 32 {
		return
	}

	// Forever beta client 1.60.1.69893: id, cooldown and duration come from the client table.
	row := spellData.BerserkerRage.ByRank(1)
	actionID := core.ActionID{SpellID: row.SpellID}
	rageMetrics := warrior.NewRageMetrics(actionID)
	instantRage := 5 * float64(warrior.Talents.ImprovedBerserkerRage)

	warrior.BerserkerRageAura = warrior.RegisterAura(core.Aura{
		Label:    "Berserker Rage",
		ActionID: actionID,
		Duration: row.Duration,

		// Forever: rage from damage taken is doubled while it is up (wowsims/forever f9f9f21883; the
		// client states no amount, they flag it for an in-game test).
		OnGain: func(aura *core.Aura, sim *core.Simulation) {
			if sim.IsForever() {
				warrior.AddDamageTakenRageMultiplier(2)
			}
		},
		OnExpire: func(aura *core.Aura, sim *core.Simulation) {
			if sim.IsForever() {
				warrior.AddDamageTakenRageMultiplier(0.5)
			}
		},
		OnSpellHitTaken: func(aura *core.Aura, sim *core.Simulation, spell *core.Spell, result *core.SpellResult) {
			if sim.IsForever() || !result.Landed() || result.Damage <= 0 {
				return
			}

			if spell.ProcMask.Matches(core.ProcMaskMeleeOrRanged) {
				// For melee attacks we find that it gives around 2.0 extra rage regardless of attacker level
				// This may give less rage for fast attacks, but we default to 2.0 for now
				warrior.AddRage(sim, 2.0, rageMetrics)
			} else if spell.ProcMask.Matches(core.ProcMaskSpellDamage) {
				// Spell attacks generally give 1 - 2 times unmodified damage as additional rage.
				rageConversionDamageTaken := core.GetRageConversion(spell.Unit.Level)
				generatedRage := result.RawDamage() * 2.5 / rageConversionDamageTaken
				generatedRage *= 1.0 // Using 1.0 because we don't know why it gives more sometimes
				warrior.AddRage(sim, generatedRage, rageMetrics)
			}
		},
	})

	warrior.BerserkerRage = warrior.RegisterSpell(BerserkerStance, core.SpellConfig{
		ActionID: actionID,

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
		ApplyEffects: func(sim *core.Simulation, _ *core.Unit, spell *core.Spell) {
			if instantRage > 0 {
				warrior.AddRage(sim, instantRage, rageMetrics)
			}
			warrior.BerserkerRageAura.Activate(sim)
		},
	})

	warrior.AddMajorCooldown(core.MajorCooldown{
		Spell: warrior.BerserkerRage.Spell,
		Type:  core.CooldownTypeSurvival,
	})
}
