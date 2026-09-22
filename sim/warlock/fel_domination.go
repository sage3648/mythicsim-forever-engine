package warlock

import (
	"slices"
	"time"

	"github.com/wowsims/classic/sim/core"
)

func (warlock *Warlock) registerFelDominationCD() {
	if !warlock.Talents.FelDomination {
		return
	}

	actionID := core.ActionID{SpellID: 18708}

	castTimeMod := warlock.AddDynamicMod(core.SpellModConfig{
		Kind:      core.SpellMod_CastTime_Flat,
		ClassMask: SpellMaskSummonDemon,
		TimeValue: -time.Millisecond * 5500,
	})
	costMod := warlock.AddDynamicMod(core.SpellModConfig{
		Kind:       core.SpellMod_PowerCost_Pct_Add,
		ClassMask:  SpellMaskSummonDemon,
		FloatValue: -0.5,
	})

	aura := warlock.RegisterAura(core.Aura{
		ActionID: actionID,
		Label:    "Fel Domination",
		Duration: time.Second * 15,
		OnGain: func(aura *core.Aura, sim *core.Simulation) {
			castTimeMod.Activate()
			costMod.Activate()
		},
		OnExpire: func(aura *core.Aura, sim *core.Simulation) {
			castTimeMod.Deactivate()
			costMod.Deactivate()
		},
		OnCastComplete: func(aura *core.Aura, sim *core.Simulation, spell *core.Spell) {
			if slices.Contains(warlock.SummonDemonSpells, spell) {
				aura.Deactivate(sim)
			}
		},
	})

	spell := warlock.RegisterSpell(core.SpellConfig{
		ActionID:    actionID,
		SpellSchool: core.SpellSchoolShadow,
		ProcMask:    core.ProcMaskEmpty,

		Cast: core.CastConfig{
			CD: core.Cooldown{
				Timer:    warlock.NewTimer(),
				Duration: time.Minute * 5, // 15 min in Classic, 5 min in the beta client
			},
		},

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			aura.Activate(sim)
		},
	})

	warlock.AddMajorCooldown(core.MajorCooldown{
		Spell: spell,
		Type:  core.CooldownTypeUnknown,
	})
}
