package priest

import (
	"time"

	"github.com/wowsims/classic/sim/core"
)

const MindBlastRanks = 9

// Forever beta client 1.60.1.69893: lower at every rank, and ranks 1 and 2 lose their downranking penalty.
// Everything but the damage comes from the client table (see shadow_word_pain.go).
var MindBlastBaseDamage = [MindBlastRanks + 1][]float64{{0}, {40, 44}, {69, 76}, {103, 110}, {154, 162}, {198, 210}, {259, 276}, {325, 344}, {406, 428}, {477, 504}}
var MindBlastLevel = [MindBlastRanks + 1]int{0, 10, 16, 22, 28, 34, 40, 46, 52, 58}

func (priest *Priest) registerMindBlast() {
	priest.MindBlast = make([]*core.Spell, MindBlastRanks+1)
	cdTimer := priest.NewTimer()

	for rank := 1; rank <= MindBlastRanks; rank++ {
		config := priest.getMindBlastBaseConfig(rank, cdTimer)

		if config.RequiredLevel <= int(priest.Level) {
			priest.MindBlast[rank] = priest.GetOrRegisterSpell(config)
		}
	}
}

func (priest *Priest) getMindBlastBaseConfig(rank int, cdTimer *core.Timer) core.SpellConfig {
	row := spellData.MindBlast.ByRank(int32(rank))
	baseDamageLow := MindBlastBaseDamage[rank][0]
	baseDamageHigh := MindBlastBaseDamage[rank][1]
	level := MindBlastLevel[rank]

	return core.SpellConfig{
		SpellCode:      SpellCode_PriestMindBlast,
		ClassSpellMask: SpellMaskMindBlast,
		ActionID:       core.ActionID{SpellID: row.SpellID},
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
				CastTime: row.CastTime,
			},
			CD: core.Cooldown{
				Timer:    cdTimer,
				Duration: row.Cooldown - time.Millisecond*500*time.Duration(priest.Talents.ImprovedMindBlast),
			},
		},

		DamageMultiplier: 1,
		ThreatMultiplier: 1,
		BonusCoefficient: roundCoef(row.Direct.BonusCoefficient()),

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			result := spell.CalcDamage(sim, target, sim.Roll(baseDamageLow, baseDamageHigh), spell.OutcomeMagicHitAndCrit)

			if result.Landed() {
				priest.AddShadowWeavingStack(sim)
			}
			spell.DealDamage(sim, result)
		},

		ExpectedInitialDamage: func(sim *core.Simulation, target *core.Unit, spell *core.Spell, _ bool) *core.SpellResult {
			damage := (baseDamageLow + baseDamageHigh) / 2
			result := spell.CalcDamage(sim, target, damage, spell.OutcomeExpectedMagicHitAndCrit)
			return result
		},
	}
}
