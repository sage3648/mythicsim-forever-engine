package druid

import (
	"time"

	"github.com/wowsims/classic/sim/core"
	"github.com/wowsims/classic/sim/core/stats"
)

func (druid *Druid) registerTigersFurySpell() {
	actionID := core.ActionID{SpellID: map[int32]int32{
		25: 5217,
		40: 6793,
		50: 9845,
		60: 9846,
	}[druid.Level]}

	dmgBonus := map[int32]float64{
		25: 10.0,
		40: 20.0,
		50: 30.0,
		60: 40.0,
	}[druid.Level]

	// Forever pays a share of Physical damage rather than Classic's flat amount, so it
	// scales with the cat's weapon and attack power instead of fading as gear improves.
	// Read off Soda's Druid on 13 September: "Increases Physical damage done by 15% for
	// 6 sec", against Classic's "+40 damage to your melee attacks".
	foreverMultiplier := 1.15

	// The client table (see wrath.go) holds Forever's one rank: its duration and cooldown are read from there. The ids
	// by level stay ours.
	row := spellData.TigersFury.ByRank(1)

	druid.TigersFuryAura = druid.RegisterAura(core.Aura{
		Label:    "Tiger's Fury Aura",
		ActionID: actionID,
		Duration: row.Duration,
		OnGain: func(aura *core.Aura, sim *core.Simulation) {
			if druid.Env.IsForever() {
				druid.PseudoStats.SchoolDamageDealtMultiplier[stats.SchoolIndexPhysical] *= foreverMultiplier
			} else {
				druid.PseudoStats.BonusPhysicalDamage += dmgBonus
			}
		},
		OnExpire: func(aura *core.Aura, sim *core.Simulation) {
			if druid.Env.IsForever() {
				druid.PseudoStats.SchoolDamageDealtMultiplier[stats.SchoolIndexPhysical] /= foreverMultiplier
			} else {
				druid.PseudoStats.BonusPhysicalDamage -= dmgBonus
			}
		},
	})

	// Tagged so it does not collide with the metrics the 30 energy cost registers under
	// the same action. Two resource metrics sharing an id and type are indistinguishable
	// in the resources tab, and the concurrency combiner folds them into one.
	energyMetrics := druid.NewEnergyMetrics(actionID.WithTag(1))

	// Forever's King of the Jungle reads "Tiger's Fury now instantly grants you Energy", the
	// Wrath talent word for word, and Wrath's Tiger's Fury had no Energy cost and a 30 second
	// cooldown. Soda's Druid on 13 September confirmed both: the tooltip reads instant,
	// 30 sec cooldown, no cost, and "Increases Physical damage done by 15% for 6 sec".
	forever := druid.Env.IsForever()
	energyCost := core.TernaryFloat64(forever, 0, 30)
	cooldown := core.TernaryDuration(forever, row.Cooldown, time.Second)

	spell := druid.RegisterSpell(Cat, core.SpellConfig{
		ActionID: actionID,
		Flags:    core.SpellFlagAPL,

		EnergyCost: core.EnergyCostOptions{
			Cost: energyCost,
		},
		Cast: core.CastConfig{
			CD: core.Cooldown{
				Timer:    druid.NewTimer(),
				Duration: cooldown,
			},
		},

		ApplyEffects: func(sim *core.Simulation, _ *core.Unit, _ *core.Spell) {
			if druid.Talents.KingOfTheJungle > 0 {
				druid.AddEnergy(sim, 20*float64(druid.Talents.KingOfTheJungle), energyMetrics)
			}

			druid.TigersFuryAura.Activate(sim)
		},
	})

	druid.TigersFury = spell
}
