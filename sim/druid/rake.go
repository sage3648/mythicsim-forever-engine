package druid

import (
	"time"

	"github.com/wowsims/classic/sim/common/shared"
	"github.com/wowsims/classic/sim/core"
)

// Beta client 1.60.1.69893: a little more damage at every rank (rank 4 58 plus 32 a tick -> 61 plus 34 a tick). The id,
// cost, hit, tick and tick schedule come from the client table (see wrath.go).
var RakeLevel = []int32{0, 24, 34, 44, 54}

func (druid *Druid) registerRakeSpell() {
	// Add highest available rake rank for level.
	for rank := len(RakeLevel) - 1; rank >= 1; rank-- {
		if druid.Level >= RakeLevel[rank] {
			config := druid.newRakeSpellConfig(rank)
			druid.Rake = druid.RegisterSpell(Cat, config)
			return
		}
	}
}

func (druid *Druid) newRakeSpellConfig(rank int) core.SpellConfig {
	row := spellData.Rake.ByRank(int32(rank))
	periodic := row.Periodic.(shared.SpellDataPeriodic)
	baseDamageInitial, _ := row.Direct.Range()
	baseDamageTick := periodic.Tick
	energyCost := float64(row.Cost) - float64(druid.Talents.Ferocity)

	return core.SpellConfig{
		SpellCode:      SpellCode_DruidRake,
		ClassSpellMask: SpellMaskRake,
		ActionID:       core.ActionID{SpellID: row.SpellID},
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskMeleeMHSpecial,
		Flags:          core.SpellFlagMeleeMetrics | core.SpellFlagIgnoreResists | core.SpellFlagBinary | core.SpellFlagAPL | SpellFlagBuilder,

		EnergyCost: core.EnergyCostOptions{
			Cost:   energyCost,
			Refund: 0.8,
		},
		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD: time.Second,
			},
			IgnoreHaste: true,
		},

		DamageMultiplierAdditive: 1 + 0.05*float64(druid.Talents.SavageFury),
		DamageMultiplier:         1,
		ThreatMultiplier:         1,

		Dot: core.DotConfig{
			Aura: core.Aura{
				Label: "Rake",
			},
			NumberOfTicks: periodic.NumberOfTicks,
			TickLength:    periodic.TickLength,
			OnSnapshot: func(sim *core.Simulation, target *core.Unit, dot *core.Dot, isRollover bool) {
				damage := baseDamageTick
				dot.Snapshot(target, damage, isRollover)
			},
			OnTick: func(sim *core.Simulation, target *core.Unit, dot *core.Dot) {
				dot.CalcAndDealPeriodicSnapshotDamage(sim, target, dot.OutcomeTick)
			},
		},

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			baseDamage := baseDamageInitial
			result := spell.CalcAndDealDamage(sim, target, baseDamage, spell.OutcomeMeleeSpecialHitAndCrit)

			if result.Landed() {
				druid.AddComboPoints(sim, 1, target, spell.ComboPointMetrics())
				spell.Dot(target).Apply(sim)
			} else {
				spell.IssueRefund(sim)
			}
		},

		ExpectedInitialDamage: func(sim *core.Simulation, target *core.Unit, spell *core.Spell, _ bool) *core.SpellResult {
			baseDamage := baseDamageInitial
			initial := spell.CalcPeriodicDamage(sim, target, baseDamage, spell.OutcomeExpectedMagicAlwaysHit)

			attackTable := spell.Unit.AttackTables[target.UnitIndex][spell.CastType]
			critChance := spell.PhysicalCritChance(attackTable)
			critMod := critChance * (spell.CritMultiplier(attackTable) - 1)
			initial.Damage *= 1 + critMod
			return initial
		},
		ExpectedTickDamage: func(sim *core.Simulation, target *core.Unit, spell *core.Spell, _ bool) *core.SpellResult {
			tickBase := baseDamageTick
			ticks := spell.CalcPeriodicDamage(sim, target, tickBase, spell.OutcomeExpectedMagicAlwaysHit)
			return ticks
		},
	}
}

func (druid *Druid) CurrentRakeCost() float64 {
	return druid.Rake.Cost.GetCurrentCost()
}
