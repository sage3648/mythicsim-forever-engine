package paladin

import (
	"github.com/wowsims/classic/sim/core"
)

// Swift Judgement is new in Forever and borrows Judgements of the Pure's spell id; the beta client's
// own is 1310994. It hands the paladin a Judgement back and pays for it. The client confirms the
// 1 min cooldown and the 100% cost reduction.
func (paladin *Paladin) registerSwiftJudgement() {
	if !paladin.Talents.SwiftJudgement {
		return
	}

	actionID := core.ActionID{SpellID: 53671}

	freeMod := paladin.AddDynamicMod(core.SpellModConfig{
		Kind:       core.SpellMod_PowerCost_Pct_Add,
		ClassMask:  SpellMaskJudgement,
		FloatValue: -1,
	})

	freeJudgementAura := paladin.RegisterAura(core.Aura{
		Label:    "Swift Judgement",
		ActionID: actionID,
		Duration: core.NeverExpires,
		OnGain: func(aura *core.Aura, sim *core.Simulation) {
			freeMod.Activate()
		},
		OnExpire: func(aura *core.Aura, sim *core.Simulation) {
			freeMod.Deactivate()
		},
		OnCastComplete: func(aura *core.Aura, sim *core.Simulation, spell *core.Spell) {
			if spell == paladin.judgement {
				aura.Deactivate(sim)
			}
		},
	})

	swiftJudgement := paladin.RegisterSpell(core.SpellConfig{
		ActionID: actionID,
		Flags:    core.SpellFlagNoOnCastComplete | core.SpellFlagAPL,

		// The cooldown comes from the client table (1310994); the id stays ours.
		Cast: core.CastConfig{
			CD: core.Cooldown{
				Timer:    paladin.NewTimer(),
				Duration: spellData.SwiftJudgement.ByRank(1).Cooldown,
			},
		},

		ApplyEffects: func(sim *core.Simulation, _ *core.Unit, _ *core.Spell) {
			paladin.judgement.CD.Set(sim.CurrentTime)
			freeJudgementAura.Activate(sim)
		},
	})

	paladin.AddMajorCooldown(core.MajorCooldown{
		Spell: swiftJudgement,
		Type:  core.CooldownTypeDPS,
		ShouldActivate: func(sim *core.Simulation, _ *core.Character) bool {
			// Nothing to finish if Judgement is already up, and the free cast is wasted
			// without a seal to spend.
			return !paladin.judgement.CD.IsReady(sim) && paladin.currentSeal != nil && paladin.currentSeal.IsActive()
		},
	})
}
