package warlock

import (
	"github.com/wowsims/classic/sim/core"
)

const ShadowburnRanks = 6

// Beta client 1.60.1 values for every rank. The BlizzCon tooltip's 102 to 111 for rank 1 is not
// what the client carries. Everything but the damage comes from the client table (see shadowbolt.go).
var ShadowburnBaseDamage = [ShadowburnRanks + 1][]float64{{0}, {65, 74}, {81, 91}, {119, 133}, {147, 164}, {201, 224}, {259, 288}}

func (warlock *Warlock) registerShadowBurnBaseConfig(rank int) core.SpellConfig {
	row := spellData.Shadowburn.ByRank(int32(rank))
	baseDamage := ShadowburnBaseDamage[rank]
	level := [ShadowburnRanks + 1]int{0, 15, 24, 32, 40, 48, 56}[rank]

	return core.SpellConfig{
		ActionID:       core.ActionID{SpellID: row.SpellID},
		SpellCode:      SpellCode_WarlockShadowburn,
		ClassSpellMask: SpellMaskShadowburn,
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskSpellDamage,
		Flags:          core.SpellFlagAPL | core.SpellFlagResetAttackSwing | core.SpellFlagBinary | WarlockFlagDestruction,
		RequiredLevel:  level,
		Rank:           rank,

		ManaCost: core.ManaCostOptions{
			FlatCost: float64(row.Cost),
		},
		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD: core.GCDDefault,
			},
			CD: core.Cooldown{
				Timer:    warlock.NewTimer(),
				Duration: row.Cooldown,
			},
		},

		DamageMultiplier: 1,
		ThreatMultiplier: 1,
		BonusCoefficient: roundCoef(row.Direct.BonusCoefficient()),

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			baseDamage := sim.Roll(baseDamage[0], baseDamage[1])
			spell.CalcAndDealDamage(sim, target, baseDamage, spell.OutcomeMagicHitAndCrit)
		},
	}
}

func (warlock *Warlock) registerShadowBurnSpell() {
	if !warlock.Talents.Shadowburn {
		return
	}

	warlock.Shadowburn = make([]*core.Spell, 0)
	for rank := 1; rank <= ShadowburnRanks; rank++ {
		config := warlock.registerShadowBurnBaseConfig(rank)

		if config.RequiredLevel <= int(warlock.Level) {
			warlock.Shadowburn = append(warlock.Shadowburn, warlock.GetOrRegisterSpell(config))
		}
	}
}
