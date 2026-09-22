package priest

import (
	"time"

	"github.com/wowsims/classic/sim/core"
)

const PenanceTicks = 3

func (priest *Priest) registerPenanceSpell() {
	if !priest.Talents.Penance {
		return
	}

	// Rank 4 (1316995), the level 60 rank in the Forever beta client 1.60.1.69893: 355 mana, a 12 sec
	// cooldown and 131 Holy damage a bolt (1316993) at .285. Ranks 1-3 learn at 30, 40 and 50. The
	// client's rank 3 bolt (180) is larger than rank 4's; the numbers are taken as they are.
	// Cost, cooldown, bolt damage and coefficient come from the client table (see shadow_word_pain.go).
	// The id stays spelled out: ranks 1-3 are never registered, and spell_sources_test.go would file
	// them as ours. The row names no defense type; ours stays magic.
	row := spellData.Penance.ByRank(4)
	baseDamage, _ := row.Direct.Range()
	spellCoeff := roundCoef(row.Direct.BonusCoefficient())

	priest.Penance = priest.RegisterSpell(core.SpellConfig{
		SpellCode:      SpellCode_PriestPenance,
		ClassSpellMask: SpellMaskPenance,
		ActionID:       core.ActionID{SpellID: 1316995},
		SpellSchool:    row.SpellSchool,
		DefenseType:    core.DefenseTypeMagic,
		ProcMask:       core.ProcMaskSpellDamage,
		Flags:          SpellFlagPriest | core.SpellFlagAPL | core.SpellFlagChanneled,

		RequiredLevel: 60,
		Rank:          1,

		ManaCost: core.ManaCostOptions{
			FlatCost: float64(row.Cost),
		},

		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD: core.GCDDefault,
			},
			CD: core.Cooldown{
				Timer:    priest.NewTimer(),
				Duration: row.Cooldown,
			},
		},

		DamageMultiplier: 1,
		ThreatMultiplier: 1,

		Dot: core.DotConfig{
			Aura: core.Aura{
				Label: "Penance",
			},

			// The bolts land at 0/1/2 sec in game, the sim spreads them evenly over the channel.
			NumberOfTicks:    PenanceTicks,
			TickLength:       time.Second * 2 / PenanceTicks,
			BonusCoefficient: spellCoeff,

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
			return spell.CalcPeriodicDamage(sim, target, baseDamage, spell.OutcomeExpectedMagicAlwaysHit)
		},
	})
}
