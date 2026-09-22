package mage

import (
	"fmt"
	"time"

	"github.com/wowsims/classic/sim/core"
)

const ArcaneMissilesRanks = 8

// Beta client 1.60.1.69893, read off the missile each rank triggers (7268 ... 25346): less damage per
// missile, but every rank scales at .286 per missile against Classic's .24.
//
// Spell ID, cost, missile count (the channel's duration), and the missile's coefficient, speed and
// school come from the client table. The per-missile damage does not: the table truncates the
// level-scaled value, reading one below ours on every rank but 3 and 8.
var ArcaneMissilesBaseTickDamage = [ArcaneMissilesRanks + 1]float64{0, 26, 33, 46, 69, 98, 134, 175, 209}
var ArcaneMissilesLevel = [ArcaneMissilesRanks + 1]int{0, 8, 16, 24, 32, 40, 48, 56, 56}

func (mage *Mage) registerArcaneMissilesSpell() {
	mage.ArcaneMissiles = make([]*core.Spell, ArcaneMissilesRanks+1)
	mage.ArcaneMissilesTickSpell = make([]*core.Spell, ArcaneMissilesRanks+1)

	maxRank := core.TernaryInt(core.IncludeAQ, ArcaneMissilesRanks, ArcaneMissilesRanks-1)
	for rank := 1; rank <= maxRank; rank++ {
		config := mage.getArcaneMissilesSpellConfig(rank)

		if config.RequiredLevel <= int(mage.Level) {
			mage.ArcaneMissiles[rank] = mage.GetOrRegisterSpell(config)
		}
	}
}

func (mage *Mage) getArcaneMissilesSpellConfig(rank int) core.SpellConfig {
	row := spellData.ArcaneMissiles.ByRank(int32(rank))
	baseTickDamage := ArcaneMissilesBaseTickDamage[rank]
	level := ArcaneMissilesLevel[rank]

	tickLength := time.Second
	numTicks := int32(row.Duration / tickLength)
	// Missile Barrage keeps the missile count and fires them every 0.5 sec instead, which is the
	// same thing as halving the channel.
	barrageTickLength := time.Millisecond * 500

	tickSpell := mage.getArcaneMissilesTickSpell(rank)
	mage.ArcaneMissilesTickSpell[rank] = tickSpell

	return core.SpellConfig{
		SpellCode:      SpellCode_MageArcaneMissiles,
		ClassSpellMask: SpellMaskArcaneMissiles,
		ActionID:       core.ActionID{SpellID: row.SpellID},
		SpellSchool:    row.SpellSchool,
		// The channel's row reads no defense type; its missiles are magic.
		DefenseType: core.DefenseTypeMagic,
		ProcMask:    core.ProcMaskSpellDamage,
		Flags:       SpellFlagMage | core.SpellFlagAPL | core.SpellFlagChanneled | core.SpellFlagNoMetrics,

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
			Aura: core.Aura{
				Label: fmt.Sprintf("ArcaneMissiles-%d-%d", +rank, numTicks),
				OnExpire: func(aura *core.Aura, sim *core.Simulation) {
					// TODO: This check is necessary to ensure the final tick occurs before
					// Arcane Blast stacks are dropped. To fix this, ticks need to reliably
					// occur before aura expirations.

					//TODO: Test interaction in classic code without aura
					dot := mage.ArcaneMissiles[rank].Dot(aura.Unit)
					if dot.TickCount < dot.NumberOfTicks {
						dot.TickCount++
						dot.TickOnce(sim)
					}

					mage.spendArcaneBlastStacks(sim)
				},
			},
			NumberOfTicks: numTicks,
			TickLength:    tickLength,
			OnTick: func(sim *core.Simulation, target *core.Unit, dot *core.Dot) {
				tickSpell.Cast(sim, target)
			},
		},

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			dot := spell.Dot(target)
			dot.TickLength = core.TernaryDuration(mage.MissileBarrageAura.IsActive(), barrageTickLength, tickLength)
			dot.RecomputeAuraDuration()
			dot.Apply(sim)
		},
		ExpectedTickDamage: func(sim *core.Simulation, target *core.Unit, spell *core.Spell, _ bool) *core.SpellResult {
			return tickSpell.CalcDamage(sim, target, baseTickDamage, spell.OutcomeExpectedMagicHitAndCrit)
		},
	}
}

func (mage *Mage) getArcaneMissilesTickSpell(rank int) *core.Spell {
	channel := spellData.ArcaneMissiles.ByRank(int32(rank))
	row := spellData.ArcaneMissilesTriggered.ByRank(int32(rank))
	baseTickDamage := ArcaneMissilesBaseTickDamage[rank]

	return mage.RegisterSpell(core.SpellConfig{
		SpellCode:      SpellCode_MageArcaneMissilesTick,
		ClassSpellMask: SpellMaskArcaneMissilesTick,
		// Filed under the channel's id, not the missile's own (7268 ...), as the metrics always were.
		ActionID:     core.ActionID{SpellID: channel.SpellID}.WithTag(1),
		SpellSchool:  row.SpellSchool,
		DefenseType:  row.DefenseType,
		ProcMask:     core.ProcMaskSpellDamage,
		Flags:        SpellFlagMage,
		MissileSpeed: row.MissileSpeed,

		Rank: 1,

		DamageMultiplier: 1,
		ThreatMultiplier: 1,
		BonusCoefficient: roundCoef(row.Direct.BonusCoefficient()),

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			result := spell.CalcDamage(sim, target, baseTickDamage, spell.OutcomeMagicHitAndCrit)

			spell.WaitTravelTime(sim, func(sim *core.Simulation) {
				spell.DealDamage(sim, result)
			})
		},
	})
}
