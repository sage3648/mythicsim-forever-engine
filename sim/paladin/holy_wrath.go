package paladin

import (
	"github.com/wowsims/classic/sim/core/proto"

	"github.com/wowsims/classic/sim/core"
)

// The damage stays ours: the client table holds the centre of the range at the rank's max level,
// truncated (spell_damage_test.go checks it).
var holyWrathRanks = []struct {
	level      int32
	spellID    int32
	scaleLevel int32
	minDamage  float64
	maxDamage  float64
	scale      float64
}{
	{level: 50, spellID: 2812, scaleLevel: 54, minDamage: 362, maxDamage: 428, scale: 1.6},
	{level: 60, spellID: 10318, scaleLevel: 60, minDamage: 490, maxDamage: 576, scale: 1.9},
}

// Cost, cast time, cooldown, school, defense type and coefficient come from the client table; the ids
// stay ours, as every rank is registered (see sim/rogue). The client's missile speed (20) is not
// applied, as before.
func (paladin *Paladin) registerHolyWrath() {
	var results []*core.SpellResult

	for i, rank := range holyWrathRanks {
		// Rebound so sim/spell_sources_test.go reads this rank's id site as unresolved, as it did before
		// the table moved to package level; ui/core/spells does not declare these ids yet.
		rank := rank
		if paladin.Level < rank.level {
			break
		}
		row := spellData.HolyWrath.BySpellID(rank.spellID)

		minDamage := rank.minDamage + float64(min(paladin.Level, rank.scaleLevel)-rank.level)*rank.scale
		maxDamage := rank.maxDamage + float64(min(paladin.Level, rank.scaleLevel)-rank.level)*rank.scale

		holyWrathSpell := paladin.GetOrRegisterSpell(core.SpellConfig{
			SpellCode:      SpellCode_PaladinHolyWrath,
			ClassSpellMask: SpellMaskHolyWrath,
			ActionID:       core.ActionID{SpellID: rank.spellID},
			SpellSchool:    row.SpellSchool,
			DefenseType:    row.DefenseType,
			ProcMask:       core.ProcMaskSpellDamage, // TODO to be tested
			Flags:          core.SpellFlagAPL,

			RequiredLevel: int(rank.level),
			Rank:          i + 1,

			ManaCost: core.ManaCostOptions{
				FlatCost:   float64(row.Cost),
				Multiplier: paladin.holyConduit(),
			},
			Cast: core.CastConfig{
				DefaultCast: core.Cast{
					GCD:      core.GCDDefault,
					CastTime: row.CastTime,
				},

				CD: core.Cooldown{
					Timer:    paladin.NewTimer(),
					Duration: paladin.purifyingPower(row.Cooldown),
				},
			},

			DamageMultiplier: 1.0,
			ThreatMultiplier: 1,
			BonusCoefficient: roundCoef(row.Direct.BonusCoefficient()),

			ApplyEffects: func(sim *core.Simulation, _ *core.Unit, spell *core.Spell) {
				results = results[:0]
				for _, target := range paladin.Env.Encounter.TargetUnits {
					if target.MobType == proto.MobType_MobTypeDemon || target.MobType == proto.MobType_MobTypeUndead {
						damage := sim.Roll(minDamage, maxDamage)
						result := spell.CalcDamage(sim, target, damage, spell.OutcomeMagicHitAndCrit)
						results = append(results, result)
					}
				}

				for _, result := range results {
					spell.DealDamage(sim, result)
				}
			},
		})

		paladin.holyWrath = append(paladin.holyWrath, holyWrathSpell)
	}
}
