package shaman

import (
	"fmt"
	"time"

	"github.com/wowsims/classic/sim/common/shared"
	"github.com/wowsims/classic/sim/core"
)

const SearingTotemRanks = 6

// Forever beta client: the attack's damage is unchanged but its coefficient falls to 0.017 at every rank. The
// totem's id, cost, duration and school and the attack's id, coefficient, school and defense type come from the
// client table; the attack's damage range stays here (the table holds its centre).
var SearingTotemBaseDamage = [SearingTotemRanks + 1][]float64{{0}, {9, 11}, {13, 17}, {19, 25}, {26, 34}, {33, 45}, {40, 54}}
var SearingTotemLevel = [SearingTotemRanks + 1]int{0, 10, 20, 30, 40, 50, 60}

func (shaman *Shaman) registerSearingTotemSpell() {
	shaman.SearingTotem = make([]*core.Spell, SearingTotemRanks+1)

	for rank := 1; rank <= SearingTotemRanks; rank++ {
		config := shaman.newSearingTotemSpellConfig(rank)

		if config.RequiredLevel <= int(shaman.Level) {
			shaman.SearingTotem[rank] = shaman.RegisterSpell(config)
		}
	}

	shaman.FireTotems = append(
		shaman.FireTotems,
		core.FilterSlice(shaman.SearingTotem, func(spell *core.Spell) bool { return spell != nil })...,
	)
}

func (shaman *Shaman) newSearingTotemSpellConfig(rank int) core.SpellConfig {
	row := spellData.SearingTotem.ByRank(int32(rank))
	attackRow := spellData.SearingTotemTriggered.ByRank(int32(rank))
	baseDamageLow := SearingTotemBaseDamage[rank][0]
	baseDamageHigh := SearingTotemBaseDamage[rank][1]
	duration := row.Duration
	level := SearingTotemLevel[rank]

	// The table's 2.2 sec attack cast and speed 19 missile are not used (see the tick comment below).
	attackInterval := time.Millisecond * 2500

	attackSpell := shaman.RegisterSpell(core.SpellConfig{
		SpellCode:      SpellCode_ShamanSearingTotem,
		ClassSpellMask: SpellMaskSearingTotem,
		ActionID:       core.ActionID{SpellID: attackRow.SpellID},
		SpellSchool:    attackRow.SpellSchool,
		DefenseType:    attackRow.DefenseType,
		ProcMask:       core.ProcMaskEmpty,
		Flags:          SpellFlagTotem,

		DamageMultiplier: shaman.callOfFlameMultiplier(),
		BonusCoefficient: roundCoef(attackRow.Direct.BonusCoefficient()),

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			baseDamage := sim.Roll(baseDamageLow, baseDamageHigh)
			spell.CalcAndDealDamage(sim, target, baseDamage, spell.OutcomeMagicHitAndCrit)
		},
	})

	spell := core.SpellConfig{
		SpellCode:      SpellCode_ShamanSearingTotem,
		ClassSpellMask: SpellMaskSearingTotem,
		ActionID:       core.ActionID{SpellID: row.SpellID},
		SpellSchool:    row.SpellSchool,
		// The table's totem row names no defense type; ours has always been Magic.
		DefenseType: core.DefenseTypeMagic,
		ProcMask:    core.ProcMaskEmpty,
		Flags:       SpellFlagTotem | core.SpellFlagAPL,

		RequiredLevel: level,
		Rank:          rank,

		ManaCost: core.ManaCostOptions{
			FlatCost:   float64(row.Cost),
			Multiplier: shaman.totemManaMultiplier(),
		},

		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD: totemGCD,
			},
			IgnoreHaste: true,
		},

		Dot: core.DotConfig{
			Aura: core.Aura{
				Label: fmt.Sprintf("Searing Totem (Rank %d)", rank),
			},
			// These are the real tick values, but searing totem doesn't start its next
			// cast until the previous missile hits the target. We don't have an option
			// for target distance yet so just pretend the tick rate is lower.
			// https://wotlk.wowhead.com/spell=25530/attack
			//TickLength:           time.Second * 2.2,
			NumberOfTicks: int32(duration / attackInterval),
			TickLength:    attackInterval,
			OnTick: func(sim *core.Simulation, target *core.Unit, dot *core.Dot) {
				attackSpell.Cast(sim, target)
			},
		},

		ApplyEffects: func(sim *core.Simulation, _ *core.Unit, spell *core.Spell) {
			if shaman.ActiveTotems[FireTotem] != nil {
				shaman.ActiveTotems[FireTotem].Dot(sim.GetTargetUnit(0)).Cancel(sim)
			}
			spell.Dot(sim.GetTargetUnit(0)).Apply(sim)
			// +1 needed because of rounding issues with totem tick time.
			shaman.TotemExpirations[FireTotem] = sim.CurrentTime + duration + 1
			shaman.ActiveTotems[FireTotem] = spell
		},
	}

	return spell
}

const MagmaTotemRanks = 4

// Forever beta client values for the pulse. The totem's id, cost, duration, school and defense type and the
// pulse's damage, coefficient, school and defense type come from the client table. The pulse ids stay here: the
// table ranks 8 of them (8188 and 10582-10584 unused), so each pulse row is looked up by its id.
var MagmaTotemAoeSpellId = [MagmaTotemRanks + 1]int32{0, 8187, 10579, 10580, 10581}
var MagmaTotemLevel = [MagmaTotemRanks + 1]int{0, 26, 36, 46, 56}

func (shaman *Shaman) registerMagmaTotemSpell() {
	shaman.MagmaTotem = make([]*core.Spell, MagmaTotemRanks+1)

	for rank := 1; rank <= MagmaTotemRanks; rank++ {
		config := shaman.newMagmaTotemSpellConfig(rank)

		if config.RequiredLevel <= int(shaman.Level) {
			shaman.MagmaTotem[rank] = shaman.RegisterSpell(config)
		}
	}

	shaman.FireTotems = append(
		shaman.FireTotems,
		core.FilterSlice(shaman.MagmaTotem, func(spell *core.Spell) bool { return spell != nil })...,
	)
}

func (shaman *Shaman) newMagmaTotemSpellConfig(rank int) core.SpellConfig {
	row := spellData.MagmaTotem.ByRank(int32(rank))
	aoeRow := spellData.MagmaTotemTriggered.BySpellID(MagmaTotemAoeSpellId[rank])
	baseDamage := aoeRow.Direct.(shared.SpellDataFlat).Value
	level := MagmaTotemLevel[rank]

	duration := row.Duration
	attackInterval := time.Second * 2

	aoeSpell := shaman.RegisterSpell(core.SpellConfig{
		SpellCode:      SpellCode_ShamanMagmaTotem,
		ClassSpellMask: SpellMaskMagmaTotem,
		ActionID:       core.ActionID{SpellID: MagmaTotemAoeSpellId[rank]},
		SpellSchool:    aoeRow.SpellSchool,
		DefenseType:    aoeRow.DefenseType,
		ProcMask:       core.ProcMaskEmpty,
		Flags:          SpellFlagTotem,

		DamageMultiplier: shaman.callOfFlameMultiplier(),
		BonusCoefficient: roundCoef(aoeRow.Direct.BonusCoefficient()),

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			for _, aoeTarget := range sim.Encounter.TargetUnits {
				spell.CalcAndDealDamage(sim, aoeTarget, baseDamage, spell.OutcomeMagicHitAndCrit)
			}
		},
	})

	spell := core.SpellConfig{
		SpellCode:      SpellCode_ShamanMagmaTotem,
		ClassSpellMask: SpellMaskMagmaTotem,
		ActionID:       core.ActionID{SpellID: row.SpellID},
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskEmpty,
		Flags:          SpellFlagTotem | core.SpellFlagAPL,

		RequiredLevel: level,
		Rank:          rank,

		ManaCost: core.ManaCostOptions{
			FlatCost:   float64(row.Cost),
			Multiplier: shaman.totemManaMultiplier(),
		},

		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD: totemGCD,
			},
			IgnoreHaste: true,
		},

		Dot: core.DotConfig{
			Aura: core.Aura{
				Label: fmt.Sprintf("Magma Totem (Rank %d)", rank),
			},
			NumberOfTicks: int32(duration / attackInterval),
			TickLength:    attackInterval,

			OnTick: func(sim *core.Simulation, target *core.Unit, dot *core.Dot) {
				aoeSpell.Cast(sim, target)
			},
		},

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			if shaman.ActiveTotems[FireTotem] != nil {
				shaman.ActiveTotems[FireTotem].Dot(sim.GetTargetUnit(0)).Cancel(sim)
			}
			spell.Dot(sim.GetTargetUnit(0)).Apply(sim)
			// +1 needed because of rounding issues with totem tick time.
			shaman.TotemExpirations[FireTotem] = sim.CurrentTime + duration + 1
			shaman.ActiveTotems[FireTotem] = spell
		},
	}

	return spell
}

// Forever has no Fire Nova Totem: its spell ids are gone from the beta client, and the Fire Nova the spellbook
// teaches in its place is the caster-centred nova, a 10 sec cooldown that bursts on every nearby enemy at once.
// Mana costs and levels are the totem's; damage and the 0.214 coefficient are the beta client's, scaled to level
// 60 like Lightning Bolt's. Spell ID, cost, cooldown, school and defense type come from the client table. Damage
// and coefficient stay here: the table's FireNovaTriggered rows (8349...) are Classic's Fire Nova Totem blast,
// 57-436 at 0.1/0.143, not the nova these ids cast.
const FireNovaRanks = 5

var FireNovaBaseDamage = [FireNovaRanks + 1][]float64{{0, 0}, {51, 60}, {103, 117}, {182, 206}, {280, 316}, {397, 443}}
var FireNovaSpellCoeff = [FireNovaRanks + 1]float64{0, .214, .214, .214, .214, .214}
var FireNovaLevel = [FireNovaRanks + 1]int{0, 12, 22, 32, 42, 52}

func (shaman *Shaman) registerFireNovaSpell() {
	shaman.FireNova = make([]*core.Spell, FireNovaRanks+1)
	cdTimer := shaman.NewTimer()

	for rank := 1; rank <= FireNovaRanks; rank++ {
		config := shaman.newFireNovaSpellConfig(rank, cdTimer)

		if config.RequiredLevel <= int(shaman.Level) {
			shaman.FireNova[rank] = shaman.RegisterSpell(config)
		}
	}
}

func (shaman *Shaman) newFireNovaSpellConfig(rank int, cdTimer *core.Timer) core.SpellConfig {
	row := spellData.FireNova.ByRank(int32(rank))
	baseDamageLow := FireNovaBaseDamage[rank][0]
	baseDamageHigh := FireNovaBaseDamage[rank][1]
	spellCoeff := FireNovaSpellCoeff[rank]
	cooldown := row.Cooldown - shaman.improvedFireNovaCooldownReduction()
	level := FireNovaLevel[rank]

	return core.SpellConfig{
		SpellCode:      SpellCode_ShamanFireNova,
		ClassSpellMask: SpellMaskFireNova,
		ActionID:       core.ActionID{SpellID: row.SpellID},
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskSpellDamage,
		Flags:          SpellFlagShaman | core.SpellFlagAPL,

		RequiredLevel: level,
		Rank:          rank,

		ManaCost: core.ManaCostOptions{
			FlatCost: float64(row.Cost),
		},

		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD: core.GCDDefault,
			},
			CD: core.Cooldown{
				Timer:    cdTimer,
				Duration: cooldown,
			},
		},

		DamageMultiplier: shaman.callOfFlameMultiplier() * shaman.improvedFireNovaMultiplier(),
		ThreatMultiplier: 1,
		BonusCoefficient: spellCoeff,

		ApplyEffects: func(sim *core.Simulation, _ *core.Unit, spell *core.Spell) {
			baseDamage := sim.Roll(baseDamageLow, baseDamageHigh)
			for _, aoeTarget := range sim.Encounter.TargetUnits {
				spell.CalcAndDealDamage(sim, aoeTarget, baseDamage, spell.OutcomeMagicHitAndCrit)
			}
		},
	}
}
