package druid

import (
	"time"

	"github.com/wowsims/classic/sim/common/shared"
	"github.com/wowsims/classic/sim/core"
)

// Beta client 1.60.1.69893: the per combo point damage moved at every rank and the base at ranks 4-6 (rank 6 17 +
// 28 a point a tick -> 15 + 25.5). The client stores no attack power scaling, so the sim's is unchanged. The id, cost,
// base tick and tick schedule come from the client table (see wrath.go); the table holds nothing per combo point, so
// that stays ours.
var RipLevel = []int32{0, 20, 28, 36, 44, 52, 60}
var RipTickPerCombo = []float64{0, 4.4, 7.2, 8.5, 12.7, 18.2, 25.5}

// Every rank ticks as often; the feral rotation reads this.
var RipTicks = spellData.Rip.HighestRank().Periodic.(shared.SpellDataPeriodic).NumberOfTicks

func (druid *Druid) registerRipSpell() {
	// Add highest available Rip rank for level.
	for rank := len(RipLevel) - 1; rank >= 1; rank-- {
		if druid.Level >= RipLevel[rank] {
			config := druid.newRipSpellConfig(rank)
			druid.Rip = druid.RegisterSpell(Cat, config)
			return
		}
	}
}

func (druid *Druid) newRipSpellConfig(rank int) core.SpellConfig {
	row := spellData.Rip.ByRank(int32(rank))
	periodic := row.Periodic.(shared.SpellDataPeriodic)
	dmgTickPerCombo := RipTickPerCombo[rank]

	return core.SpellConfig{
		SpellCode:      SpellCode_DruidRip,
		ClassSpellMask: SpellMaskRip,
		ActionID:       core.ActionID{SpellID: row.SpellID},
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskMeleeMHSpecial,
		Flags:          core.SpellFlagMeleeMetrics | core.SpellFlagAPL | core.SpellFlagPureDot,

		EnergyCost: core.EnergyCostOptions{
			Cost:   float64(row.Cost),
			Refund: 0.8,
		},
		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD: time.Second,
			},
			IgnoreHaste: true,
		},
		ExtraCastCondition: func(sim *core.Simulation, target *core.Unit) bool {
			return druid.ComboPoints() > 0
		},

		DamageMultiplier: 1,
		ThreatMultiplier: 1,

		Dot: core.DotConfig{
			Aura: core.Aura{
				Label: "Rip",
			},
			NumberOfTicks: periodic.NumberOfTicks,
			TickLength:    periodic.TickLength,

			OnSnapshot: func(sim *core.Simulation, target *core.Unit, dot *core.Dot, isRollover bool) {
				cp := float64(druid.ComboPoints())
				cpScaling := core.TernaryFloat64(cp == 5, 4, cp)
				baseDamage := periodic.Tick + dmgTickPerCombo*cp
				// AP scaling is 6% per combo point from 1 to 4, and 24% again for 5
				tickDamage := baseDamage + 0.01*cpScaling*dot.Spell.MeleeAttackPower(target)
				dot.Snapshot(target, tickDamage, isRollover)
			},
			OnTick: func(sim *core.Simulation, target *core.Unit, dot *core.Dot) {
				dot.CalcAndDealPeriodicSnapshotDamage(sim, target, dot.OutcomeTick)
			},
		},

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			result := spell.CalcOutcome(sim, target, spell.OutcomeMeleeSpecialHitNoHitCounter)
			if result.Landed() {
				dot := spell.Dot(target)
				dot.Apply(sim)
				druid.SpendComboPoints(sim, spell)
			} else {
				spell.IssueRefund(sim)
			}
			spell.DealOutcome(sim, result)
		},
	}
}

func (druid *Druid) CurrentRipCost() float64 {
	return druid.Rip.Cost.GetCurrentCost()
}
