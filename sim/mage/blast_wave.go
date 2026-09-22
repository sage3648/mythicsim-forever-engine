package mage

import (
	"github.com/wowsims/classic/sim/core"
)

const BlastWaveRanks = 5

// Spell ID, cost, cooldown and coefficient come from the client table (see
// frostbolt.go for why the damage does not).
//
// Beta client 1.60.1.69893.
var BlastWaveBaseDamage = [BlastWaveRanks + 1][]float64{{0}, {154, 184}, {200, 239}, {276, 327}, {365, 432}, {453, 533}}
var BlastWaveLevel = [BlastWaveRanks + 1]int{0, 30, 36, 44, 52, 60}

func (mage *Mage) registerBlastWaveSpell() {
	if !mage.Talents.BlastWave {
		return
	}

	mage.BlastWave = make([]*core.Spell, BlastWaveRanks+1)
	cdTimer := mage.NewTimer()

	for rank := 1; rank <= BlastWaveRanks; rank++ {
		config := mage.newBlastWaveSpellConfig(rank, cdTimer)

		if config.RequiredLevel <= int(mage.Level) {
			mage.BlastWave[rank] = mage.GetOrRegisterSpell(config)
		}
	}
}

func (mage *Mage) newBlastWaveSpellConfig(rank int, cooldownTimer *core.Timer) core.SpellConfig {
	row := spellData.BlastWave.ByRank(int32(rank))
	baseDamageLow := BlastWaveBaseDamage[rank][0]
	baseDamageHigh := BlastWaveBaseDamage[rank][1]
	level := BlastWaveLevel[rank]

	return core.SpellConfig{
		SpellCode:      SpellCode_MageBlastWave,
		ClassSpellMask: SpellMaskBlastWave,
		ActionID:       core.ActionID{SpellID: row.SpellID},
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskSpellDamage,
		Flags:          SpellFlagMage | core.SpellFlagBinary | core.SpellFlagAPL,

		RequiredLevel: level,
		Rank:          rank,

		ManaCost: core.ManaCostOptions{
			FlatCost: float64(row.Cost),
		},
		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD: core.GCDDefault,
			},
			CD: core.Cooldown{
				Timer:    cooldownTimer,
				Duration: row.Cooldown,
			},
		},

		DamageMultiplier: 1,
		ThreatMultiplier: 1,
		BonusCoefficient: roundCoef(row.Direct.BonusCoefficient()),

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			for _, aoeTarget := range sim.Encounter.TargetUnits {
				baseDamage := sim.Roll(baseDamageLow, baseDamageHigh)
				spell.CalcAndDealDamage(sim, aoeTarget, baseDamage, spell.OutcomeMagicCrit)
			}
		},
	}
}
