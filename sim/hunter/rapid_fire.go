package hunter

import (
	"time"

	"github.com/wowsims/classic/sim/core"
)

func (hunter *Hunter) registerRapidFire() {
	if hunter.Level < 26 {
		return
	}

	// Spell ID, cost, cooldown and duration from the client table (see aimed_shot.go). The 40% stays
	// ours rather than read a damage field as a haste multiplier.
	row := spellData.RapidFire.ByRank(1)
	actionID := core.ActionID{SpellID: row.SpellID}
	// Rapid Killing takes 1 min off a rank (client curve 60000/120000 ms). The buff a kill grants
	// (415407: 20% on the next Shot within 20 sec) is not modelled, nothing dies in a boss fight.
	cooldown := row.Cooldown - time.Minute*time.Duration(hunter.Talents.RapidKilling)

	hunter.RapidFireAura = hunter.RegisterAura(core.Aura{
		Label:    "Rapid Fire",
		ActionID: actionID,
		Duration: row.Duration,

		// Forever: ranged and melee attack speed, where Classic's was ranged only.
		OnGain: func(aura *core.Aura, sim *core.Simulation) {
			aura.Unit.MultiplyAttackSpeed(sim, 1.4)
		},
		OnExpire: func(aura *core.Aura, sim *core.Simulation) {
			aura.Unit.MultiplyAttackSpeed(sim, 1/1.4)
		},
	})

	hunter.RapidFire = hunter.RegisterSpell(core.SpellConfig{
		ActionID: actionID,

		ManaCost: core.ManaCostOptions{
			FlatCost: float64(row.Cost),
		},
		Cast: core.CastConfig{
			CD: core.Cooldown{
				Timer:    hunter.NewTimer(),
				Duration: cooldown,
			},
		},

		ApplyEffects: func(sim *core.Simulation, _ *core.Unit, _ *core.Spell) {
			hunter.RapidFireAura.Activate(sim)
		},
	})

	hunter.AddMajorCooldown(core.MajorCooldown{
		Spell: hunter.RapidFire,
		Type:  core.CooldownTypeDPS,
	})
}
