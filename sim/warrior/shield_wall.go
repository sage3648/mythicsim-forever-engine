package warrior

import (
	"time"

	"github.com/wowsims/classic/sim/core"
)

// TODO: Classic Update
func (warrior *Warrior) RegisterShieldWallCD() {
	// Forever trades strength for uptime: 60% for 12 sec against Classic's 75% for 10.
	// Confirmed by the beta client.
	// Forever beta client 1.60.1.69893: id, and Forever's duration and base cooldown, come from the
	// client table.
	row := spellData.ShieldWall.ByRank(1)
	forever := warrior.Env.IsForever()
	duration := core.TernaryDuration(forever, row.Duration, time.Second*10)
	//This is the inverse of the tooltip since it is a damage TAKEN coefficient
	damageTaken := core.TernaryFloat64(forever, 0.40, 0.25)

	actionID := core.ActionID{SpellID: row.SpellID}
	swAura := warrior.RegisterAura(core.Aura{
		Label:    "Shield Wall",
		ActionID: actionID,
		Duration: duration,
		OnGain: func(aura *core.Aura, sim *core.Simulation) {
			warrior.PseudoStats.DamageTakenMultiplier *= damageTaken
		},
		OnExpire: func(aura *core.Aura, sim *core.Simulation) {
			warrior.PseudoStats.DamageTakenMultiplier /= damageTaken
		},
	})

	// Improved Shield Wall reduces the cooldown instead of extending the duration in Forever,
	// 5.5 min per point (the beta client's curve reads 5.5 / 11 min), off a 15 min base.
	baseCooldown := core.TernaryDuration(forever, row.Cooldown, time.Minute*30)
	cooldownDur := baseCooldown - time.Second*330*time.Duration(warrior.Talents.ImprovedShieldWall)

	swSpell := warrior.RegisterSpell(DefensiveStance, core.SpellConfig{
		ActionID: actionID,

		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD: 0,
			},
			IgnoreHaste: true,
			CD: core.Cooldown{
				Timer:    warrior.NewTimer(),
				Duration: cooldownDur,
			},
		},
		ExtraCastCondition: func(sim *core.Simulation, target *core.Unit) bool {
			return warrior.PseudoStats.CanBlock
		},

		ApplyEffects: func(sim *core.Simulation, _ *core.Unit, spell *core.Spell) {
			swAura.Activate(sim)
		},
	})

	warrior.AddMajorCooldown(core.MajorCooldown{
		Spell: swSpell.Spell,
		Type:  core.CooldownTypeSurvival,
	})
}
