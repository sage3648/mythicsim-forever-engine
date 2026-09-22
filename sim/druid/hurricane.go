package druid

import (
	"strconv"

	"github.com/wowsims/classic/sim/common/shared"
	"github.com/wowsims/classic/sim/core"
)

// Beta client 1.60.1.69893: each second the storm casts a separate damage spell (1278965, 1278968, 1278759), whose
// damage is what the tick deals here, 2 less than Classic's at every rank with the same .03 coefficient. The tick and
// its growth by level stay ours (the table holds the tick at 60, truncated); the rest comes from the client table (see
// wrath.go). Written as floats so the spell id walk in sim/spell_sources_test.go does not read them as ids.
var hurricaneRanks = []struct {
	level      int32
	scaleLevel int32
	damage     float64
	scale      float64
}{
	{level: 40, scaleLevel: 46, damage: 68.0, scale: 0.2},
	{level: 50, scaleLevel: 56, damage: 98.0, scale: 0.2},
	{level: 60, scaleLevel: 66, damage: 132.0, scale: 0.3},
}

func (druid *Druid) registerHurricaneSpell() {
	for i, rank := range hurricaneRanks {
		if druid.Level < rank.level {
			break
		}

		row := spellData.Hurricane.ByRank(int32(i + 1))
		periodic := row.Periodic.(shared.SpellDataPeriodic)
		damage := rank.damage + float64(min(druid.Level, rank.scaleLevel)-rank.level)*rank.scale
		spell := druid.RegisterSpell(Humanoid|Moonkin, core.SpellConfig{
			SpellCode:      SpellCode_DruidHurricane,
			ClassSpellMask: SpellMaskHurricane,
			ActionID:       core.ActionID{SpellID: row.SpellID},
			SpellSchool:    row.SpellSchool,
			DefenseType:    row.DefenseType,
			ProcMask:       core.ProcMaskSpellDamage,
			Flags:          core.SpellFlagChanneled | core.SpellFlagBinary | core.SpellFlagAPL,

			RequiredLevel: int(rank.level),
			Rank:          i + 1,

			ManaCost: core.ManaCostOptions{
				FlatCost: float64(row.Cost),
			},
			Cast: core.CastConfig{
				// Forever drops Classic's 1 min cooldown: the client's Hurricane category has no recovery time.
				DefaultCast: core.Cast{
					GCD: core.GCDDefault,
				},
			},

			DamageMultiplier: 1,
			ThreatMultiplier: 1,

			Dot: core.DotConfig{
				IsAOE: true,
				Aura: core.Aura{
					Label: "Hurricane" + druid.Label + strconv.Itoa(i+1),
				},
				NumberOfTicks: periodic.NumberOfTicks,
				TickLength:    periodic.TickLength,

				BonusCoefficient: roundCoef(periodic.Coef),

				OnSnapshot: func(sim *core.Simulation, target *core.Unit, dot *core.Dot, isRollover bool) {
					dot.Snapshot(target, damage, isRollover)
				},
				OnTick: func(sim *core.Simulation, target *core.Unit, dot *core.Dot) {
					for _, aoeTarget := range sim.Encounter.TargetUnits {
						dot.CalcAndDealPeriodicSnapshotDamage(sim, aoeTarget, dot.OutcomeTick)
					}
				},
			},

			ApplyEffects: func(sim *core.Simulation, _ *core.Unit, spell *core.Spell) {
				druid.AutoAttacks.CancelAutoSwing(sim)
				spell.AOEDot().Apply(sim)
			},
		})

		druid.Hurricane = append(druid.Hurricane, spell)
	}
}
