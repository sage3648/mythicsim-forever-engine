package mage

import (
	"time"

	"github.com/wowsims/classic/sim/core"
)

// Ignite pays out a fixed share of the critical strike that lit it, split over two ticks. When a
// second crit lands while the dot is still running the damage it still owes rolls into the new
// one, so a crit is never paid twice and never dropped.
const IgniteTicks = 2

func (mage *Mage) applyIgnite() {
	if mage.Talents.Ignite == 0 {
		return
	}

	igniteShare := .08 * float64(mage.Talents.Ignite)
	pendingIgniteDamage := 0.0

	mage.RegisterAura(core.Aura{
		Label:    "Ignite Talent",
		Duration: core.NeverExpires,
		OnReset: func(aura *core.Aura, sim *core.Simulation) {
			pendingIgniteDamage = 0
			aura.Activate(sim)
		},
		OnSpellHitDealt: func(aura *core.Aura, sim *core.Simulation, spell *core.Spell, result *core.SpellResult) {
			if !spell.ProcMask.Matches(core.ProcMaskSpellDamage) {
				return
			}
			if !spell.SpellSchool.Matches(core.SpellSchoolFire) || !result.DidCrit() {
				return
			}

			dot := mage.igniteTick.Dot(result.Target)
			rollover := 0.0
			if dot.IsActive() {
				rollover = dot.SnapshotBaseDamage * float64(dot.MaxTicksRemaining())
			}

			pendingIgniteDamage = rollover + result.Damage*igniteShare
			mage.Ignite.Cast(sim, result.Target)
		},
	})

	mage.Ignite = mage.RegisterSpell(core.SpellConfig{
		ActionID: core.ActionID{SpellID: 12654},
		Flags:    core.SpellFlagNoOnCastComplete | core.SpellFlagPassiveSpell | SpellFlagMage,

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			mage.igniteTick.Dot(target).Apply(sim)
		},
	})

	// The share is taken from a hit that has already been through every one of the mage's damage
	// multipliers and the target's, so the ticks skip both rather than pay them a second time.
	// Dots can crit under the Forever ruleset, but Ignite's damage already carries the crit
	// multiplier of the strike that lit it, so it opts out of that too.
	mage.igniteTick = mage.RegisterSpell(core.SpellConfig{
		SpellCode:      SpellCode_MageIgnite,
		ClassSpellMask: SpellMaskIgnite,
		ActionID:       core.ActionID{SpellID: 12654},
		SpellSchool:    core.SpellSchoolFire,
		DefenseType:    core.DefenseTypeMagic,
		ProcMask:       core.ProcMaskSpellProc,
		Flags:          core.SpellFlagNoOnCastComplete | core.SpellFlagPassiveSpell | core.SpellFlagNoPeriodicCrit | core.SpellFlagIgnoreModifiers | SpellFlagMage,

		DamageMultiplier: 1,
		ThreatMultiplier: 1,

		Cast: core.CastConfig{
			IgnoreHaste: true,
		},

		Dot: core.DotConfig{
			Aura: core.Aura{
				Label: "Ignite",
			},
			NumberOfTicks: IgniteTicks,
			TickLength:    time.Second * 2,

			OnSnapshot: func(sim *core.Simulation, target *core.Unit, dot *core.Dot, _ bool) {
				dot.SnapshotBaseDamage = pendingIgniteDamage / IgniteTicks
				dot.SnapshotAttackerMultiplier = 1
			},
			OnTick: func(sim *core.Simulation, target *core.Unit, dot *core.Dot) {
				dot.CalcAndDealPeriodicSnapshotDamage(sim, target, dot.OutcomeTick)
			},
		},
	})
}
