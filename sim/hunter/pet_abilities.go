package hunter

import (
	"time"

	"github.com/wowsims/classic/sim/common/shared"
	"github.com/wowsims/classic/sim/core"
)

type PetAbilityType int

// Pet AI doesn't use abilities immediately, so model this with a 1.6s GCD.
const PetGCD = time.Millisecond * 1600

const (
	Unknown PetAbilityType = iota
	Bite
	Claw
	Screech
	FuriousHowl
	LightningBreath
	ScorpidPoison
)

// Pet ability ids by the owner's level. Damage ranges stay ours (the client table holds the
// truncated centre, see aimed_shot.go); spell_damage_test.go checks each still contains it.
var PetClawSpellID = map[int32]int32{
	25: 16830,
	40: 16832,
	50: 3010,
	60: 3009,
}
var PetClawDamage = map[int32][]float64{25: {16, 22}, 40: {26, 36}, 50: {35, 49}, 60: {43, 59}}

var PetBiteSpellID = map[int32]int32{
	25: 17257,
	40: 17259,
	50: 17260,
	60: 17261,
}
var PetBiteDamage = map[int32][]float64{25: {31, 37}, 40: {49, 59}, 50: {66, 80}, 60: {81, 99}}

var PetLightningBreathSpellID = map[int32]int32{
	25: 25009,
	40: 25009, // rank 4 not available in SoD Phase 2
	50: 25011,
	60: 25012,
}
var PetLightningBreathDamage = map[int32][]float64{25: {32, 36}, 40: {32, 36}, 50: {71, 81}, 60: {86, 98}}

var PetScreechSpellID = map[int32]int32{
	15: 24580,
	40: 24580,
	50: 24581,
	60: 24582,
}
var PetScreechDamage = map[int32][]float64{25: {9, 13}, 40: {9, 13}, 50: {21, 27}, 60: {24, 42}}

func (hp *HunterPet) NewPetAbility(abilityType PetAbilityType, isPrimary bool) *core.Spell {
	switch abilityType {
	case Bite:
		return hp.newBite()
	case Claw:
		return hp.newClaw()
	case Screech:
		return hp.newScreech()
	// case FuriousHowl:
	// 	return hp.newFuriousHowl()
	case LightningBreath:
		return hp.newLightningBreath()
	case ScorpidPoison:
		return hp.newScorpidPoison()
	// case Swipe:
	// 	return hp.newSwipe()
	case Unknown:
		return nil
	default:
		panic("Invalid pet ability type")
	}
}

func (hp *HunterPet) newClaw() *core.Spell {
	spellID := PetClawSpellID[hp.Owner.Level]
	baseDamageMin, baseDamageMax := PetClawDamage[hp.Owner.Level][0], PetClawDamage[hp.Owner.Level][1]
	row := spellData.ClawTriggered.BySpellID(spellID)

	return hp.RegisterSpell(core.SpellConfig{
		ActionID:       core.ActionID{SpellID: spellID},
		SpellCode:      SpellCode_HunterPetClaw,
		ClassSpellMask: SpellMaskPetClaw,
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskMeleeMHSpecial,
		Flags:          core.SpellFlagMeleeMetrics,

		FocusCost: core.FocusCostOptions{
			Cost: float64(row.Cost),
		},
		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD: PetGCD,
			},
			IgnoreHaste: true,
		},

		DamageMultiplier: 1,
		ThreatMultiplier: 1,
		BonusCoefficient: roundCoef(row.Direct.BonusCoefficient()),

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			baseDamage := sim.Roll(baseDamageMin, baseDamageMax)
			spell.CalcAndDealDamage(sim, target, baseDamage, spell.OutcomeMeleeSpecialHitAndCrit)
		},
	})
}

func (hp *HunterPet) newBite() *core.Spell {
	spellID := PetBiteSpellID[hp.Owner.Level]
	baseDamageMin, baseDamageMax := PetBiteDamage[hp.Owner.Level][0], PetBiteDamage[hp.Owner.Level][1]
	row := spellData.BiteTriggered.BySpellID(spellID)

	return hp.RegisterSpell(core.SpellConfig{
		ActionID:       core.ActionID{SpellID: spellID},
		SpellCode:      SpellCode_HunterPetBite,
		ClassSpellMask: SpellMaskPetBite,
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskMeleeMHSpecial,
		Flags:          core.SpellFlagMeleeMetrics,

		FocusCost: core.FocusCostOptions{
			Cost: float64(row.Cost),
		},
		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD: PetGCD,
			},
			CD: core.Cooldown{
				Timer:    hp.NewTimer(),
				Duration: row.Cooldown,
			},
			IgnoreHaste: true,
		},

		DamageMultiplier: 1,
		ThreatMultiplier: 1,
		BonusCoefficient: roundCoef(row.Direct.BonusCoefficient()),

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			baseDamage := sim.Roll(baseDamageMin, baseDamageMax)
			spell.CalcAndDealDamage(sim, target, baseDamage, spell.OutcomeMeleeSpecialHitAndCrit)
		},
	})
}

// Beta client: every rank lower, and no more growth per level.
func (hp *HunterPet) newLightningBreath() *core.Spell {
	spellID := PetLightningBreathSpellID[hp.Owner.Level]
	baseDamageMin, baseDamageMax := PetLightningBreathDamage[hp.Owner.Level][0], PetLightningBreathDamage[hp.Owner.Level][1]
	// The table's coefficient of 0 is not used; ours stays 1.
	row := spellData.LightningBreathTriggered.BySpellID(spellID)

	return hp.RegisterSpell(core.SpellConfig{
		ActionID:       core.ActionID{SpellID: spellID},
		SpellCode:      SpellCode_HunterPetLightningBreath,
		ClassSpellMask: SpellMaskPetLightningBreath,
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskSpellDamage,

		FocusCost: core.FocusCostOptions{
			Cost: float64(row.Cost),
		},
		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD: PetGCD,
			},
			IgnoreHaste: true,
		},

		DamageMultiplier: 1,
		ThreatMultiplier: 1,
		BonusCoefficient: 1,

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			baseDamage := sim.Roll(baseDamageMin, baseDamageMax)

			spell.CalcAndDealDamage(sim, target, baseDamage, spell.OutcomeMagicHitAndCrit)
		},
	})
}

// Demoralizing Screech in the beta client: new damage, and a 10 sec cooldown Classic did not have.
func (hp *HunterPet) newScreech() *core.Spell {
	spellID := PetScreechSpellID[hp.Owner.Level]
	baseDamageMin, baseDamageMax := PetScreechDamage[hp.Owner.Level][0], PetScreechDamage[hp.Owner.Level][1]
	// Cost, cooldown, school and defense type sit on the rank's triggered row.
	row := spellData.DemoralizingScreechTriggered.ByRank(spellData.DemoralizingScreech.BySpellID(spellID).Rank)

	return hp.RegisterSpell(core.SpellConfig{
		ActionID:       core.ActionID{SpellID: spellID},
		SpellCode:      SpellCode_HunterPetScreech,
		ClassSpellMask: SpellMaskPetScreech,
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskMeleeSpecial,
		Flags:          core.SpellFlagMeleeMetrics,

		FocusCost: core.FocusCostOptions{
			Cost: float64(row.Cost),
		},
		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD: PetGCD,
			},
			IgnoreHaste: true,
			CD: core.Cooldown{
				Timer:    hp.NewTimer(),
				Duration: row.Cooldown,
			},
		},

		DamageMultiplier: 1,
		ThreatMultiplier: 1,

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			baseDamage := sim.Roll(baseDamageMin, baseDamageMax)
			// This ability also applies a melee attack power reduction similar to demoralizing shout - left it out for now
			spell.CalcAndDealDamage(sim, target, baseDamage, spell.OutcomeMeleeSpecialHitAndCrit)
		},
	})
}

// func (hp *HunterPet) newFuriousHowl() *core.Spell {
// 	actionID := core.ActionID{SpellID: 64495}

// 	petAura := hp.NewTemporaryStatsAura("FuriousHowl", actionID, stats.Stats{stats.AttackPower: 320, stats.RangedAttackPower: 320}, time.Second*20)
// 	ownerAura := hp.hunterOwner.NewTemporaryStatsAura("FuriousHowl", actionID, stats.Stats{stats.AttackPower: 320, stats.RangedAttackPower: 320}, time.Second*20)

// 	howlSpell := hp.RegisterSpell(core.SpellConfig{
// 		ActionID: actionID,

// 		FocusCost: core.FocusCostOptions{
// 			Cost: 20,
// 		},
// 		Cast: core.CastConfig{
// 			CD: core.Cooldown{
// 				Timer:    hp.NewTimer(),
// 				Duration: time.Second * 40,
// 			},
// 		},
// 		ExtraCastCondition: func(sim *core.Simulation, target *core.Unit) bool {
// 			return hp.IsEnabled()
// 		},
// 		ApplyEffects: func(sim *core.Simulation, _ *core.Unit, _ *core.Spell) {
// 			petAura.Activate(sim)
// 			ownerAura.Activate(sim)
// 		},
// 	})

// 	hp.hunterOwner.RegisterSpell(core.SpellConfig{
// 		ActionID: actionID,
// 		Flags:    core.SpellFlagAPL | core.SpellFlagMCD,
// 		ExtraCastCondition: func(sim *core.Simulation, target *core.Unit) bool {
// 			return howlSpell.CanCast(sim, target)
// 		},
// 		ApplyEffects: func(sim *core.Simulation, target *core.Unit, _ *core.Spell) {
// 			howlSpell.Cast(sim, target)
// 		},
// 	})

// 	hp.hunterOwner.AddMajorCooldown(core.MajorCooldown{
// 		Spell: howlSpell,
// 		Type:  core.CooldownTypeDPS,
// 	})

// 	return nil
// }

func (hp *HunterPet) newScorpidPoison() *core.Spell {
	// Beta client: 2/4/5 a tick, down from 3/6/8. Everything but the id comes from the client table.
	spellID := map[int32]int32{
		25: 24583,
		40: 24586,
		50: 24586,
		60: 24587,
	}[hp.Owner.Level]
	row := spellData.ScorpidPoisonTriggered.BySpellID(spellID)
	periodic := row.Periodic.(shared.SpellDataPeriodic)
	baseDamageTick := periodic.Tick

	return hp.RegisterSpell(core.SpellConfig{
		ActionID:       core.ActionID{SpellID: spellID},
		SpellCode:      SpellCode_HunterPetScorpidPoison,
		ClassSpellMask: SpellMaskPetScorpidPoison,
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskMeleeMHSpecial,
		Flags:          core.SpellFlagPassiveSpell | core.SpellFlagPoison,

		FocusCost: core.FocusCostOptions{
			Cost: float64(row.Cost),
		},
		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD: PetGCD,
			},
			IgnoreHaste: true,
			CD: core.Cooldown{
				Timer:    hp.NewTimer(),
				Duration: row.Cooldown,
			},
		},

		DamageMultiplier: 1,
		ThreatMultiplier: 1,

		Dot: core.DotConfig{
			Aura: core.Aura{
				Label:     "ScorpidPoison",
				MaxStacks: 5,
				Duration:  row.Duration,
			},
			NumberOfTicks: periodic.NumberOfTicks,
			TickLength:    periodic.TickLength,

			OnSnapshot: func(sim *core.Simulation, target *core.Unit, dot *core.Dot, applyStack bool) {
				if !applyStack {
					return
				}

				// only the first stack snapshots the multiplier
				if dot.GetStacks() == 1 {
					attackTable := dot.Spell.Unit.AttackTables[target.UnitIndex][dot.Spell.CastType]
					dot.SnapshotAttackerMultiplier = dot.Spell.AttackerDamageMultiplier(attackTable, true)
					dot.SnapshotBaseDamage = 0
				}

				dot.SnapshotBaseDamage += baseDamageTick
			},
			OnTick: func(sim *core.Simulation, target *core.Unit, dot *core.Dot) {
				dot.CalcAndDealPeriodicSnapshotDamage(sim, target, dot.OutcomeTick)
			},
		},

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			result := spell.CalcAndDealOutcome(sim, target, spell.OutcomeMeleeSpecialHit)
			if !result.Landed() {
				return
			}

			dot := spell.Dot(target)
			dot.ApplyOrRefresh(sim)
			if dot.GetStacks() < dot.MaxStacks {
				dot.AddStack(sim)
				dot.TakeSnapshot(sim, true)
			}
		},
	})
}
