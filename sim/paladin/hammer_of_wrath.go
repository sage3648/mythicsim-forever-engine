package paladin

import (
	"time"

	"github.com/wowsims/classic/sim/core/proto"

	"github.com/wowsims/classic/sim/core"
)

// Beta client 1.60.1.69893, with each rank's growth to its max level folded in: rank 3 504-566
// (Classic's client reads 504-556) -> 473-523. The damage stays ours: the client table holds the
// centre of the range, truncated (spell_damage_test.go checks it).
var hammerOfWrathRanks = []struct {
	level     int32
	minDamage float64
	maxDamage float64
}{
	{level: 44, minDamage: 285, maxDamage: 315},
	{level: 52, minDamage: 382, maxDamage: 421},
	{level: 60, minDamage: 473, maxDamage: 523},
}

// Cost, cast time, cooldown, school, defense type and coefficient come from the client table; the
// ids stay ours, as every rank is registered (see sim/rogue). The client's missile speed (35) is not
// applied, as before.
func (paladin *Paladin) registerHammerOfWrath() {
	cd := core.Cooldown{
		Timer:    paladin.NewTimer(),
		Duration: spellData.HammerOfWrath.ByRank(1).Cooldown,
	}

	// Instrument of Law: 0.5 sec a rank, confirmed by the beta client's talent data.
	castTime := spellData.HammerOfWrath.ByRank(1).CastTime - time.Millisecond*500*time.Duration(paladin.Talents.InstrumentOfLaw)

	for i, rank := range hammerOfWrathRanks {
		spellID := []int32{24275, 24274, 24239}[i]
		if paladin.Level < rank.level {
			break
		}
		row := spellData.HammerOfWrath.BySpellID(spellID)

		paladin.GetOrRegisterSpell(core.SpellConfig{
			ActionID:    core.ActionID{SpellID: spellID},
			SpellSchool: row.SpellSchool,
			DefenseType: row.DefenseType,
			ProcMask:    core.ProcMaskRangedSpecial, // TODO to be tested
			Flags:       core.SpellFlagMeleeMetrics | core.SpellFlagAPL,
			CastType:    proto.CastType_CastTypeRanged,

			Rank:           i + 1,
			RequiredLevel:  int(rank.level),
			SpellCode:      SpellCode_PaladinHammerOfWrath,
			ClassSpellMask: SpellMaskHammerOfWrath,

			ManaCost: core.ManaCostOptions{
				FlatCost:   float64(row.Cost),
				Multiplier: paladin.holyConduit(),
			},
			Cast: core.CastConfig{
				DefaultCast: core.Cast{
					GCD:      time.Second,
					CastTime: castTime,
				},
				IgnoreHaste: true,
				CD:          cd,
			},

			DamageMultiplier: 1,
			ThreatMultiplier: 1,
			BonusCoefficient: roundCoef(row.Direct.BonusCoefficient()),

			ExtraCastCondition: func(sim *core.Simulation, target *core.Unit) bool {
				return sim.IsExecutePhase20()
			},

			ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
				damage := sim.Roll(rank.minDamage, rank.maxDamage)
				spell.CalcAndDealDamage(sim, target, damage, spell.OutcomeRangedHitAndCrit)
			},
		})
	}
}
