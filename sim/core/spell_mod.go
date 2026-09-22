package core

import (
	"math"
	"strconv"
	"time"
)

// SpellMod system, taken from wowsims/forever (sim/core/spell_mod.go) and adapted to this engine.
// It lives beside SpellCode: a mod picks its spells by ClassSpellMask, school, defense type,
// proc mask, spell flags and cost type, and applies the change to every matching spell
// registered on the unit, now or later.
//
// Adapted from upstream:
//   - Crit/hit kinds write Spell.BonusCritRating / BonusHitRating, which hold percent in this
//     engine (SpellCritRatingPerCritChance == 1). Values are still given in percent.
//   - CritMultiplier_Flat adds to Spell.CritDamageBonus (our crit bonus field).
//   - PowerCost_Pct_Add adds to SpellCost.Multiplier, our additive integer cost percent.
//   - BonusSpellDamage_Flat adds to Spell.BonusDamage.
//   - The ResourceType filter is our CostType.
//   - BonusHit_Percent may be combined with a School: our hit bonus is per spell, upstream
//     routes school hit through PseudoStats instead.
//   - DamageDone_Flat has no reset-time rounding: upstream rounds DamageMultiplierAdditive to
//     4 decimals every reset, which would move our results by float noise.
//
// Dropped (no matching field in this engine): PowerCost_Pct (multiplicative cost bucket),
// Cooldown_Multiplier, CritMultiplier_Pct, BonusExpertise_Rating (rating), DotNumberOfTicks_Flat
// and DotTickLength_Flat (our dot duration is derived at apply time), DotBaseDuration_Pct,
// DebuffDuration_Flat, BuffDuration_Flat, ModCharges_Flat, BaseDamage_Flat,
// AllowCastWhileMoving/Channeling. Add them when a class needs one.

type SpellModConfig struct {
	ClassMask         int64
	Kind              SpellModType
	School            SpellSchool
	DefenseType       DefenseType // Only apply to spells with a matching DefenseType
	ProcMask          ProcMask
	SpellFlag         SpellFlag
	CostType          CostType
	IntValue          int32
	TimeValue         time.Duration
	FloatValue        float64
	ApplyCustom       SpellModApply
	RemoveCustom      SpellModRemove
	ResetCustom       SpellModOnReset
	ShouldApplyToPets bool
}

type SpellMod struct {
	ClassMask      int64
	Kind           SpellModType
	School         SpellSchool
	DefenseType    DefenseType
	ProcMask       ProcMask
	SpellFlag      SpellFlag
	CostType       CostType
	floatValue     float64
	intValue       int32
	timeValue      time.Duration
	Apply          SpellModApply
	Remove         SpellModRemove
	IsActive       bool
	AffectedSpells []*Spell
	OnReset        SpellModOnReset
}

type SpellModApply func(mod *SpellMod, spell *Spell)
type SpellModRemove func(mod *SpellMod, spell *Spell)
type SpellModOnReset func(mod *SpellMod)

type SpellModFunctions struct {
	Apply   SpellModApply
	Remove  SpellModRemove
	OnReset SpellModOnReset
}

func buildMod(unit *Unit, config SpellModConfig) *SpellMod {
	functions := spellModMap[config.Kind]
	if functions == nil {
		panic("SpellMod " + strconv.Itoa(int(config.Kind)) + " not implemented")
	}

	applyFn, removeFn, resetFn := functions.Apply, functions.Remove, functions.OnReset
	if config.Kind == SpellMod_Custom {
		if config.ApplyCustom == nil || config.RemoveCustom == nil {
			panic("ApplyCustom and RemoveCustom are mandatory fields for SpellMod_Custom")
		}
		applyFn, removeFn, resetFn = config.ApplyCustom, config.RemoveCustom, config.ResetCustom
	}

	mod := &SpellMod{
		ClassMask:   config.ClassMask,
		Kind:        config.Kind,
		School:      config.School,
		DefenseType: config.DefenseType,
		ProcMask:    config.ProcMask,
		SpellFlag:   config.SpellFlag,
		CostType:    config.CostType,
		floatValue:  config.FloatValue,
		intValue:    config.IntValue,
		timeValue:   config.TimeValue,
		Apply:       applyFn,
		Remove:      removeFn,
		OnReset:     resetFn,
	}

	units := []*Unit{unit}
	if config.ShouldApplyToPets {
		for _, pet := range unit.PetAgents {
			units = append(units, &pet.GetPet().Unit)
		}
	}
	for _, u := range units {
		u.OnSpellRegistered(func(spell *Spell) {
			if shouldApply(spell, mod) {
				mod.AffectedSpells = append(mod.AffectedSpells, spell)
				if mod.IsActive {
					mod.Apply(mod, spell)
				}
			}
		})
		if mod.OnReset != nil {
			u.RegisterResetEffect(func(*Simulation) { mod.OnReset(mod) })
		}
	}

	return mod
}

// AddStaticMod adds a mod that is always active.
func (unit *Unit) AddStaticMod(config SpellModConfig) {
	buildMod(unit, config).Activate()
}

// AddDynamicMod adds an inactive mod, to be switched with Activate/Deactivate (e.g. by an aura).
func (unit *Unit) AddDynamicMod(config SpellModConfig) *SpellMod {
	return buildMod(unit, config)
}

func shouldApply(spell *Spell, mod *SpellMod) bool {
	if mod.CostType != CostTypeUnknown && (spell.Cost == nil || spell.Cost.CostType() != mod.CostType) {
		return false
	}
	if mod.ClassMask > 0 && !spell.Matches(mod.ClassMask) {
		return false
	}
	if mod.School > 0 && !mod.School.Matches(spell.SpellSchool) {
		return false
	}
	if mod.DefenseType > 0 && spell.DefenseType != mod.DefenseType {
		return false
	}
	if mod.ProcMask > 0 && !mod.ProcMask.Matches(spell.ProcMask) {
		return false
	}
	if mod.SpellFlag > 0 && !mod.SpellFlag.Matches(spell.Flags) {
		return false
	}
	return true
}

func (mod *SpellMod) UpdateIntValue(value int32) {
	mod.update(func() { mod.intValue = value })
}

func (mod *SpellMod) UpdateTimeValue(value time.Duration) {
	mod.update(func() { mod.timeValue = value })
}

func (mod *SpellMod) UpdateFloatValue(value float64) {
	mod.update(func() { mod.floatValue = value })
}

func (mod *SpellMod) update(set func()) {
	if mod.IsActive {
		mod.Deactivate()
		set()
		mod.Activate()
	} else {
		set()
	}
}

func (mod *SpellMod) GetIntValue() int32 {
	return mod.intValue
}

func (mod *SpellMod) GetFloatValue() float64 {
	return mod.floatValue
}

func (mod *SpellMod) GetTimeValue() time.Duration {
	return mod.timeValue
}

func (mod *SpellMod) Activate() {
	if mod.IsActive {
		return
	}
	for _, spell := range mod.AffectedSpells {
		mod.Apply(mod, spell)
	}
	mod.IsActive = true
}

func (mod *SpellMod) Deactivate() {
	if !mod.IsActive {
		return
	}
	for _, spell := range mod.AffectedSpells {
		mod.Remove(mod, spell)
	}
	mod.IsActive = false
}

type SpellModType uint32

const (
	// Multiplies spell.DamageMultiplier. +5% = 0.05. Uses FloatValue.
	SpellMod_DamageDone_Pct SpellModType = 1 << iota

	// Adds to spell.DamageMultiplierAdditive. Uses FloatValue.
	SpellMod_DamageDone_Flat

	// Adds to spell.Cost.Multiplier, our additive cost percent. +75% = 0.75, -5% = -0.05.
	// Rounded to whole percent, which is what SpellCost.Multiplier holds. Uses FloatValue.
	SpellMod_PowerCost_Pct_Add

	// Adds to spell.Cost.FlatModifier. -5 Mana = -5. Uses IntValue.
	SpellMod_PowerCost_Flat

	// Adds to spell.CD.Duration. Uses TimeValue.
	SpellMod_Cooldown_Flat

	// Adds to spell.CritDamageBonus. +100% = 1.0. Uses FloatValue.
	SpellMod_CritMultiplier_Flat

	// Adds to spell.CastTimeMultiplier. -25% = -0.25. Uses FloatValue.
	SpellMod_CastTime_Pct

	// Adds to spell.DefaultCast.CastTime. Uses TimeValue.
	SpellMod_CastTime_Flat

	// Adds bonus crit, in percent. Uses FloatValue.
	SpellMod_BonusCrit_Percent

	// Adds bonus hit, in percent. Uses FloatValue.
	SpellMod_BonusHit_Percent

	// Adds to spell.DefaultCast.GCD. Uses TimeValue.
	SpellMod_GlobalCooldown_Flat

	// Adds to spell.BonusCoefficient. Uses FloatValue.
	SpellMod_BonusCoeffecient_Flat

	// Adds to spell.BonusDamage. Uses FloatValue.
	SpellMod_BonusSpellDamage_Flat

	// User-defined. Uses ApplyCustom, RemoveCustom and optionally ResetCustom.
	SpellMod_Custom

	// Multiplies the periodic DamageMultiplier of the spell's dots. +5% = 0.05. Uses FloatValue.
	SpellMod_DotDamageDone_Pct

	// Adds to the BonusCoefficient of the spell's dots. Uses FloatValue.
	SpellMod_DotBonusCoeffecient_Flat

	// Multiplies spell.ThreatMultiplier. -15% = -0.15. Uses FloatValue.
	SpellMod_ThreatMultiplier_Pct

	// Adds to spell.PeriodicDamageMultiplierAdditive. Uses FloatValue.
	SpellMod_PeriodicDamageDone_Flat

	// Adds to spell.BaseDamageMultiplierAdditive. Uses FloatValue.
	SpellMod_BaseDamageDone_Flat
)

var spellModMap = map[SpellModType]*SpellModFunctions{
	SpellMod_DamageDone_Pct: {
		Apply:  func(mod *SpellMod, spell *Spell) { spell.DamageMultiplier *= 1 + mod.floatValue },
		Remove: func(mod *SpellMod, spell *Spell) { spell.DamageMultiplier /= 1 + mod.floatValue },
	},
	SpellMod_DamageDone_Flat: {
		Apply:  func(mod *SpellMod, spell *Spell) { spell.DamageMultiplierAdditive += mod.floatValue },
		Remove: func(mod *SpellMod, spell *Spell) { spell.DamageMultiplierAdditive -= mod.floatValue },
	},
	SpellMod_PowerCost_Pct_Add: {
		Apply: func(mod *SpellMod, spell *Spell) {
			if spell.Cost != nil {
				spell.Cost.Multiplier += int32(math.Round(mod.floatValue * 100))
			}
		},
		Remove: func(mod *SpellMod, spell *Spell) {
			if spell.Cost != nil {
				spell.Cost.Multiplier -= int32(math.Round(mod.floatValue * 100))
			}
		},
	},
	SpellMod_PowerCost_Flat: {
		Apply: func(mod *SpellMod, spell *Spell) {
			if spell.Cost != nil {
				spell.Cost.FlatModifier += mod.intValue
			}
		},
		Remove: func(mod *SpellMod, spell *Spell) {
			if spell.Cost != nil {
				spell.Cost.FlatModifier -= mod.intValue
			}
		},
	},
	SpellMod_Cooldown_Flat: {
		Apply:  func(mod *SpellMod, spell *Spell) { spell.CD.Duration += mod.timeValue },
		Remove: func(mod *SpellMod, spell *Spell) { spell.CD.Duration -= mod.timeValue },
	},
	SpellMod_CritMultiplier_Flat: {
		Apply:  func(mod *SpellMod, spell *Spell) { spell.CritDamageBonus += mod.floatValue },
		Remove: func(mod *SpellMod, spell *Spell) { spell.CritDamageBonus -= mod.floatValue },
	},
	SpellMod_CastTime_Pct: {
		Apply:  func(mod *SpellMod, spell *Spell) { spell.CastTimeMultiplier += mod.floatValue },
		Remove: func(mod *SpellMod, spell *Spell) { spell.CastTimeMultiplier -= mod.floatValue },
	},
	SpellMod_CastTime_Flat: {
		Apply:  func(mod *SpellMod, spell *Spell) { spell.DefaultCast.CastTime += mod.timeValue },
		Remove: func(mod *SpellMod, spell *Spell) { spell.DefaultCast.CastTime -= mod.timeValue },
	},
	SpellMod_BonusCrit_Percent: {
		Apply: func(mod *SpellMod, spell *Spell) {
			spell.BonusCritRating += mod.floatValue * SpellCritRatingPerCritChance
		},
		Remove: func(mod *SpellMod, spell *Spell) {
			spell.BonusCritRating -= mod.floatValue * SpellCritRatingPerCritChance
		},
	},
	SpellMod_BonusHit_Percent: {
		Apply:  func(mod *SpellMod, spell *Spell) { spell.BonusHitRating += mod.floatValue * SpellHitRatingPerHitChance },
		Remove: func(mod *SpellMod, spell *Spell) { spell.BonusHitRating -= mod.floatValue * SpellHitRatingPerHitChance },
	},
	SpellMod_GlobalCooldown_Flat: {
		Apply:  func(mod *SpellMod, spell *Spell) { spell.DefaultCast.GCD += mod.timeValue },
		Remove: func(mod *SpellMod, spell *Spell) { spell.DefaultCast.GCD -= mod.timeValue },
	},
	SpellMod_BonusCoeffecient_Flat: {
		Apply:  func(mod *SpellMod, spell *Spell) { spell.BonusCoefficient += mod.floatValue },
		Remove: func(mod *SpellMod, spell *Spell) { spell.BonusCoefficient -= mod.floatValue },
	},
	SpellMod_BonusSpellDamage_Flat: {
		Apply:  func(mod *SpellMod, spell *Spell) { spell.BonusDamage += mod.floatValue },
		Remove: func(mod *SpellMod, spell *Spell) { spell.BonusDamage -= mod.floatValue },
	},
	SpellMod_Custom: {
		// ApplyCustom/RemoveCustom are wired in buildMod().
	},
	SpellMod_DotDamageDone_Pct: {
		Apply: func(mod *SpellMod, spell *Spell) {
			eachDot(spell, func(dot *Dot) { dot.DamageMultiplier *= 1 + mod.floatValue })
		},
		Remove: func(mod *SpellMod, spell *Spell) {
			eachDot(spell, func(dot *Dot) { dot.DamageMultiplier /= 1 + mod.floatValue })
		},
	},
	SpellMod_DotBonusCoeffecient_Flat: {
		Apply: func(mod *SpellMod, spell *Spell) {
			eachDot(spell, func(dot *Dot) { dot.BonusCoefficient += mod.floatValue })
		},
		Remove: func(mod *SpellMod, spell *Spell) {
			eachDot(spell, func(dot *Dot) { dot.BonusCoefficient -= mod.floatValue })
		},
	},
	SpellMod_ThreatMultiplier_Pct: {
		Apply:  func(mod *SpellMod, spell *Spell) { spell.ThreatMultiplier *= 1 + mod.floatValue },
		Remove: func(mod *SpellMod, spell *Spell) { spell.ThreatMultiplier /= 1 + mod.floatValue },
	},
	SpellMod_PeriodicDamageDone_Flat: {
		Apply:  func(mod *SpellMod, spell *Spell) { spell.PeriodicDamageMultiplierAdditive += mod.floatValue },
		Remove: func(mod *SpellMod, spell *Spell) { spell.PeriodicDamageMultiplierAdditive -= mod.floatValue },
	},
	SpellMod_BaseDamageDone_Flat: {
		Apply:  func(mod *SpellMod, spell *Spell) { spell.BaseDamageMultiplierAdditive += mod.floatValue },
		Remove: func(mod *SpellMod, spell *Spell) { spell.BaseDamageMultiplierAdditive -= mod.floatValue },
	},
}

func eachDot(spell *Spell, f func(dot *Dot)) {
	for _, dot := range spell.dots {
		if dot != nil {
			f(dot)
		}
	}
	if spell.aoeDot != nil {
		f(spell.aoeDot)
	}
}
