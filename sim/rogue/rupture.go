package rogue

import (
	"time"

	"github.com/wowsims/classic/sim/common/shared"
	"github.com/wowsims/classic/sim/core"
)

func (rogue *Rogue) registerRupture() {
	spellID := map[int32]int32{
		25: 1943,
		40: 8640,
		50: 11273,
		60: 11275,
	}[rogue.Level]

	// Cost, school, defense type, tick length, base tick and base tick count come from the client
	// table; the id stays ours (see sinister_strike.go). The per combo point step sits on a dummy
	// effect that reads 0, so it stays ours.
	row := spellData.Rupture.BySpellID(spellID)

	rogue.Rupture = rogue.RegisterSpell(core.SpellConfig{
		SpellCode:      SpellCode_RogueRupture,
		ClassSpellMask: SpellMaskRupture,
		ActionID:       core.ActionID{SpellID: spellID},
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskMeleeMHSpecial,
		Flags:          rogue.finisherFlags(),
		MetricSplits:   6,

		EnergyCost: core.EnergyCostOptions{
			Cost:   float64(row.Cost),
			Refund: 0,
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

		DamageMultiplier: []float64{1, 1.1, 1.2, 1.3}[rogue.Talents.SerratedBlades],
		ThreatMultiplier: 1,

		Dot: core.DotConfig{
			Aura: core.Aura{
				Label: "Rupture",
			},
			NumberOfTicks: 0, // Set dynamically
			TickLength:    row.Periodic.(shared.SpellDataPeriodic).TickLength,

			OnSnapshot: func(sim *core.Simulation, target *core.Unit, dot *core.Dot, isRollover bool) {
				damage := rogue.RuptureDamage(target, rogue.ComboPoints())
				if rogue.isHemorrhaging(target) {
					damage *= HemorrhageRuptureMultiplier
				}
				dot.Snapshot(target, damage, isRollover)
			},
			OnTick: func(sim *core.Simulation, target *core.Unit, dot *core.Dot) {
				dot.CalcAndDealPeriodicSnapshotDamage(sim, target, dot.OutcomeTick)
			},
		},

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			rogue.BreakStealth(sim)
			result := spell.CalcOutcome(sim, target, spell.OutcomeMeleeSpecialHitNoHitCounter)
			if result.Landed() {
				dot := spell.Dot(target)
				dot.Spell = spell
				dot.NumberOfTicks = rogue.RuptureTicks(rogue.ComboPoints())
				dot.Apply(sim)
				rogue.SpendComboPoints(sim, spell)
			} else {
				spell.IssueRefund(sim)
			}
			spell.DealOutcome(sim, result)
		},
	})
	rogue.Finishers = append(rogue.Finishers, rogue.Rupture)
}

func (rogue *Rogue) RuptureDamage(target *core.Unit, comboPoints int32) float64 {
	// Beta client 1.60.1.69893 cut every rank's tick and its per combo point step (rank 6 60 + 8
	// -> 35 + 4.73). The attack power share below is not in the client and stays Classic's.
	baseTickDamage := rogue.rupturePeriodic().Tick

	comboTickDamage := map[int32]float64{
		25: 1.18,
		40: 2.37,
		50: 2.96,
		60: 4.73,
	}[rogue.Level]

	return baseTickDamage + comboTickDamage*float64(comboPoints) +
		[]float64{0, 0.04 / 4, 0.10 / 5, 0.18 / 6, 0.21 / 7, 0.24 / 8}[comboPoints]*rogue.Rupture.MeleeAttackPower(target)
}

func (rogue *Rogue) RuptureTicks(comboPoints int32) int32 {
	return rogue.rupturePeriodic().NumberOfTicks + comboPoints
}

func (rogue *Rogue) RuptureDuration(comboPoints int32) time.Duration {
	return time.Duration(rogue.RuptureTicks(comboPoints)) * rogue.rupturePeriodic().TickLength
}

func (rogue *Rogue) rupturePeriodic() shared.SpellDataPeriodic {
	return spellData.Rupture.BySpellID(rogue.Rupture.ActionID.SpellID).Periodic.(shared.SpellDataPeriodic)
}
