package shaman

import (
	"github.com/wowsims/classic/sim/core"
)

// The talent teaches rank 1 and the spellbook adds ranks 2 and 3 at 50 and 60. Everything here is the beta
// client's: a 2.5 sec cast, a 10 sec cooldown, flat mana costs, the full 0.714 coefficient, a speed 20 missile
// and 20% more damage on a target carrying the shaman's Flame Shock. All but the Flame Shock bonus and the
// damage (scaled to level 60 like Lightning Bolt's) come from the client table.
const LavaBurstRanks = 3
const LavaBurstFlameShockBonus = .2

var LavaBurstBaseDamage = [LavaBurstRanks + 1][]float64{{0}, {105, 135}, {165, 211}, {192, 248}}
var LavaBurstLevel = [LavaBurstRanks + 1]int{0, 40, 50, 60}

func (shaman *Shaman) registerLavaBurstSpell() {
	shaman.LavaBurst = make([]*core.Spell, LavaBurstRanks+1)

	if !shaman.Talents.LavaBurst {
		return
	}

	cdTimer := shaman.NewTimer()

	for rank := 1; rank <= LavaBurstRanks; rank++ {
		if LavaBurstLevel[rank] <= int(shaman.Level) {
			shaman.LavaBurst[rank] = shaman.RegisterSpell(shaman.newLavaBurstSpellConfig(rank, cdTimer))
		}
	}
}

func (shaman *Shaman) newLavaBurstSpellConfig(rank int, cdTimer *core.Timer) core.SpellConfig {
	row := spellData.LavaBurst.ByRank(int32(rank))
	baseDamageLow := LavaBurstBaseDamage[rank][0]
	baseDamageHigh := LavaBurstBaseDamage[rank][1]
	level := LavaBurstLevel[rank]

	return core.SpellConfig{
		SpellCode:      SpellCode_ShamanLavaBurst,
		ClassSpellMask: SpellMaskLavaBurst,
		ActionID:       core.ActionID{SpellID: row.SpellID},
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskSpellDamage,
		Flags:          SpellFlagShaman | core.SpellFlagAPL,

		RequiredLevel: level,
		Rank:          rank,

		MissileSpeed: row.MissileSpeed,

		ManaCost: core.ManaCostOptions{
			FlatCost:   float64(row.Cost),
			Multiplier: 100 - 2*shaman.Talents.Convection,
		},
		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				CastTime: row.CastTime - shaman.elementalAlacrityReduction(),
				GCD:      core.GCDDefault,
			},
			CD: core.Cooldown{
				Timer:    cdTimer,
				Duration: row.Cooldown,
			},
			ModifyCast: func(sim *core.Simulation, spell *core.Spell, cast *core.Cast) {
				castTime := shaman.ApplyCastSpeedForSpell(cast.CastTime, spell)
				shaman.AutoAttacks.StopMeleeUntil(sim, sim.CurrentTime+castTime, false)
			},
		},

		DamageMultiplier: shaman.callOfFlameMultiplier(),
		ThreatMultiplier: 1,
		BonusCoefficient: roundCoef(row.Direct.BonusCoefficient()),

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			if shaman.hasActiveFlameShock(target) {
				spell.DamageMultiplier *= 1 + LavaBurstFlameShockBonus
				defer func() { spell.DamageMultiplier /= 1 + LavaBurstFlameShockBonus }()
			}

			baseDamage := sim.Roll(baseDamageLow, baseDamageHigh)
			result := spell.CalcDamage(sim, target, baseDamage, spell.OutcomeMagicHitAndCrit)

			spell.WaitTravelTime(sim, func(sim *core.Simulation) {
				spell.DealDamage(sim, result)
			})
		},
	}
}

func (shaman *Shaman) hasActiveFlameShock(target *core.Unit) bool {
	for _, spell := range shaman.FlameShock {
		if spell != nil && spell.Dot(target).IsActive() {
			return true
		}
	}
	return false
}
