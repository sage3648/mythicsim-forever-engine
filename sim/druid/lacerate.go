package druid

import (
	"github.com/wowsims/classic/sim/common/shared"
	"github.com/wowsims/classic/sim/core"
)

const LacerateMaxStacks int32 = 5

// Forever trains Lacerate at 42, 50 and 58 (414644, 1235826, 1235827). Beta client 1.60.1.69893: 15 Rage, 10% weapon
// damage per stack on the hit, and a bleed of 10 / 12 / 15 a tick per stack over 15 sec that no longer scales with
// attack power. The client does not carry threat, so the 3.33x is still Season of Discovery's. The cost, tick and tick
// schedule come from the client table (see wrath.go); the ids stay ours (rank 1's at every level, as the APLs name it).
func (druid *Druid) lacerateRow() shared.SpellData {
	rank := int32(1)
	if druid.Level >= 58 {
		rank = 3
	} else if druid.Level >= 50 {
		rank = 2
	}
	return spellData.Lacerate.ByRank(rank)
}

func (druid *Druid) registerLacerateSpell() {
	druid.registerLacerateBleedSpell()
	row := druid.lacerateRow()

	results := make([]*core.SpellResult, min(MangleBerserkTargets, druid.Env.GetNumTargets()))

	druid.Lacerate = druid.RegisterSpell(Bear, core.SpellConfig{
		SpellCode:      SpellCode_DruidLacerate,
		ClassSpellMask: SpellMaskLacerate,
		ActionID:       core.ActionID{SpellID: 414644},
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskMeleeMHSpecial,
		Flags:          core.SpellFlagMeleeMetrics | core.SpellFlagAPL,

		RageCost: core.RageCostOptions{
			Cost:   float64(row.Cost) - float64(druid.Talents.ShreddingAttacks),
			Refund: 0.8,
		},
		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD: core.GCDDefault,
			},
			IgnoreHaste: true,
		},

		DamageMultiplier: 1,
		ThreatMultiplier: 3.33,

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			numHits := 1
			if druid.BerserkAura.IsActive() {
				numHits = len(results)
			}

			for idx := 0; idx < numHits; idx++ {
				stacks := min(druid.LacerateBleed.Dot(target).GetStacks()+1, LacerateMaxStacks)
				baseDamage := spell.Unit.MHWeaponDamage(sim, spell.MeleeAttackPower(target)) * 0.1 * float64(stacks)
				results[idx] = spell.CalcAndDealDamage(sim, target, baseDamage, spell.OutcomeMeleeSpecialHitAndCrit)

				if results[idx].Landed() {
					druid.LacerateBleed.Cast(sim, target)
				}
				target = sim.Environment.NextTargetUnit(target)
			}

			if !results[0].Landed() {
				spell.IssueRefund(sim)
			}
		},
	})
}

func (druid *Druid) registerLacerateBleedSpell() {
	row := druid.lacerateRow()
	periodic := row.Periodic.(shared.SpellDataPeriodic)
	tickDamage := periodic.Tick

	druid.LacerateBleed = druid.RegisterSpell(Bear, core.SpellConfig{
		ClassSpellMask: SpellMaskLacerateBleed,
		ActionID:       core.ActionID{SpellID: 414647},
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskEmpty,
		Flags:          core.SpellFlagMeleeMetrics | core.SpellFlagNoOnCastComplete,

		DamageMultiplier: 1,
		ThreatMultiplier: 3.33,

		Dot: core.DotConfig{
			Aura: core.Aura{
				Label:     "Lacerate",
				MaxStacks: LacerateMaxStacks,
				Duration:  row.Duration,
			},
			NumberOfTicks: periodic.NumberOfTicks,
			TickLength:    periodic.TickLength,

			OnSnapshot: func(sim *core.Simulation, target *core.Unit, dot *core.Dot, isRollover bool) {
				dot.Snapshot(target, tickDamage*float64(dot.Aura.GetStacks()), isRollover)
			},
			OnTick: func(sim *core.Simulation, target *core.Unit, dot *core.Dot) {
				dot.CalcAndDealPeriodicSnapshotDamage(sim, target, dot.OutcomeTick)
			},
		},

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			dot := spell.Dot(target)
			if dot.IsActive() {
				dot.Refresh(sim)
				dot.AddStack(sim)
			} else {
				dot.Apply(sim)
				dot.SetStacks(sim, 1)
			}
			// Snapshot again once the stacks are in, since the damage grows with them.
			dot.TakeSnapshot(sim, false)
		},
	})
}
