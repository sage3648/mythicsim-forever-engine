package paladin

import (
	"strconv"
	"time"

	"github.com/wowsims/classic/sim/core"
)

// Seal of Command is a spell consisting of:
// - A judgement that has a flat damage roll, and scales with spellpower.
// - A 7ppm on-hit proc with a 1s ICD that deals 70% weapon damage and scales with spellpower.

// Judgement of Command in the Forever client (1.60.1.69893):
// - The judgement is a dummy, 20968, that halves the damage unless the target is stunned (the
//   base damage only is multiplied by 2 if the target is stunned). It is DefenseType Melee with No
//   Active Defense (SpellMisc Attributes[0] 0x200000), so it rolls a melee miss but can't be dodged,
//   parried or blocked. Classic's dummy was Magic and rolled on the spell hit table.
// - The damage spell, 20966, ignores the hit result (Attributes[3] 0x40000) and is Melee, so it
//   crits on the melee table for double damage.

// Every rank's aura triggers the same proc, 20424, in both clients; 20944-20947 were Classic's
// learn-spell dummies and are gone from the beta client (1.60.1.69893). Nothing else moved.
//
// The seal's cost, duration and school, the proc's 70% of weapon damage, school and defense type, and
// the judgement's coefficient, school and defense type come from the client table. The ids and the
// judgement's roll and per level growth stay ours; the table holds the centre at the rank's max level,
// truncated (spell_damage_test.go checks it). The proc's 0.29 coefficient is not in the table.
var sealOfCommandRanks = []struct {
	level      int32
	spellID    int32
	scaleLevel int32
	proc       proc
	judge      judge
}{
	{level: 20, spellID: 20375, scaleLevel: 28, proc: proc{spellID: 20424}, judge: judge{spellID: 20467, minDamage: 93, maxDamage: 101, scale: 5.6}},
	{level: 30, spellID: 20915, scaleLevel: 38, proc: proc{spellID: 20424}, judge: judge{spellID: 20963, minDamage: 146, maxDamage: 160, scale: 6.1}},
	{level: 40, spellID: 20918, scaleLevel: 48, proc: proc{spellID: 20424}, judge: judge{spellID: 20964, minDamage: 204, maxDamage: 224, scale: 5.6}},
	{level: 50, spellID: 20919, scaleLevel: 58, proc: proc{spellID: 20424}, judge: judge{spellID: 20965, minDamage: 261, maxDamage: 287, scale: 6.1}},
	{level: 60, spellID: 20920, scaleLevel: 60, proc: proc{spellID: 20424}, judge: judge{spellID: 20966, minDamage: 339, maxDamage: 373, scale: 6.1}},
}

func (paladin *Paladin) registerSealOfCommand() {
	improvedSeals := paladin.improvedSeals()

	ppmm := paladin.AutoAttacks.NewPPMManager(7, core.ProcMaskMelee)

	icd := core.Cooldown{
		Timer:    paladin.NewTimer(),
		Duration: time.Second * 1,
	}

	for i, rank := range sealOfCommandRanks {
		// Rebound so sim/spell_sources_test.go reads this rank's id site as unresolved, as it did before
		// the table moved to package level; ui/core/spells does not declare these ids yet.
		rank := rank
		if paladin.Level < rank.level {
			break
		}
		sealRow := spellData.SealOfCommand.BySpellID(rank.spellID)
		judgeRow := spellData.SealOfCommandTriggered.BySpellID(rank.judge.spellID)
		procRow := spellData.SealOfCommandTriggered.BySpellID(rank.proc.spellID)

		minDamage := rank.judge.minDamage + float64(min(paladin.Level, rank.scaleLevel)-rank.level)*rank.judge.scale
		maxDamage := rank.judge.maxDamage + float64(min(paladin.Level, rank.scaleLevel)-rank.level)*rank.judge.scale

		judgeSpell := paladin.RegisterSpell(core.SpellConfig{
			SpellCode:      SpellCode_PaladinJudgementOfCommand, // used in judgement.go
			ClassSpellMask: SpellMaskJudgementOfCommand,
			ActionID:       core.ActionID{SpellID: rank.judge.spellID},
			SpellSchool:    judgeRow.SpellSchool,
			DefenseType:    judgeRow.DefenseType,
			ProcMask:       core.ProcMaskMeleeMHSpecial,
			Flags:          core.SpellFlagMeleeMetrics | core.SpellFlagNoOnCastComplete,

			// Improved Seals is a percent modifier, so it belongs on the whole spell rather than on
			// the base roll, which left the coefficient's share of the damage out of it.
			DamageMultiplier: improvedSeals,
			ThreatMultiplier: 1,
			BonusCoefficient: roundCoef(judgeRow.Direct.BonusCoefficient()),

			ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
				baseDamage := sim.Roll(minDamage, maxDamage) * 0.5 // unless stunned

				// The client's dummy, 20968, is DefenseType Melee with No Active Defense: it rolls a melee
				// miss, never a dodge, parry or block. The damage spell it triggers, 20966, ignores the hit
				// result and crits on the melee table.
				spell.CalcAndDealDamage(sim, target, baseDamage, spell.OutcomeMeleeSpecialNoBlockDodgeParry)
			},
		})

		procSpell := paladin.RegisterSpell(core.SpellConfig{
			ActionID:    core.ActionID{SpellID: rank.proc.spellID},
			SpellSchool: procRow.SpellSchool,
			DefenseType: procRow.DefenseType,
			ProcMask:    core.ProcMaskMeleeMHSpecial | core.ProcMaskMeleeProc | core.ProcMaskMeleeDamageProc,
			Flags:       core.SpellFlagMeleeMetrics | core.SpellFlagNotAProc,

			DamageMultiplier: procRow.Effects[0].Value / 100 * improvedSeals,
			ThreatMultiplier: 1,

			BonusCoefficient: 0.29,

			ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
				baseDamage := spell.Unit.MHWeaponDamage(sim, spell.MeleeAttackPower(target))
				result := spell.CalcDamage(sim, target, baseDamage, spell.OutcomeMeleeSpecialHitAndCrit)

				core.StartDelayedAction(sim, core.DelayedActionOptions{
					DoAt: sim.CurrentTime + core.SpellBatchWindow,
					OnAction: func(s *core.Simulation) {
						spell.DealDamage(sim, result)
					},
				})
			},
		})

		aura := paladin.RegisterAura(core.Aura{
			Label:    "Seal of Command" + paladin.Label + strconv.Itoa(i+1),
			ActionID: core.ActionID{SpellID: rank.spellID},
			Duration: sealRow.Duration,
			OnSpellHitDealt: func(aura *core.Aura, sim *core.Simulation, spell *core.Spell, result *core.SpellResult) {
				if !result.Landed() {
					return
				}

				if spell.ProcMask.Matches(core.ProcMaskMeleeWhiteHit) {
					if icd.IsReady(sim) && ppmm.Proc(sim, spell.ProcMask, "seal of command") {
						icd.Use(sim)
						procSpell.Cast(sim, result.Target)
					}
				}
			},
		})

		paladin.aurasSoC = append(paladin.aurasSoC, aura)
		// Echo of Command (client 1311703) "empowers your next melee attack with a chance to
		// activate Seal of Command": the Echo rolls the seal's own chance, it does not land for sure.
		paladin.registerSealProc(aura, func(sim *core.Simulation, target *core.Unit) {
			if icd.IsReady(sim) && ppmm.Proc(sim, core.ProcMaskMeleeMHAuto, "seal of command echo") {
				icd.Use(sim)
				procSpell.Cast(sim, target)
			}
		})

		paladin.sealOfCommand = paladin.RegisterSpell(core.SpellConfig{
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

		paladin.spellsJoC = append(paladin.spellsJoC, judgeSpell)
	}
}
