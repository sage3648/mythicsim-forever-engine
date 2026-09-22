package rogue

import (
	"time"

	"github.com/wowsims/classic/sim/common/shared"
	"github.com/wowsims/classic/sim/core"
)

// Cost, school, defense type, tick, tick count and tick length come from the client table; the id
// stays ours (see sinister_strike.go).
func (rogue *Rogue) registerGarrote() {
	spellID := map[int32]int32{
		25: 8631,
		40: 8633,
		50: 11289,
		60: 11290,
	}[rogue.Level]

	row := spellData.Garrote.BySpellID(spellID)
	periodic := row.Periodic.(shared.SpellDataPeriodic)

	rogue.Garrote = rogue.GetOrRegisterSpell(core.SpellConfig{
		SpellCode:      SpellCode_RogueGarrote,
		ClassSpellMask: SpellMaskGarrote,
		ActionID:       core.ActionID{SpellID: spellID},
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskMeleeMHSpecial,
		Flags:          SpellFlagBuilder | core.SpellFlagMeleeMetrics | core.SpellFlagAPL,

		EnergyCost: core.EnergyCostOptions{
			Cost:   float64(row.Cost) - 10*float64(rogue.Talents.DirtyDeeds),
			Refund: 0.8,
		},
		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD: time.Second,
			},
			IgnoreHaste: true,
		},
		ExtraCastCondition: func(sim *core.Simulation, target *core.Unit) bool {
			if !rogue.IsStealthed() {
				return false
			}
			// Dirty Deeds drops the positional requirement.
			return rogue.Talents.DirtyDeeds > 0 || !rogue.PseudoStats.InFrontOfTarget
		},

		DamageMultiplier: 1 +
			0.05*float64(rogue.Talents.Opportunity),
		ThreatMultiplier: 1,

		Dot: core.DotConfig{
			Aura: core.Aura{
				Label: "Garrote",
			},
			NumberOfTicks: periodic.NumberOfTicks,
			TickLength:    periodic.TickLength,
			OnSnapshot: func(sim *core.Simulation, target *core.Unit, dot *core.Dot, isRollover bool) {
				damage := periodic.Tick + dot.Spell.MeleeAttackPower(target)*0.03
				dot.Snapshot(target, damage, isRollover)
			},
			OnTick: func(sim *core.Simulation, target *core.Unit, dot *core.Dot) {
				dot.CalcAndDealPeriodicSnapshotDamage(sim, target, dot.OutcomeTick)
			},
		},

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			rogue.BreakStealth(sim)
			result := spell.CalcOutcome(sim, target, spell.OutcomeMeleeSpecialNoBlockDodgeParryNoCritNoHitCounter)
			if result.Landed() {
				rogue.AddComboPoints(sim, 1, target, spell.ComboPointMetrics())
				spell.Dot(target).Apply(sim)
			} else {
				spell.IssueRefund(sim)
			}
			spell.DealOutcome(sim, result)
		},
	})
}
