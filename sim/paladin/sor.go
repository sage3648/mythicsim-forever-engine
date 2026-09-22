package paladin

import (
	"strconv"

	"github.com/wowsims/classic/sim/core"
)

// A seal's on-hit proc and its judgement, shared by every seal's rank table. Seal of Command's proc and
// Seal of the Crusader's judgement carry only the id. The names are what sim/spell_sources_test.go
// follows `rank.judge.spellID` through.
type proc struct {
	spellID int32
	value   float64
	scale   float64
	coeff   float64
}

type judge struct {
	spellID   int32
	minDamage float64
	maxDamage float64
	scale     float64
}

// Beta client 1.60.1.69893: no downranking penalty, so ranks 1-3 take the full 0.1 on the seal
// and 0.5 on the judgement (Classic 0.029/0.063/0.093 and 0.144/0.312/0.462), and rank 1's
// judgement rolls 14-16 instead of a flat 15. Ranks 4-8 are unchanged.
//
// The seal's cost and duration and the judgement's coefficient and school come from the client table.
// The rest stays ours: the ids (rank 1's seal is 21084 there), the per level growth, and the
// judgement's roll, where the table holds the centre at the rank's max level, truncated; the seal's
// value there is ours at its max level (spell_damage_test.go checks both). The table's seal
// coefficient (0.058/0.125/0.185/0.2) is not the 0.1 the sim applies to the proc, and it puts the
// judgement on the melee table where the sim rolls it on the spell table.
var sealOfRighteousnessRanks = []struct {
	level      int32
	spellID    int32
	scaleLevel int32
	proc       proc
	judge      judge
}{
	{level: 1, spellID: 20154, scaleLevel: 7, proc: proc{spellID: 25742, value: 108, scale: 18, coeff: 0.1}, judge: judge{spellID: 20187, minDamage: 14, maxDamage: 16, scale: 1.8}},
	{level: 10, spellID: 20287, scaleLevel: 16, proc: proc{spellID: 25740, value: 216, scale: 17, coeff: 0.1}, judge: judge{spellID: 20280, minDamage: 25, maxDamage: 27, scale: 1.9}},
	{level: 18, spellID: 20288, scaleLevel: 24, proc: proc{spellID: 25739, value: 352, scale: 23, coeff: 0.1}, judge: judge{spellID: 20281, minDamage: 39, maxDamage: 43, scale: 2.4}},
	{level: 26, spellID: 20289, scaleLevel: 32, proc: proc{spellID: 25738, value: 541, scale: 31, coeff: 0.1}, judge: judge{spellID: 20282, minDamage: 57, maxDamage: 63, scale: 2.8}},
	{level: 34, spellID: 20290, scaleLevel: 40, proc: proc{spellID: 25737, value: 785, scale: 37, coeff: 0.1}, judge: judge{spellID: 20283, minDamage: 78, maxDamage: 86, scale: 3.1}},
	{level: 42, spellID: 20291, scaleLevel: 48, proc: proc{spellID: 25736, value: 1082, scale: 41, coeff: 0.1}, judge: judge{spellID: 20284, minDamage: 102, maxDamage: 112, scale: 3.8}},
	{level: 50, spellID: 20292, scaleLevel: 56, proc: proc{spellID: 25735, value: 1407, scale: 47, coeff: 0.1}, judge: judge{spellID: 20285, minDamage: 131, maxDamage: 143, scale: 4.1}},
	{level: 58, spellID: 20293, scaleLevel: 60, proc: proc{spellID: 25713, value: 1786, scale: 47, coeff: 0.1}, judge: judge{spellID: 20286, minDamage: 162, maxDamage: 178, scale: 4.1}},
}

func (paladin *Paladin) registerSealOfRighteousness() {
	improvedSeals := paladin.improvedSeals()

	for i, rank := range sealOfRighteousnessRanks {
		// Rebound so sim/spell_sources_test.go reads this rank's id site as unresolved, as it did before
		// the table moved to package level; ui/core/spells does not declare these ids yet.
		rank := rank
		if paladin.Level < rank.level {
			break
		}
		sealRow := spellData.SealOfRighteousness.ByRank(int32(i + 1))
		judgeRow := spellData.JudgementOfRighteousness.BySpellID(rank.judge.spellID)

		/*
		 * Seal of Righteousness is a Spell/Aura that when active makes the paladin capable of procing
		 * two different SpellIDs depending on a paladin's casted spell or melee swing.
		 *
		 * (Judgement of Righteousness):
		 *   - Deals flat damage that is affected by the Improved Seals talent, and
		 *     has a spellpower scaling that is unaffected by that talent.
		 *   - Targets magic defense and rolls to hit and crit.
		 *
		 * (Seal of Righteousness):
		 *   - Procs from white hits.
		 *   - Cannot miss or be dodged/parried/blocked if the underlying white hit lands.
		 *   - Deals damage that is a function of weapon speed, and spellpower.
		 *   - Has 0.85 scale factor on base damage if using 1h, 1.2 if using 2h.
		 *   - Calculates damage including spellpower scaling but ignoring damage multipliers,
		 *      then feeds that value as base damage into the proc spell.
		 */

		minDamage := rank.judge.minDamage + rank.judge.scale*float64(min(paladin.Level, rank.scaleLevel)-rank.level)
		maxDamage := rank.judge.maxDamage + rank.judge.scale*float64(min(paladin.Level, rank.scaleLevel)-rank.level)

		judgeSpell := paladin.RegisterSpell(core.SpellConfig{
			SpellCode:      SpellCode_PaladinJudgementOfRighteousness,
			ClassSpellMask: SpellMaskJudgementOfRighteousness,
			ActionID:       core.ActionID{SpellID: rank.judge.spellID},
			SpellSchool:    judgeRow.SpellSchool,
			DefenseType:    core.DefenseTypeMagic,
			ProcMask:       core.ProcMaskSpellDamage,
			Flags:          core.SpellFlagMeleeMetrics | core.SpellFlagSuppressWeaponProcs | core.SpellFlagSuppressEquipProcs | core.SpellFlagBinary,

			// Improved Seals is a percent modifier (aura 108), so it scales the whole spell, spell
			// power included. Multiplying the base roll by it left the coefficient's share out.
			DamageMultiplier: improvedSeals,
			ThreatMultiplier: 1,

			BonusCoefficient: roundCoef(judgeRow.Direct.BonusCoefficient()),

			ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
				baseDamage := sim.Roll(minDamage, maxDamage)
				spell.CalcAndDealDamage(sim, target, baseDamage, spell.OutcomeMagicHitAndCrit)
			},
		})

		value := 0.01 * (rank.proc.value + rank.proc.scale*float64(min(paladin.Level, rank.scaleLevel)-rank.level))

		coeff := rank.proc.coeff
		damage := value * 0.85 * paladin.MainHand().SwingSpeed
		if paladin.has2hEquipped() {
			coeff = rank.proc.coeff * 1.1 // from testing in SoD
			damage = value * 1.2 * paladin.MainHand().SwingSpeed
		}

		procSpell := paladin.RegisterSpell(core.SpellConfig{
			ActionID:    core.ActionID{SpellID: rank.proc.spellID},
			SpellSchool: core.SpellSchoolHoly,
			DefenseType: core.DefenseTypeMelee,
			ProcMask:    core.ProcMaskMeleeMHSpecial,                                   //changed to ProcMaskMeleeMHSpecial, to allow procs from weapons/oils which do proc from SoR,
			Flags:       core.SpellFlagMeleeMetrics | core.SpellFlagSuppressEquipProcs, // but Wild Strikes does not proc, nor equip procs

			//BonusCritRating: paladin.holyCrit(), // TODO to be tested, but unlikely

			DamageMultiplier: improvedSeals * paladin.getWeaponSpecializationModifier(),
			ThreatMultiplier: 1,

			BonusCoefficient: coeff,

			ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
				// effectively scales with coeff x 2, and damage dealt multipliers affect half the damage taken bonus
				baseDamage := damage + spell.BonusCoefficient*(spell.GetBonusDamage(target)+target.GetSchoolBonusDamageTaken(spell))
				spell.CalcAndDealDamage(sim, target, baseDamage, spell.OutcomeMeleeSpecialCritOnly)
			},
		})

		aura := paladin.RegisterAura(core.Aura{
			Label:    "Seal of Righteousness" + paladin.Label + strconv.Itoa(i+1),
			ActionID: core.ActionID{SpellID: rank.spellID},
			Duration: sealRow.Duration,

			OnSpellHitDealt: func(_ *core.Aura, sim *core.Simulation, spell *core.Spell, result *core.SpellResult) {
				if !result.Landed() {
					return
				}
				if spell.ProcMask.Matches(core.ProcMaskMeleeWhiteHit) {
					procSpell.Cast(sim, result.Target)
				}
			},
		})

		paladin.aurasSoR = append(paladin.aurasSoR, aura)
		paladin.registerSealProc(aura, procSpell)

		paladin.sealOfRighteousness = paladin.RegisterSpell(core.SpellConfig{
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

		paladin.spellsJoR = append(paladin.spellsJoR, judgeSpell)
	}
}
