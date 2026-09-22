package paladin

import (
	"strconv"

	"github.com/wowsims/classic/sim/core"
	"github.com/wowsims/classic/sim/core/stats"
)

// The seal's cost, duration, school and attack speed come from the client table. The ids, the attack
// power and its per level growth stay ours: the table holds the attack power at the rank's max level,
// truncated (spell_damage_test.go checks it). The table puts the judgement on the melee table where the
// sim has it on the spell table; it cannot miss here either way. The judgement's bonus (the table's
// 23 ... 161) was never read here; core.JudgementOfTheCrusaderAura builds it.
var sealOfTheCrusaderRanks = []struct {
	level      int32
	spellID    int32
	scaleLevel int32
	ap         float64
	scale      float64
	judge      judge
}{
	{level: 6, spellID: 21082, scaleLevel: 12, ap: 31, scale: 0.7, judge: judge{spellID: 21183}},
	{level: 12, spellID: 20162, scaleLevel: 20, ap: 51, scale: 1.1, judge: judge{spellID: 20188}},
	{level: 22, spellID: 20305, scaleLevel: 30, ap: 94, scale: 1.7, judge: judge{spellID: 20300}},
	{level: 32, spellID: 20306, scaleLevel: 40, ap: 145, scale: 2, judge: judge{spellID: 20301}},
	{level: 42, spellID: 20307, scaleLevel: 50, ap: 221, scale: 2.2, judge: judge{spellID: 20302}},
	{level: 52, spellID: 20308, scaleLevel: 60, ap: 306, scale: 2.4, judge: judge{spellID: 20303}},
}

func (paladin *Paladin) registerSealOfTheCrusader() {

	// Beta client 1.60.1.69893: Improved Seal of the Crusader's 15% is baked into the judgement
	// (rank 6 140 -> 161, which core.JudgementOfTheCrusaderAura builds as 140 x 1.15) but not into
	// the seal, whose attack power is Classic's at every rank. The debuff also lasts 40 sec
	// instead of 10, which core sets.
	const improvedSotC = 1.15

	var libramAp, libramBonus float64
	if paladin.Ranged().ID == LibramOfFervor {
		libramAp = 48
		libramBonus = 33
	}

	for i, rank := range sealOfTheCrusaderRanks {
		// Rebound so sim/spell_sources_test.go reads this rank's id site as unresolved, as it did before
		// the table moved to package level; ui/core/spells does not declare these ids yet.
		rank := rank
		if paladin.Level < rank.level {
			break
		}
		sealRow := spellData.SealOfTheCrusader.BySpellID(rank.spellID)
		judgeRow := spellData.SealOfTheCrusaderTriggered.BySpellID(rank.judge.spellID)
		attackSpeed := (100 + sealRow.Effects[1].Value) / 100

		debuffs := paladin.NewEnemyAuraArray(func(target *core.Unit) *core.Aura {
			return core.JudgementOfTheCrusaderAura(&paladin.Unit, target, improvedSotC, libramBonus)
		})

		judgeSpell := paladin.RegisterSpell(core.SpellConfig{
			ActionID:    core.ActionID{SpellID: rank.judge.spellID},
			SpellSchool: judgeRow.SpellSchool,
			DefenseType: core.DefenseTypeMagic,
			ProcMask:    core.ProcMaskEmpty,
			Flags:       core.SpellFlagMeleeMetrics,

			ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
				spell.CalcAndDealOutcome(sim, target, spell.OutcomeAlwaysHit)
				debuffs.Get(target).Activate(sim)
			},
		})

		ap := rank.ap + rank.scale*float64(min(paladin.Level, rank.scaleLevel)-rank.level)

		aura := paladin.RegisterAura(core.Aura{
			Label:    "Seal of the Crusader" + paladin.Label + strconv.Itoa(i+1),
			ActionID: core.ActionID{SpellID: rank.spellID},
			Duration: sealRow.Duration,
			OnGain: func(_ *core.Aura, sim *core.Simulation) {
				paladin.MultiplyMeleeSpeed(sim, attackSpeed)
				paladin.AutoAttacks.MHAuto().DamageMultiplier /= attackSpeed
				paladin.AddStatDynamic(sim, stats.AttackPower, ap+libramAp)
			},
			OnExpire: func(_ *core.Aura, sim *core.Simulation) {
				paladin.MultiplyMeleeSpeed(sim, 1/attackSpeed)
				paladin.AutoAttacks.MHAuto().DamageMultiplier *= attackSpeed
				paladin.AddStatDynamic(sim, stats.AttackPower, -(ap + libramAp))
			},
		})

		paladin.aurasSotC = append(paladin.aurasSotC, aura)

		paladin.RegisterSpell(core.SpellConfig{
			ActionID:    aura.ActionID,
			SpellSchool: sealRow.SpellSchool,
			Flags:       core.SpellFlagAPL,

			RequiredLevel: int(rank.level),
			Rank:          i + 1,

			ManaCost: core.ManaCostOptions{
				FlatCost:   float64(sealRow.Cost) - paladin.getLibramSealCostReduction(),
				Multiplier: paladin.benediction(),
			},
			Cast: core.CastConfig{
				DefaultCast: core.Cast{
					GCD: core.GCDDefault,
				},
			},

			ApplyEffects: func(sim *core.Simulation, _ *core.Unit, spell *core.Spell) {
				paladin.applySeal(aura, spell, judgeSpell, sim)
			},
		})

		paladin.spellsJotC = append(paladin.spellsJotC, judgeSpell)
	}
}
