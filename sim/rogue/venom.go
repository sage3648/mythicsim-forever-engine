package rogue

import (
	"time"

	"github.com/wowsims/classic/sim/core"
)

func (rogue *Rogue) registerVenom() {
	if !rogue.Talents.Venom {
		return
	}

	// Forever beta client 1.60.1.69893: id and cost come from the client table. Its 6s duration is the
	// base before the per combo point step, so the durations stay ours.
	row := spellData.Venom.ByRank(1)
	actionID := core.ActionID{SpellID: row.SpellID}

	durations := [6]time.Duration{
		0,
		time.Second * 9,
		time.Second * 12,
		time.Second * 15,
		time.Second * 18,
		time.Second * 21,
	}

	rogue.VenomAura = rogue.RegisterAura(core.Aura{
		Label:    "Venom",
		ActionID: actionID,
		// This will be overridden on cast, but set a non-zero default so it doesn't crash when used in APL prepull
		Duration: durations[5],
		OnGain: func(aura *core.Aura, sim *core.Simulation) {
			rogue.additivePoisonBonusChance += 0.1
			rogue.multiplyPoisonDamage(1.3)
		},
		OnExpire: func(aura *core.Aura, sim *core.Simulation) {
			rogue.additivePoisonBonusChance -= 0.1
			rogue.multiplyPoisonDamage(1 / 1.3)
		},
	})

	rogue.Venom = rogue.RegisterSpell(core.SpellConfig{
		SpellCode:      SpellCode_RogueVenom,
		ClassSpellMask: SpellMaskVenom,
		ActionID:       actionID,
		Flags:          rogue.finisherFlags(),
		MetricSplits:   6,

		EnergyCost: core.EnergyCostOptions{
			Cost: float64(row.Cost),
		},
		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD: time.Second,
			},
			IgnoreHaste: true,
			ModifyCast: func(sim *core.Simulation, spell *core.Spell, cast *core.Cast) {
				spell.SetMetricsSplit(spell.Unit.ComboPoints())
			},
		},
		ExtraCastCondition: func(sim *core.Simulation, target *core.Unit) bool {
			return rogue.ComboPoints() > 0
		},

		ApplyEffects: func(sim *core.Simulation, _ *core.Unit, spell *core.Spell) {
			rogue.BreakStealth(sim)
			rogue.VenomAura.Duration = durations[rogue.ComboPoints()]
			rogue.VenomAura.Activate(sim)
			rogue.SpendComboPoints(sim, spell)
		},
	})
	rogue.Finishers = append(rogue.Finishers, rogue.Venom)
}
