package mage

import (
	"time"

	"github.com/wowsims/classic/sim/core"
)

const FireBlastRanks = 7

// Spell ID, cost, cooldown and coefficient come from the client table (see
// frostbolt.go for why the damage does not).
//
// Beta client 1.60.1.69893.
var FireBlastBaseDamage = [FireBlastRanks + 1][]float64{{0}, {27, 35}, {57, 69}, {97, 117}, {157, 187}, {226, 268}, {316, 372}, {417, 489}}
var FireBlastLevel = [FireBlastRanks + 1]int{0, 6, 14, 22, 30, 38, 46, 54}

func (mage *Mage) registerFireBlastSpell() {
	mage.FireBlast = make([]*core.Spell, FireBlastRanks+1)
	cdTimer := mage.NewTimer()

	for rank := 1; rank <= FireBlastRanks; rank++ {
		config := mage.newFireBlastSpellConfig(rank, cdTimer)

		if config.RequiredLevel <= int(mage.Level) {
			mage.FireBlast[rank] = mage.GetOrRegisterSpell(config)
		}
	}
}

func (mage *Mage) newFireBlastSpellConfig(rank int, cdTimer *core.Timer) core.SpellConfig {

	row := spellData.FireBlast.ByRank(int32(rank))
	baseDamageLow := FireBlastBaseDamage[rank][0]
	baseDamageHigh := FireBlastBaseDamage[rank][1]
	level := FireBlastLevel[rank]

	// Wake of Fire takes 1 and 2 sec off in the beta client.
	cooldown := row.Cooldown - []time.Duration{0, time.Second, time.Second * 2}[mage.Talents.WakeOfFire]
	flags := SpellFlagMage | core.SpellFlagAPL

	return core.SpellConfig{
		ActionID:       core.ActionID{SpellID: row.SpellID},
		SpellCode:      SpellCode_MageFireBlast,
		ClassSpellMask: SpellMaskFireBlast,
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskSpellDamage,
		Flags:          flags,

		Rank:          rank,
		RequiredLevel: level,

		ManaCost: core.ManaCostOptions{
			FlatCost: float64(row.Cost),
		},
		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD: core.GCDDefault,
			},
			CD: core.Cooldown{
				Timer:    cdTimer,
				Duration: cooldown,
			},
		},

		DamageMultiplier: 1,
		ThreatMultiplier: 1,
		BonusCoefficient: roundCoef(row.Direct.BonusCoefficient()),

		ExpectedInitialDamage: func(sim *core.Simulation, target *core.Unit, spell *core.Spell, _ bool) *core.SpellResult {
			baseDamageCalc := (baseDamageLow + baseDamageHigh) / 2
			return spell.CalcDamage(sim, target, baseDamageCalc, spell.OutcomeExpectedMagicHitAndCrit)
		},
		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			baseDamage := sim.Roll(baseDamageLow, baseDamageHigh)
			spell.CalcAndDealDamage(sim, target, baseDamage, spell.OutcomeMagicHitAndCrit)
		},
	}
}
