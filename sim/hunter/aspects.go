package hunter

import (
	"strconv"

	"github.com/wowsims/classic/sim/core"
	"github.com/wowsims/classic/sim/core/stats"
)

// Only Aspect of the Hawk is modelled. Forever changed Aspect of the Beast - rank 1 now
// adds 50 melee attack power on top of making you untrackable, where Classic gave only the
// untrackability - but a hunter holds one aspect at a time, and Hawk pays 120 ranged attack
// power at rank 7 to Beast's 50 melee. Nothing a ranged hunter does would pick Beast, so
// registering it would add a spell no rotation casts.

// Utility function to create the Deadly Aspects haste aura
func (hunter *Hunter) createDeadlyAspectsAura(auraLabel string, actionID core.ActionID) *core.Aura {
	// Every rank of Deadly Aspects triggers the same Quick Shots (6150): 30% for 12 sec. The
	// points buy only the proc chance. The duration comes from the client table.
	bonusMultiplier := 1.3
	return hunter.GetOrRegisterAura(core.Aura{
		Label:    auraLabel,
		ActionID: actionID,
		Duration: spellData.AspectOfTheHawkTriggered.ByRank(1).Duration,
		OnGain: func(aura *core.Aura, sim *core.Simulation) {
			aura.Unit.MultiplyRangedSpeed(sim, bonusMultiplier)
		},
		OnExpire: func(aura *core.Aura, sim *core.Simulation) {
			aura.Unit.MultiplyRangedSpeed(sim, 1/bonusMultiplier)
		},
	})
}

// Function to get the maximum attack power for Aspect of the Hawk based on rank
func (hunter *Hunter) getMaxAspectOfTheHawkAttackPower(rank int) float64 {
	// Rank 6 is 55 in the beta client, down from 110, while ranks 5 and 7 are unchanged. Read from
	// the client table.
	if rank < 1 || rank > 7 {
		return 0.0
	}

	attackPower, _ := spellData.AspectOfTheHawk.ByRank(int32(rank)).Direct.Range()
	return attackPower
}

func (hunter *Hunter) getMaxHawkRank() int {
	maxRank := core.TernaryInt(core.IncludeAQ, 7, 6)

	for i := maxRank; i > 0; i-- {
		config := hunter.getAspectOfTheHawkSpellConfig(i)
		if config.RequiredLevel <= int(hunter.Level) {
			return i
		}
	}
	return 1
}

func (hunter *Hunter) getAspectOfTheHawkSpellConfig(rank int) core.SpellConfig {
	var deadlyAspectsAura *core.Aura
	// Deadly Aspects: 2/4/6/8/10% (client curve).
	deadlyAspectsProcChance := 0.02 * float64(hunter.Talents.DeadlyAspects)

	// The id and mana cost (20-120) come from the client table.
	levels := [8]int{0, 10, 18, 28, 38, 48, 58, 60}

	row := spellData.AspectOfTheHawk.ByRank(int32(rank))
	level := levels[rank]

	if hunter.Talents.DeadlyAspects > 0 {
		deadlyAspectsAura = hunter.createDeadlyAspectsAura(
			"Quick Shots",
			core.ActionID{SpellID: 6150},
		)
	}
	// Use utility function to get the attack power based on rank
	rap := hunter.getMaxAspectOfTheHawkAttackPower(rank)

	actionID := core.ActionID{SpellID: row.SpellID}
	aspectOfTheHawkAura := hunter.GetOrRegisterAura(core.Aura{
		Label:    "Aspect of the Hawk" + strconv.Itoa(rank),
		ActionID: actionID,
		Duration: core.NeverExpires,
		OnGain: func(aura *core.Aura, sim *core.Simulation) {
			aura.Unit.AddStatDynamic(sim, stats.RangedAttackPower, rap*hunter.AspectOfTheHawkAPMultiplier)
		},
		OnExpire: func(aura *core.Aura, sim *core.Simulation) {
			aura.Unit.AddStatDynamic(sim, stats.RangedAttackPower, -rap*hunter.AspectOfTheHawkAPMultiplier)
		},
		OnSpellHitDealt: func(aura *core.Aura, sim *core.Simulation, spell *core.Spell, result *core.SpellResult) {
			if !spell.ProcMask.Matches(core.ProcMaskRangedAuto) {
				return
			}

			if deadlyAspectsAura != nil && sim.Proc(deadlyAspectsProcChance, "Deadly Aspects") {
				deadlyAspectsAura.Activate(sim)
			}
		},
	})

	aspectOfTheHawkAura.NewExclusiveEffect("Aspect", true, core.ExclusiveEffect{})

	return core.SpellConfig{
		ActionID:      actionID,
		Flags:         core.SpellFlagAPL,
		Rank:          rank,
		RequiredLevel: level,

		ManaCost: core.ManaCostOptions{
			FlatCost: float64(row.Cost),
		},
		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD: core.GCDDefault,
			},
		},
		ExtraCastCondition: func(sim *core.Simulation, target *core.Unit) bool {
			return !aspectOfTheHawkAura.IsActive()
		},

		ApplyEffects: func(sim *core.Simulation, _ *core.Unit, _ *core.Spell) {
			aspectOfTheHawkAura.Activate(sim)
		},
	}
}

func (hunter *Hunter) registerAspectOfTheHawkSpell() {
	hunter.AspectOfTheHawkAPMultiplier = 1.0
	maxRank := hunter.getMaxHawkRank()
	config := hunter.getAspectOfTheHawkSpellConfig(maxRank)
	hunter.GetOrRegisterSpell(config)
}
