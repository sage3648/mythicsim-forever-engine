package priest

import (
	"fmt"

	"github.com/wowsims/classic/sim/common/shared"
	"github.com/wowsims/classic/sim/core"
	"github.com/wowsims/classic/sim/core/proto"
)

const StarshardsRanks = 7
const StarshardsTicks = 6

var StarshardsTickSpellId = [StarshardsRanks + 1]int32{0, 19350, 19351, 19352, 19353, 19354, 19355, 19356}

// Forever beta client 1.60.1.69893, about double Classic's, at .167 a tick for every rank. Spell ID,
// cost, school, tick, tick count and coefficient come from the client table. Its 30 sec cooldown is not
// used: ours has never had one.
var StarshardsLevel = [StarshardsRanks + 1]int{0, 10, 18, 26, 34, 42, 50, 58}

func (priest *Priest) registerStarshardsSpell() {
	if priest.Race != proto.Race_RaceNightElf {
		return
	}

	priest.Starshards = make([][]*core.Spell, StarshardsRanks+1)

	for rank := 1; rank <= StarshardsRanks; rank++ {
		priest.Starshards[rank] = make([]*core.Spell, StarshardsTicks+1)

		var tick int32
		for tick = 0; tick < StarshardsTicks; tick++ {
			config := priest.newStarshardsSpellConfig(rank, tick)

			if config.RequiredLevel <= int(priest.Level) {
				priest.Starshards[rank][tick] = priest.RegisterSpell(config)
			}
		}
	}
}

func (priest *Priest) newStarshardsSpellConfig(rank int, tickIdx int32) core.SpellConfig {
	row := spellData.Starshards.ByRank(int32(rank))
	periodic := row.Periodic.(shared.SpellDataPeriodic)
	ticks := tickIdx
	flags := SpellFlagPriest | core.SpellFlagChanneled | core.SpellFlagBinary
	if tickIdx == 0 {
		ticks = periodic.NumberOfTicks
		flags |= core.SpellFlagAPL
	}

	baseDamage := periodic.Tick
	level := StarshardsLevel[rank]

	return core.SpellConfig{
		SpellCode:      SpellCode_PriestStarshards,
		ClassSpellMask: SpellMaskStarshards,
		ActionID:       core.ActionID{SpellID: row.SpellID}.WithTag(tickIdx),
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskSpellDamage,
		Flags:          flags,

		RequiredLevel: level,
		Rank:          rank,

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
				Label: fmt.Sprintf("Starshards-%d-%d", rank, tickIdx),
			},
			NumberOfTicks:       ticks,
			TickLength:          periodic.TickLength,
			AffectedByCastSpeed: false,
			BonusCoefficient:    roundCoef(periodic.Coef),
			OnSnapshot: func(sim *core.Simulation, target *core.Unit, dot *core.Dot, isRollover bool) {
				dot.Snapshot(target, baseDamage, isRollover)
			},
			OnTick: func(sim *core.Simulation, target *core.Unit, dot *core.Dot) {
				dot.CalcAndDealPeriodicSnapshotDamage(sim, target, dot.OutcomeTick)
			},
		},

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			result := spell.CalcOutcome(sim, target, spell.OutcomeMagicHit)
			if result.Landed() {
				spell.Dot(target).Apply(sim)
			}
			spell.DealOutcome(sim, result)
		},

		ExpectedTickDamage: func(sim *core.Simulation, target *core.Unit, spell *core.Spell, _ bool) *core.SpellResult {
			result := spell.CalcPeriodicDamage(sim, target, baseDamage, spell.OutcomeExpectedMagicAlwaysHit)
			return result
		},
	}
}
