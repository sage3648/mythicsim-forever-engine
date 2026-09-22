package druid

import (
	"fmt"

	"github.com/wowsims/classic/sim/common/shared"
	"github.com/wowsims/classic/sim/core"
)

const InsectSwarmRanks = 5

// Beta client 1.60.1.69893: 6 ticks of 2 sec (rank 5 54 -> 31 a tick). The .158 per tick coefficient and the costs
// are Classic's. All of it comes from the client table (see wrath.go).
var InsectSwarmLevel = [InsectSwarmRanks + 1]int{0, 20, 30, 40, 50, 60}

func (druid *Druid) registerInsectSwarmSpell() {
	if !druid.Talents.InsectSwarm {
		return
	}

	druid.InsectSwarm = make([]*DruidSpell, InsectSwarmRanks+1)

	druid.InsectSwarmAuras = druid.NewEnemyAuraArray(core.InsectSwarmAura)

	for rank := 1; rank <= InsectSwarmRanks; rank++ {
		level := InsectSwarmLevel[rank]
		if int32(level) <= druid.Level {
			row := spellData.InsectSwarm.ByRank(int32(rank))
			periodic := row.Periodic.(shared.SpellDataPeriodic)

			druid.InsectSwarm[rank] = druid.RegisterSpell(Humanoid|Moonkin, core.SpellConfig{
				SpellCode:      SpellCode_DruidInsectSwarm,
				ClassSpellMask: SpellMaskInsectSwarm,
				ActionID:       core.ActionID{SpellID: row.SpellID},
				SpellSchool:    row.SpellSchool,
				DefenseType:    row.DefenseType,
				ProcMask:       core.ProcMaskSpellDamage,
				Flags:          core.SpellFlagAPL | core.SpellFlagBinary,

				ManaCost: core.ManaCostOptions{
					FlatCost: float64(row.Cost),
				},
				Cast: core.CastConfig{
					DefaultCast: core.Cast{
						GCD: core.GCDDefault,
					},
				},

				DamageMultiplier: 1,
				ThreatMultiplier: 1,

				Dot: core.DotConfig{
					Aura: core.Aura{
						Label: fmt.Sprintf("Insect Swarm (Rank %d)", rank),
						OnGain: func(aura *core.Aura, sim *core.Simulation) {
							druid.InsectSwarmAuras.Get(aura.Unit).Activate(sim)
						},
						OnExpire: func(aura *core.Aura, sim *core.Simulation) {
							insectSwarmAura := druid.InsectSwarmAuras.Get(aura.Unit)
							if !insectSwarmAura.IsPermanent() {
								insectSwarmAura.Deactivate(sim)
							}
						},
					},

					NumberOfTicks:    periodic.NumberOfTicks,
					TickLength:       periodic.TickLength,
					BonusCoefficient: roundCoef(periodic.Coef),

					OnSnapshot: func(sim *core.Simulation, target *core.Unit, dot *core.Dot, isRollover bool) {
						dot.Snapshot(target, periodic.Tick, isRollover)
					},
					OnTick: func(sim *core.Simulation, target *core.Unit, dot *core.Dot) {
						dot.CalcAndDealPeriodicSnapshotDamage(sim, target, dot.OutcomeTick)
					},
				},

				ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
					result := spell.CalcOutcome(sim, target, spell.OutcomeMagicHitNoHitCounter)
					if result.Landed() {
						spell.Dot(target).Apply(sim)
					}
					spell.DealOutcome(sim, result)
				},

				RelatedAuras: []core.AuraArray{druid.InsectSwarmAuras},
			})
		}
	}
}
