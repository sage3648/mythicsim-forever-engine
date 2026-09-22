package paladin

import (
	"github.com/wowsims/classic/sim/core"
	"github.com/wowsims/classic/sim/core/proto"
)

// Beta client 1.60.1.69893: every rank hits a little softer (rank 6 505-563 -> 474-530); per level
// growth is Classic's. The damage stays ours: the client table holds one value per rank, the centre
// of the range at the rank's max level (spell_damage_test.go checks it).
var exorcismRanks = []struct {
	level      int32
	scaleLevel int32
	minDamage  float64
	maxDamage  float64
	scale      float64
}{
	{level: 20, scaleLevel: 25, minDamage: 73, maxDamage: 85, scale: 1.2},
	{level: 28, scaleLevel: 33, minDamage: 132, maxDamage: 150, scale: 1.6},
	{level: 36, scaleLevel: 41, minDamage: 189, maxDamage: 215, scale: 2.0},
	{level: 44, scaleLevel: 49, minDamage: 273, maxDamage: 309, scale: 2.4},
	{level: 52, scaleLevel: 57, minDamage: 362, maxDamage: 406, scale: 2.8},
	{level: 60, scaleLevel: 60, minDamage: 474, maxDamage: 530, scale: 3.2},
}

// Cost, cooldown, school, defense type and coefficient come from the client table; the ids stay ours,
// as every rank is registered (see sim/rogue).
func (paladin *Paladin) registerExorcism() {
	for i, rank := range exorcismRanks {
		spellID := []int32{879, 5614, 5615, 10312, 10313, 10314}[i]
		if paladin.Level < rank.level {
			break
		}
		row := spellData.Exorcism.BySpellID(spellID)

		minDamage := rank.minDamage + float64(min(paladin.Level, rank.scaleLevel)-rank.level)*rank.scale
		maxDamage := rank.maxDamage + float64(min(paladin.Level, rank.scaleLevel)-rank.level)*rank.scale

		spell := paladin.RegisterSpell(core.SpellConfig{
			ActionID:    core.ActionID{SpellID: spellID},
			SpellSchool: row.SpellSchool,
			DefenseType: row.DefenseType,
			ProcMask:    core.ProcMaskSpellDamage,
			Flags:       core.SpellFlagMeleeMetrics | core.SpellFlagAPL | core.SpellFlagBinary, //Logs show it never has partial resists, No clue why, still misses

			RequiredLevel: int(rank.level),
			Rank:          i + 1,

			SpellCode:      SpellCode_PaladinExorcism,
			ClassSpellMask: SpellMaskExorcism,
			ManaCost: core.ManaCostOptions{
				FlatCost:   float64(row.Cost),
				Multiplier: paladin.benediction() * paladin.holyConduit() / 100,
			},

			Cast: core.CastConfig{
				DefaultCast: core.Cast{
					GCD: core.GCDDefault,
				},
				CD: core.Cooldown{
					Timer:    paladin.NewTimer(),
					Duration: paladin.purifyingPower(row.Cooldown),
				},
			},

			DamageMultiplier: 1,
			ThreatMultiplier: 1,

			BonusCoefficient: roundCoef(row.Direct.BonusCoefficient()),

			ExtraCastCondition: func(sim *core.Simulation, target *core.Unit) bool {
				return target.MobType == proto.MobType_MobTypeDemon || target.MobType == proto.MobType_MobTypeUndead
			},

			ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
				spell.CalcAndDealDamage(sim, target, sim.Roll(minDamage, maxDamage), spell.OutcomeMagicHitAndCrit)
			},
		})

		paladin.exorcism = append(paladin.exorcism, spell)
	}
}
