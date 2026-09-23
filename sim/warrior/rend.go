package warrior

import (
	"github.com/wowsims/classic/sim/common/shared"
	"github.com/wowsims/classic/sim/core"
)

func (warrior *Warrior) registerRendSpell() {

	spellID := map[int32]int32{
		25: 6547,
		40: 11572,
		50: 11573,
		60: 11574,
	}[warrior.Level]

	// Forever beta client 1.60.1.69893: cost, school, defense type, tick, tick count and tick length come
	// from the client table; the id stays ours (see registerHeroicStrikeSpell).
	row := spellData.Rend.BySpellID(spellID)
	periodic := row.Periodic.(shared.SpellDataPeriodic)
	baseDamage := periodic.Tick

	// 12/23/35, not the 12/24/36 that multiplying rank 1 gives. Rank 3's 35% is confirmed
	// on the beta and rank 2's 23 is what the tree reads.
	damageMultiplier := []float64{1, 1.12, 1.23, 1.35}[warrior.Talents.ImprovedRend]

	warrior.Rend = warrior.RegisterSpell(BattleStance|DefensiveStance, core.SpellConfig{
		SpellCode:      SpellCode_WarriorRend,
		ClassSpellMask: SpellMaskRend,
		ActionID:       core.ActionID{SpellID: spellID},
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskMeleeMHSpecial,
		Flags:          core.SpellFlagAPL | core.SpellFlagNoOnCastComplete | SpellFlagOffensive,

		RageCost: core.RageCostOptions{
			Cost:   float64(row.Cost),
			Refund: 0.8,
		},
		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD: core.GCDDefault,
			},
		},

		DamageMultiplier: damageMultiplier,
		ThreatMultiplier: 1,

		Dot: core.DotConfig{
			Aura: core.Aura{
				Label: "Rend",
				Tag:   "Rend",
			},
			NumberOfTicks: periodic.NumberOfTicks,
			TickLength:    periodic.TickLength,
			// Periodic damage does not snapshot in Forever (wowsims/forever 36cfc58328): each tick takes
			// the modifiers of the moment it lands. Ticks crit live through OutcomeTick (row: Periodic Can Crit).
			OnTick: func(sim *core.Simulation, target *core.Unit, dot *core.Dot) {
				dot.Spell.CalcAndDealPeriodicDamage(sim, target, baseDamage, dot.OutcomeTick)
			},
		},

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			result := spell.CalcOutcome(sim, target, spell.OutcomeMeleeSpecialHitNoHitCounter)
			if result.Landed() {
				spell.Dot(target).Apply(sim)
			} else {
				spell.IssueRefund(sim)
			}

			spell.DealOutcome(sim, result)
		},
	})

}
