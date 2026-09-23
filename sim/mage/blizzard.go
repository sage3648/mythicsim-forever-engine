package mage

import (
	"fmt"
	"time"

	"github.com/wowsims/classic/sim/common/shared"
	"github.com/wowsims/classic/sim/core"
)

const BlizzardRanks = 6

// Beta client 1.60.1.69893. Forever's Blizzard is an area trigger that casts a damage spell every
// second (1279976 ... 1279949), 8 times; the tick below is that spell's damage. The coefficient below
// is the damage spell's, per tick; the parent spell's dummy effect carries 0.03, which is not used.
//
// Spell ID, cost, ticks and the coefficient come from the client table. The tick does not: the table
// holds each rank at level 60, but the tick grows with the caster's level up to the tick spell's max
// level (base, per level, spell level, max level from 1279976 ... 1279949), so level 40 casts rank 3
// for 62 (table 63) and level 50 rank 4 for 88 (ours was the unscaled 87).
var blizzardTicks = [BlizzardRanks + 1]struct {
	base, perLevel       float64
	spellLevel, maxLevel int32
}{{}, {24, 0.1, 20, 25}, {42, 0.2, 28, 33}, {62, 0.2, 36, 41}, {87, 0.3, 44, 49}, {114, 0.3, 52, 57}, {146, 0.4, 60, 65}}
var BlizzardLevel = [BlizzardRanks + 1]int{0, 20, 28, 36, 44, 52, 60}

func (mage *Mage) registerBlizzardSpell() {
	mage.Blizzard = make([]*core.Spell, BlizzardRanks+1)

	for rank := 1; rank <= BlizzardRanks; rank++ {
		config := mage.newBlizzardSpellConfig(rank)

		if config.RequiredLevel <= int(mage.Level) {
			mage.Blizzard[rank] = mage.GetOrRegisterSpell(config)
		}
	}
}

func (mage *Mage) newBlizzardSpellConfig(rank int) core.SpellConfig {
	row := spellData.Blizzard.ByRank(int32(rank))
	periodic := row.Periodic.(shared.SpellDataPeriodic)
	tick := blizzardTicks[rank]
	baseDamage := shared.LevelScaled(tick.base, tick.perLevel, tick.spellLevel, tick.maxLevel, mage.Level)
	level := BlizzardLevel[rank]

	var improvedBlizzardProcApplication *core.Spell
	if mage.Talents.ImprovedBlizzard > 0 {
		impId := []int32{0, 11185, 12487, 12488}[mage.Talents.ImprovedBlizzard]
		auras := mage.NewEnemyAuraArray(func(unit *core.Unit) *core.Aura {
			return unit.GetOrRegisterAura(core.Aura{
				ActionID: core.ActionID{SpellID: impId},
				Label:    "Improved Blizzard",
				Duration: time.Millisecond * 1500,
			})
		})
		improvedBlizzardProcApplication = mage.RegisterSpell(core.SpellConfig{
			ActionID: core.ActionID{SpellID: impId},
			ProcMask: core.ProcMaskSpellProc,
			Flags:    SpellFlagMage | core.SpellFlagNoLogs | SpellFlagChillSpell,
			ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
				auras.Get(target).Activate(sim)
			},
		})
	}

	return core.SpellConfig{
		ActionID:    core.ActionID{SpellID: row.SpellID},
		SpellSchool: row.SpellSchool,
		DefenseType: row.DefenseType,
		ProcMask:    core.ProcMaskSpellDamage,
		Flags:       SpellFlagMage | core.SpellFlagChanneled | core.SpellFlagAPL,

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

		Dot: core.DotConfig{
			IsAOE: true,
			Aura: core.Aura{
				Label: fmt.Sprintf("Blizzard (Rank %d)", rank),
			},
			NumberOfTicks:    periodic.NumberOfTicks,
			TickLength:       periodic.TickLength,
			BonusCoefficient: roundCoef(periodic.Coef),
			OnSnapshot: func(sim *core.Simulation, target *core.Unit, dot *core.Dot, isRollover bool) {
				dot.Snapshot(target, baseDamage, isRollover)
			},
			OnTick: func(sim *core.Simulation, target *core.Unit, dot *core.Dot) {
				for _, aoeTarget := range sim.Encounter.TargetUnits {
					dot.CalcAndDealPeriodicSnapshotDamage(sim, aoeTarget, dot.OutcomeTick)

					if improvedBlizzardProcApplication != nil {
						improvedBlizzardProcApplication.Cast(sim, aoeTarget)
					}
				}
			},
		},

		DamageMultiplier: 1,
		ThreatMultiplier: 1,

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			spell.AOEDot().Apply(sim)
		},
	}
}
