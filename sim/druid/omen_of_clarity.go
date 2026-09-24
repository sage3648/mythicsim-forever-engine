package druid

import (
	"time"

	"github.com/wowsims/classic/sim/common/shared"
	"github.com/wowsims/classic/sim/core"
)

// The spells Clearcasting's (16870) cost modifier names, as the sim knows them. Client 1.60.1.69977's
// mask [15973374, 100664659, 1024, 142606400] covers Moonfire, Starfire, Insect Swarm, Hurricane,
// Demoralizing Roar, Maul and Swipe (one bit), Claw and Rake (one bit), Shred, Rip and Ferocious Bite
// (one bit) and Mangle, besides heals and spells the sim does not have. Wrath, Faerie Fire,
// Lacerate, Tiger's Fury, Berserk and the forms are not in it. The cat's Mangle shares the Bear
// Mangle's mask: it stands in for the one Mangle Forever has.
const clearcastingSpells = SpellMaskMoonfire | SpellMaskStarfire | SpellMaskInsectSwarm | SpellMaskHurricane |
	SpellMaskDemoralizingRoar | SpellMaskMaul | SpellMaskSwipe | SpellMaskClaw | SpellMaskRake | SpellMaskShred |
	SpellMaskRip | SpellMaskFerociousBite | SpellMaskMangle

// ASSUMPTION: the client states no proc rate for Omen of Clarity (16864's chance column of 100 is the
// "no roll here" convention, and no effect carries a rate). 2 procs a minute is the rate the new
// engine line took from upstream's port, and it is used here until the client or a measurement
// gives one. It is measured on the paw swing for white hits (the weapon when there is no paw), on
// the equipped weapon's speed for melee abilities, and on the cast time, at least a global
// cooldown, for spells.
const omenOfClarityPPM = 2.0

// 16864's SpellAuraOptions cooldown, 10 sec.
const omenOfClarityICD = time.Second * 10

// Moonkin Form (24858) effects 4 and 5: +100% to Omen of Clarity's chance (SPELLMOD_CHANCE_OF_SUCCESS)
// and -50% to its proc cooldown.
const (
	moonkinOmenChanceMultiplier   = 2.0
	moonkinOmenCooldownMultiplier = 0.5
)

// Omen of Clarity (16864), a baseline passive from level 20 in Forever rather than a talent: melee
// swings, melee abilities and spells, harmful or healing (proc flags 0x14014), can grant Clearcasting
// (16870), which makes the next ability in its mask free for 15 sec and is spent by it (one charge).
func (druid *Druid) applyOmenOfClarity() {
	omen := spellData.OmenOfClarity.ByRank(1)
	clearcasting := spellData.OmenOfClarityTriggered.ByRank(1)
	if druid.Level < 20 {
		return
	}

	costMod := druid.AddDynamicMod(core.SpellModConfig{
		Kind:       core.SpellMod_PowerCost_Pct_Add,
		ClassMask:  clearcastingSpells,
		FloatValue: clearcasting.Effect(shared.A_ADD_PCT_MODIFIER, 14).Value / 100, // SPELLMOD_COST, -100%
	})

	// The spell whose hit last granted Clearcasting, and when: that cast must not spend the charge it
	// just earned, since our OnCastComplete runs after the cast's hits.
	var procSpell *core.Spell
	var procAt time.Duration

	druid.ClearcastingAura = druid.RegisterAura(core.Aura{
		Label:    "Clearcasting",
		ActionID: core.ActionID{SpellID: clearcasting.SpellID},
		Duration: clearcasting.Duration,
		OnReset: func(_ *core.Aura, _ *core.Simulation) {
			procSpell = nil
		},
		OnGain: func(_ *core.Aura, _ *core.Simulation) {
			costMod.Activate()
		},
		OnExpire: func(_ *core.Aura, _ *core.Simulation) {
			costMod.Deactivate()
		},
		OnCastComplete: func(_ *core.Aura, sim *core.Simulation, spell *core.Spell) {
			if spell == procSpell && sim.CurrentTime == procAt {
				return
			}
			druid.spendClearcasting(sim, spell)
		},
	})

	icd := core.Cooldown{
		Timer:    druid.NewTimer(),
		Duration: omenOfClarityICD,
	}

	tryProc := func(sim *core.Simulation, spell *core.Spell, result *core.SpellResult) {
		if !result.Landed() || !spell.ProcMask.Matches(core.ProcMaskMelee|core.ProcMaskSpellDamage|core.ProcMaskSpellHealing) {
			return
		}
		if spell.ProcMask.Matches(core.ProcMaskProc | core.ProcMaskDamageProc) {
			return
		}
		if !icd.IsReady(sim) {
			return
		}

		var seconds float64
		switch {
		case spell.ProcMask.Matches(core.ProcMaskSpellDamage | core.ProcMaskSpellHealing):
			seconds = max(spell.DefaultCast.CastTime, core.GCDDefault).Seconds()
		case spell.ProcMask.Matches(core.ProcMaskMeleeWhiteHit) || !druid.HasMHWeapon():
			seconds = druid.AutoAttacks.MH().SwingSpeed
		default:
			seconds = druid.GetMHWeapon().SwingSpeed
		}

		chance := omenOfClarityPPM * seconds / 60
		cooldown := omenOfClarityICD
		if druid.MoonkinFormAura != nil && druid.MoonkinFormAura.IsActive() {
			chance *= moonkinOmenChanceMultiplier
			cooldown = time.Duration(float64(cooldown) * moonkinOmenCooldownMultiplier)
		}

		if !sim.Proc(chance, "Omen of Clarity") {
			return
		}

		icd.Duration = cooldown
		icd.Use(sim)
		procSpell, procAt = spell, sim.CurrentTime
		druid.ClearcastingAura.Activate(sim)
	}

	omenAura := core.MakePermanent(druid.RegisterAura(core.Aura{
		Label:    "Omen of Clarity",
		ActionID: core.ActionID{SpellID: omen.SpellID},
		OnSpellHitDealt: func(_ *core.Aura, sim *core.Simulation, spell *core.Spell, result *core.SpellResult) {
			tryProc(sim, spell, result)
		},
		OnHealDealt: func(_ *core.Aura, sim *core.Simulation, spell *core.Spell, result *core.SpellResult) {
			tryProc(sim, spell, result)
		},
	}))
	omenAura.Icd = &icd
}

// Spends Clearcasting on a cast it made free: a spell in its mask that has a cost and paid nothing
// for it. A cast that paid (Clearcasting came up during it) leaves the charge alone.
func (druid *Druid) spendClearcasting(sim *core.Simulation, spell *core.Spell) {
	if druid.ClearcastingAura == nil || !druid.ClearcastingAura.IsActive() {
		return
	}
	if !spell.Matches(clearcastingSpells) || spell.Cost == nil || spell.Cost.BaseCost <= 0 || spell.CurCast.Cost > 0 {
		return
	}
	druid.ClearcastingAura.Deactivate(sim)
}
