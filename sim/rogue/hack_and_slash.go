package rogue

import (
	"time"

	"github.com/wowsims/classic/sim/core"
	"github.com/wowsims/classic/sim/core/proto"
	"github.com/wowsims/classic/sim/core/stats"
)

// Hack and Slash folds the four Classic weapon specialization talents into one, and picks
// its effect from the weapons the rogue has equipped. All three effects scale linearly with
// rank in the beta client: 1% extra attack, 1% crit and 3% armor ignored per rank.
func (rogue *Rogue) applyHackAndSlash() {
	if rogue.Talents.HackAndSlash == 0 {
		return
	}

	points := float64(rogue.Talents.HackAndSlash)

	// Axes and swords: extra attack.
	if mask := rogue.GetProcMaskForTypes(proto.WeaponType_WeaponTypeAxe, proto.WeaponType_WeaponTypeSword); mask != core.ProcMaskUnknown {
		rogue.registerHackAndSlashExtraAttack(mask, 0.01*points)
	}

	// Daggers and fists: crit. Bonus is shown if the main hand qualifies, but not if the off hand only does.
	switch rogue.GetProcMaskForTypes(proto.WeaponType_WeaponTypeDagger, proto.WeaponType_WeaponTypeFist) {
	case core.ProcMaskMelee:
		rogue.AddStat(stats.MeleeCrit, core.CritRatingPerCritChance*points)
	case core.ProcMaskMeleeMH:
		// the default character pane displays critical strike chance for main hand only
		rogue.AddStat(stats.MeleeCrit, core.CritRatingPerCritChance*points)
		rogue.AddStaticMod(core.SpellModConfig{
			Kind:       core.SpellMod_BonusCrit_Percent,
			ProcMask:   core.ProcMaskMeleeOH,
			FloatValue: -points,
		})
	case core.ProcMaskMeleeOH:
		rogue.AddStaticMod(core.SpellModConfig{
			Kind:       core.SpellMod_BonusCrit_Percent,
			ProcMask:   core.ProcMaskMeleeOH,
			FloatValue: points,
		})
	}

	// Maces: armor ignore.
	if mask := rogue.GetProcMaskForTypes(proto.WeaponType_WeaponTypeMace); mask != core.ProcMaskUnknown {
		rogue.PseudoStats.ArmorIgnorePercent += 0.03 * points
	}
}

func (rogue *Rogue) registerHackAndSlashExtraAttack(mask core.ProcMask, procChance float64) {
	icd := core.Cooldown{
		Timer:    rogue.NewTimer(),
		Duration: time.Millisecond * 200,
	}

	rogue.RegisterAura(core.Aura{
		Label:    "Hack and Slash",
		Duration: core.NeverExpires,
		OnReset: func(aura *core.Aura, sim *core.Simulation) {
			aura.Activate(sim)
		},
		OnSpellHitDealt: func(aura *core.Aura, sim *core.Simulation, spell *core.Spell, result *core.SpellResult) {
			if !result.Landed() {
				return
			}
			if !spell.ProcMask.Matches(mask) {
				return
			}
			if !icd.IsReady(sim) {
				return
			}
			if sim.RandomFloat("Hack and Slash") < procChance {
				icd.Use(sim)
				rogue.AutoAttacks.ExtraMHAttack(sim, 1, core.ActionID{SpellID: 13964}, spell.ActionID)
			}
		},
	})
}
