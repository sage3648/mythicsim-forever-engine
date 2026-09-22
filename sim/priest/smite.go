package priest

import (
	"time"

	"github.com/wowsims/classic/sim/core"
)

const SmiteRanks = 8

// Forever beta client 1.60.1.69893: ranks 3 and up hit for less, and there is no downranking penalty.
// Everything but the damage comes from the client table (see shadow_word_pain.go).
var SmiteBaseDamage = [SmiteRanks + 1][]float64{{0}, {15, 20}, {28, 34}, {47, 53}, {60, 68}, {81, 91}, {93, 106}, {124, 139}, {166, 187}}
var SmiteLevel = [SmiteRanks + 1]int{0, 1, 6, 14, 22, 30, 38, 46, 54}

func (priest *Priest) registerSmiteSpell() {
	priest.Smite = make([]*core.Spell, SmiteRanks+1)

	for rank := 1; rank <= SmiteRanks; rank++ {
		config := priest.getSmiteBaseConfig(rank)

		if config.RequiredLevel <= int(priest.Level) {
			priest.Smite[rank] = priest.GetOrRegisterSpell(config)
		}
	}
}

func (priest *Priest) getSmiteBaseConfig(rank int) core.SpellConfig {
	row := spellData.Smite.ByRank(int32(rank))
	baseDamageLow := SmiteBaseDamage[rank][0]
	baseDamageHigh := SmiteBaseDamage[rank][1]
	level := SmiteLevel[rank]

	return core.SpellConfig{
		ActionID:       core.ActionID{SpellID: row.SpellID},
		SpellCode:      SpellCode_PriestSmite,
		ClassSpellMask: SpellMaskSmite,
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskSpellDamage,
		Flags:          SpellFlagPriest | core.SpellFlagAPL,

		RequiredLevel: level,
		Rank:          rank,

		ManaCost: core.ManaCostOptions{
			FlatCost: float64(row.Cost),
		},
		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD:      core.GCDDefault,
				CastTime: row.CastTime - time.Millisecond*100*time.Duration(priest.Talents.DivineFury),
			},
		},

		BonusCoefficient: roundCoef(row.Direct.BonusCoefficient()),

		DamageMultiplier: 1,
		ThreatMultiplier: 1,

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			baseDamage := sim.Roll(baseDamageLow, baseDamageHigh)
			spell.CalcAndDealDamage(sim, target, baseDamage, spell.OutcomeMagicHitAndCrit)
		},
	}
}
