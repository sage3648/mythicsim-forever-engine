package paladin

import (
	"github.com/wowsims/classic/sim/core"
)

func (paladin *Paladin) registerDivineFavor() {
	if !paladin.Talents.DivineFavor {
		return
	}

	// Id, cooldown and the 100% crit come from the client table.
	row := spellData.DivineFavor.ByRank(1)

	critMod := paladin.AddDynamicMod(core.SpellModConfig{
		Kind:       core.SpellMod_BonusCrit_Percent,
		ClassMask:  SpellMaskHolyShock,
		FloatValue: row.Effects[0].Value,
	})

	cd := core.Cooldown{
		Timer:    paladin.NewTimer(),
		Duration: row.Cooldown,
	}

	aura := paladin.RegisterAura(core.Aura{
		Label:    "Divine Favor",
		ActionID: core.ActionID{SpellID: row.SpellID},
		Duration: core.NeverExpires,
		OnGain: func(aura *core.Aura, sim *core.Simulation) {
			critMod.Activate()
		},
		OnExpire: func(aura *core.Aura, sim *core.Simulation) {
			critMod.Deactivate()
		},
		OnSpellHitDealt: func(aura *core.Aura, sim *core.Simulation, spell *core.Spell, result *core.SpellResult) {
			if spell.SpellCode != SpellCode_PaladinHolyShock {
				return
			}
			// Remove the buff and put skill on CD
			aura.Deactivate(sim)
			cd.Set(sim.CurrentTime + cd.Duration)
			paladin.UpdateMajorCooldowns()
		},
	})

	divineFavor := paladin.RegisterSpell(core.SpellConfig{
		ActionID: aura.ActionID,
		Flags:    core.SpellFlagNoOnCastComplete,
		Cast: core.CastConfig{
			CD: cd,
		},
		ApplyEffects: func(sim *core.Simulation, _ *core.Unit, _ *core.Spell) {
			aura.Activate(sim)
		},
	})
	paladin.AddMajorCooldown(core.MajorCooldown{
		Spell: divineFavor,
		Type:  core.CooldownTypeDPS,
	})
}
