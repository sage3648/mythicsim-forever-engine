package paladin

import (
	"github.com/wowsims/classic/sim/core"
)

// Twist of Light is new in Forever and borrows Echo of Light's spell id, after the Echo the
// tooltip names. Swapping seals normally throws the old one away; with the talent the seal the
// paladin cancels fires once more off the next melee attack.
// Seal of the Crusader is not on the tooltip's list and has no on-hit proc to echo either, so
// Seal of Fury, Seal of Righteousness and Seal of Command can bank one.
func (paladin *Paladin) registerTwistOfLight() {
	if !paladin.Talents.TwistOfLight {
		return
	}

	paladin.sealEchoAura = paladin.RegisterAura(core.Aura{
		Label:    "Twist of Light",
		ActionID: core.ActionID{SpellID: 77485},
		Duration: core.NeverExpires,
		OnSpellHitDealt: func(aura *core.Aura, sim *core.Simulation, spell *core.Spell, result *core.SpellResult) {
			if !result.Landed() || !spell.ProcMask.Matches(core.ProcMaskMelee) {
				return
			}

			echo := paladin.sealEcho
			paladin.sealEcho = nil
			aura.Deactivate(sim)
			echo(sim, result.Target)
		},
	})
}

// Called by each seal that carries an on-hit proc as it registers, with what its Echo does to the
// target of the next melee attack.
func (paladin *Paladin) registerSealProc(seal *core.Aura, proc func(*core.Simulation, *core.Unit)) {
	if paladin.sealProcs == nil {
		paladin.sealProcs = make(map[*core.Aura]func(*core.Simulation, *core.Unit))
	}
	paladin.sealProcs[seal] = proc
}

// Banks the seal being replaced, if the paladin has Twist of Light and the seal is one that
// leaves something behind to echo.
func (paladin *Paladin) bankSealEcho(sim *core.Simulation, newSeal *core.Aura) {
	if paladin.sealEchoAura == nil || newSeal == paladin.currentSeal || !paladin.currentSeal.IsActive() {
		return
	}

	proc, ok := paladin.sealProcs[paladin.currentSeal]
	if !ok {
		return
	}

	paladin.sealEcho = proc
	paladin.sealEchoAura.Activate(sim)
}
