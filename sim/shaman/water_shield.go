package shaman

import (
	"time"

	"github.com/wowsims/classic/sim/core"
)

func (shaman *Shaman) registerWaterShieldSpell() {
	if !shaman.Talents.WaterShield {
		return
	}

	// Beta client (408510): three globes of 2% maximum mana, one every 3.5 sec at most (the aura's proc
	// recovery, the same as Lightning Shield's), no mana cost and a 15 sec cooldown.
	actionID := core.ActionID{SpellID: 408510}
	manaMetrics := shaman.NewManaMetrics(actionID)
	globes := int32(3)

	icd := core.Cooldown{
		Timer:    shaman.NewTimer(),
		Duration: time.Millisecond * 3500,
	}

	consumeGlobe := func(sim *core.Simulation) {
		if !icd.IsReady(sim) {
			return
		}

		icd.Use(sim)
		shaman.AddMana(sim, shaman.MaxMana()*0.02, manaMetrics)
		shaman.WaterShieldAura.RemoveStack(sim)
	}

	shaman.WaterShieldAura = shaman.RegisterAura(core.Aura{
		Label:     "Water Shield",
		ActionID:  actionID,
		Duration:  time.Minute * 10,
		MaxStacks: globes,
		OnGain: func(aura *core.Aura, sim *core.Simulation) {
			aura.SetStacks(sim, globes)
		},
		OnExpire: func(aura *core.Aura, sim *core.Simulation) {
			if shaman.ActiveShieldAura.ActionID == aura.ActionID {
				shaman.ActiveShieldAura = nil
				shaman.ActiveShield = nil
			}
		},
		OnSpellHitTaken: func(aura *core.Aura, sim *core.Simulation, spell *core.Spell, result *core.SpellResult) {
			if result.Landed() {
				consumeGlobe(sim)
			}
		},
		OnHealDealt: func(aura *core.Aura, sim *core.Simulation, spell *core.Spell, result *core.SpellResult) {
			if result.DidCrit() {
				consumeGlobe(sim)
			}
		},
	})

	shaman.WaterShield = shaman.RegisterSpell(core.SpellConfig{
		SpellCode:      SpellCode_ShamanWaterShield,
		ClassSpellMask: SpellMaskWaterShield,
		ActionID:       actionID,
		ProcMask:       core.ProcMaskEmpty,
		Flags:          core.SpellFlagAPL | SpellFlagShaman,

		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD: core.GCDDefault,
			},
			CD: core.Cooldown{
				Timer:    shaman.NewTimer(),
				Duration: time.Second * 15,
			},
		},
		ApplyEffects: func(sim *core.Simulation, _ *core.Unit, spell *core.Spell) {
			if shaman.ActiveShieldAura != nil {
				shaman.ActiveShieldAura.Deactivate(sim)
			}
			shaman.ActiveShield = spell
			shaman.ActiveShieldAura = shaman.WaterShieldAura
			shaman.ActiveShieldAura.Activate(sim)
		},
	})
}
