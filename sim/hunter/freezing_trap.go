package hunter

import (
	"time"

	"github.com/wowsims/classic/sim/core"
)

func (hunter *Hunter) getFreezingTrapConfig(timer *core.Timer) core.SpellConfig {
	// Cost and cooldown from the trap's row of the client table, school and defense type from its
	// effect's (see aimed_shot.go). The id stays spelled out: ranks 2-3 are never registered, and
	// spell_sources_test.go would file them as ours.
	row := spellData.FreezingTrap.ByRank(1)
	effect := spellData.FreezingTrapTriggered.ByRank(1)

	return core.SpellConfig{
		SpellCode:      SpellCode_HunterFreezingTrap,
		ClassSpellMask: SpellMaskFreezingTrap,
		ActionID:       core.ActionID{SpellID: 1499},
		SpellSchool:    effect.SpellSchool,
		DefenseType:    effect.DefenseType,
		ProcMask:       core.ProcMaskSpellDamage,
		Flags:          core.SpellFlagAPL | SpellFlagTrap,
		RequiredLevel:  20,
		MissileSpeed:   24,

		ManaCost: core.ManaCostOptions{
			FlatCost: float64(row.Cost),
		},
		Cast: core.CastConfig{
			CD: core.Cooldown{
				Timer: timer,
				// Forever doubles the shared trap cooldown to 30 sec. Seen on every trap tooltip
				// from the demo streams (Savix, Xaryu and Soda, 12-13 September).
				Duration: core.TernaryDuration(hunter.Env.IsForever(), row.Cooldown, time.Second*15),
			},
			DefaultCast: core.Cast{
				GCD: core.GCDDefault,
			},
			IgnoreHaste: true, // Hunter GCD is locked at 1.5s
		},

		DamageMultiplier: 1,
		ThreatMultiplier: 1,

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
		},
	}
}

func (hunter *Hunter) registerFreezingTrapSpell(timer *core.Timer) {
	config := hunter.getFreezingTrapConfig(timer)

	if config.RequiredLevel <= int(hunter.Level) {
		hunter.FreezingTrap = hunter.GetOrRegisterSpell(config)
	}
}
