package shared

import (
	"fmt"
	"math"
	"time"

	"github.com/wowsims/classic/sim/core"
)

// A rank's value, by shape. Only a periodic value has a tick schedule, so TickLength and
// NumberOfTicks are reached by asserting to SpellDataPeriodic rather than through a method Flat and
// Range would answer with zeroes.
type SpellDataValue interface {
	// Returns the damage range of the spell.
	// Min/Max are the same if the spell only has a single value.
	Range() (float64, float64)

	// Returns the SP scaling coefficient
	BonusCoefficient() float64

	// Attack Power scaling coefficient
	// Defined manually, DBC does not hold this value - see WithSpellDataAPCoef.
	APBonusCoefficient() float64

	// Returns a static damage value or rolls between the min/max of the range.
	Damage(sim *core.Simulation) float64

	isSpellDataValue()
}

// A single number: a mana restore, a talent's value, damage the client does not roll.
type SpellDataFlat struct {
	Value  float64
	Coef   float64
	APCoef float64
}

// Damage or Healing "Direct/Impact"
type SpellDataRange struct {
	Min    float64
	Max    float64
	Coef   float64
	APCoef float64
}

// DoT
type SpellDataPeriodic struct {
	Tick float64

	// The high end where the client rolls the tick, which one spell in 293 does - Hellfire ticks
	// 307-308. Zero on the rest, meaning Tick is the whole answer.
	TickMax float64

	Coef   float64
	APCoef float64

	// Named for the core.DotConfig fields they feed.
	TickLength    time.Duration
	NumberOfTicks int32

	// The spell the tick was read from when it is not the rank's own: Consecration rank 5 ticks
	// through 1280349, which the client links from nowhere but the tooltip's "$1280349m1". Zero
	// where the rank's own effect states the tick.
	SpellID int32
}

func (v SpellDataFlat) Range() (float64, float64)  { return v.Value, v.Value }
func (v SpellDataRange) Range() (float64, float64) { return v.Min, v.Max }
func (v SpellDataPeriodic) Range() (float64, float64) {
	if v.TickMax > v.Tick {
		return v.Tick, v.TickMax
	}
	return v.Tick, v.Tick
}

func (v SpellDataFlat) Damage(_ *core.Simulation) float64    { return v.Value }
func (v SpellDataRange) Damage(sim *core.Simulation) float64 { return sim.Roll(v.Min, v.Max) }
func (v SpellDataPeriodic) Damage(sim *core.Simulation) float64 {
	if v.TickMax > v.Tick {
		return sim.Roll(v.Tick, v.TickMax)
	}
	return v.Tick
}

func (v SpellDataFlat) BonusCoefficient() float64     { return v.Coef }
func (v SpellDataRange) BonusCoefficient() float64    { return v.Coef }
func (v SpellDataPeriodic) BonusCoefficient() float64 { return v.Coef }

func (v SpellDataFlat) APBonusCoefficient() float64     { return v.APCoef }
func (v SpellDataRange) APBonusCoefficient() float64    { return v.APCoef }
func (v SpellDataPeriodic) APBonusCoefficient() float64 { return v.APCoef }

// Nil reads as zero: Lay on Hands rank 1 restores no mana where ranks 2-4 do.
func SpellDataCoef(v SpellDataValue) float64 {
	if v == nil {
		return 0
	}
	return v.BonusCoefficient()
}

func SpellDataAPCoef(v SpellDataValue) float64 {
	if v == nil {
		return 0
	}
	return v.APBonusCoefficient()
}

func SpellDataMin(v SpellDataValue) float64 {
	if v == nil {
		return 0
	}
	min, _ := v.Range()
	return min
}

func SpellDataMax(v SpellDataValue) float64 {
	if v == nil {
		return 0
	}
	_, max := v.Range()
	return max
}

func (v SpellDataFlat) isSpellDataValue()     {}
func (v SpellDataRange) isSpellDataValue()    {}
func (v SpellDataPeriodic) isSpellDataValue() {}

// One rank of a spell, as the DBC describes it.
type SpellData struct {
	Rank    int32
	SpellID int32
	Cost    int32

	// SpellPower.PowerCostPct: the share of the pool the cast costs, as a percentage of it. Bloodrage
	// reads 20 against health, Arcane Blast 15 against mana. Zero where the cost is Cost alone.
	PowerCostPct float64

	// Cast times differ per rank - Fireball is 1.5s at rank 1 and 3.5s at rank 13 - so a downrank
	// cannot be registered faithfully without them.
	CastTime time.Duration
	GCD      time.Duration
	Cooldown time.Duration
	// The aura or effect the spell leaves, as SpellDuration states it: Shield Wall 12 s, Berserker
	// Rage 10 s. Zero is instant or permanent. A talent proc's aura is usually a triggered spell of
	// its own, so its duration sits on that spell's row.
	Duration time.Duration

	// MinRange gates a cast from too close - the dead zone on a charge - the way MaxRange gates it
	// from too far. Zero means ungated, which is what core reads a zero as.
	MinRange float64
	MaxRange float64

	// Yards per second the projectile travels, which core turns into the delay before the damage
	// lands. Zero is an instant hit.
	MissileSpeed float64

	// The client's proc chance as a percentage: Seal Fate reads 20/40/60/80/100. A 100 means the
	// aura fires on its own condition rather than on a roll, as Flurry's does on a crit, so it is
	// not always the number a ProcTrigger wants.
	ProcChance int32

	// SpellAuraOptions.ProcCharges: Shield Block blocks 2 attacks, Retaliation answers 30. Zero is
	// unlimited.
	ProcCharges int32

	// SpellTargetRestrictions.MaxTargets for an area effect: Whirlwind and Thunder Clap hit 4.
	// Zero is unlimited.
	MaxTargets int32

	// The client's Discount Power On Miss attribute: the server gives 80% of the cost back when
	// the spell misses. Rend and Heroic Strike carry it, Cleave and Whirlwind do not.
	RefundsOnMiss bool

	// The client's Periodic Can Crit attribute: the ticks of the periodic effect roll a critical
	// strike. Rend and Corruption carry it; Deep Wounds does not.
	PeriodicCanCrit bool

	// SpellSchool and DefenseType as core names them. The client's school bits are in a different
	// order - Holy is 2 there and 32 here - so the generator translates rather than copies.
	SpellSchool core.SpellSchool
	DefenseType core.DefenseType
	Direct      SpellDataValue
	Heal        SpellDataValue
	Periodic    SpellDataValue
	Energize    SpellDataValue

	// A second tick the description names after Periodic's. One spell has one: Consecration's is
	// the extra damage its first $s3 targets take.
	SecondaryPeriodic SpellDataValue

	// Every effect the client states, in index order. A role field above holds one each, which is not
	// enough for a talent: Improved Righteous Fury raises threat on one effect and cuts damage taken
	// on another, and only one of them can be Direct.
	Effects []SpellDataEffect

	// Flat threat, before ThreatMultiplier. The client states one only where the ability exists to
	// shed or hold threat - E_THREAT on Feint, Cower, Disengage and Distracting Shot - so everything
	// else arrives through WithSpellDataFlatThreat.
	FlatThreatBonus float64
}

// Declared here rather than in the generated constant file so that an empty or missing one still
// compiles - the generator cannot run if the package it writes into does not build.
type SpellDataEffectKind int32
type SpellDataAura int32

// One effect of a rank, as the client states it, so a caller can name the one it means instead of
// depending on which effect the generator happened to file under a role.
type SpellDataEffect struct {
	Index  int32
	Effect SpellDataEffectKind
	Aura   SpellDataAura

	// What Aura applies it to, and so what it means: for A_ADD_PCT_MODIFIER it selects which part of
	// the spell is modified, for A_MOD_TOTAL_STAT_PERCENTAGE it is a stat, for A_MOD_DAMAGE_DONE a
	// school mask. It stays an int because there is no one enum to name it with.
	Misc int32

	// The client's own number in the client's own units, so a percentage is an integer: Improved
	// Righteous Fury reads 16, not 0.16. Where the effect rolls this is the low end - Arcane Blast
	// rank 1 reads 668 here against Direct's 668-772.
	Value float64

	// The high end, and zero where the two agree, which is 82% of effects. An aura with one die side
	// and a fractional base has two ends a whole number apart and the game shows the higher: Seal of
	// the Crusader rank 1 states 39.2 attack power and buffs for 41, not 40. ValueAt reads Value, so
	// an effect wanting the other end has to say so.
	ValueMax float64

	// SpellEffect.EffectChainAmplitude where it is not the client's default of 1: Execute's 1.5,
	// which its tooltip multiplies by 10 for the damage each extra rage adds. Zero on the rest.
	ChainAmplitude float64
}

// The high end of the effect, which is Value wherever the two agree - ValueMax is only stored where
// they differ. Seal of the Crusader rank 4's base is a whole number, so it has no ValueMax and its
// answer is Value; every other rank has both.
// The share of the cost a miss refunds, for RageCostOptions.Refund: 80% where the client flags
// Discount Power On Miss, nothing otherwise.
func (s SpellData) MissRefund() float64 {
	if s.RefundsOnMiss {
		return 0.8
	}
	return 0
}

// Their PeriodicTickOutcome is left out: our core's tick outcomes and the Forever dot-crit rule
// differ from theirs, so a dot picks its own until that rule is ported (PLAN step 7).

func (e SpellDataEffect) High() float64 {
	if e.ValueMax != 0 {
		return e.ValueMax
	}
	return e.Value
}

// Panics when no effect matches, and when two do - 186 ranked spells carry a duplicate aura/misc
// pair. Index into Effects where the pair cannot tell them apart.
func (r SpellData) Effect(aura SpellDataAura, misc int32) SpellDataEffect {
	found, matches := -1, 0
	for i, e := range r.Effects {
		if e.Aura == aura && e.Misc == misc {
			matches++
			if found < 0 {
				found = i
			}
		}
	}
	if matches > 1 {
		panic(fmt.Sprintf("spell %d rank %d has %d effects with aura %d misc %d - index them instead",
			r.SpellID, r.Rank, matches, aura, misc))
	}
	if found < 0 {
		panic(fmt.Sprintf("spell %d rank %d has no effect with aura %d misc %d, in %d effects",
			r.SpellID, r.Rank, aura, misc, len(r.Effects)))
	}
	return r.Effects[found]
}

func (r SpellData) GetRank() int32 { return r.Rank }

func (r SpellData) GetSpellID() int32 { return r.SpellID }

func (r SpellData) GetRankLabel() string { return fmt.Sprintf("Rank %d", r.Rank) }

type SpellDataRanked interface {
	GetRank() int32
	GetSpellID() int32
}

// A spell's ranks, keyed by rank number but held as a slice.
type SpellDataTableOf[T SpellDataRanked] []T

// Panics on a rank the table does not have. A downrank chosen by typo has to fail loudly rather than
// silently register nothing.
func (t SpellDataTableOf[T]) ByRank(rank int32) T {
	for _, row := range t {
		if row.GetRank() == rank {
			return row
		}
	}
	panic(fmt.Sprintf("no rank %d in table of %d ranks", rank, len(t)))
}

func (t SpellDataTableOf[T]) BySpellID(spellID int32) T {
	for _, row := range t {
		if row.GetSpellID() == spellID {
			return row
		}
	}
	panic(fmt.Sprintf("no spell %d in table of %d ranks", spellID, len(t)))
}

// The highest rank present in the DATA, which is not always the last element -
// Flamestrike is declared rank 7 then 6 - and is not necessarily a rank the game grants. See BySpellID.
func (t SpellDataTableOf[T]) HighestRank() T {
	if len(t) == 0 {
		panic("HighestRank() on an empty rank table")
	}

	best := t[0]
	for _, row := range t[1:] {
		if row.GetRank() > best.GetRank() {
			best = row
		}
	}
	return best
}

func (t SpellDataTableOf[T]) Ranks(ranks ...int32) SpellDataTableOf[T] {
	out := make(SpellDataTableOf[T], 0, len(ranks))
	for _, rank := range ranks {
		out = append(out, t.ByRank(rank))
	}
	return out
}

func (t SpellDataTableOf[T]) RegisterAll(factory func(T)) {
	for _, row := range t {
		factory(row)
	}
}

type SpellDataTable = SpellDataTableOf[SpellData]

// Threat, the same way and for the same reason: the client states one only on the abilities whose
// point is threat, through E_THREAT, and the generator fills those 22 rows. Both forms return a copy
// and panic if the table already carries a value.
func WithSpellDataFlatThreat(table SpellDataTable, threat float64) SpellDataTable {
	return applyFlatThreat(table, func(SpellData) float64 { return threat })
}

// Every rank in the table has to be named, so a ladder that gains one fails loudly instead of
// leaving the new rank at zero threat.
func WithSpellDataFlatThreats(table SpellDataTable, threats map[int32]float64) SpellDataTable {
	requireEveryRank(table, threats)
	return applyFlatThreat(table, func(r SpellData) float64 { return threats[r.Rank] })
}

func applyFlatThreat(table SpellDataTable, threatOf func(SpellData) float64) SpellDataTable {
	out := make(SpellDataTable, len(table))
	for i, row := range table {
		out[i] = row
		if row.FlatThreatBonus != 0 {
			panic(fmt.Sprintf("spell %d rank %d already has flat threat %v from the client DB",
				row.SpellID, row.Rank, row.FlatThreatBonus))
		}
		out[i].FlatThreatBonus = threatOf(row)
	}
	return out
}

// Attack power is hand-supplied: one effect in 38357 carries a nonzero BonusCoefficientFromAP, so
// melee spells hardcode theirs (sim/druid/rip.go:52 reads 990 + 0.18*ap). All forms return a copy and
// panic if the table already carries one.
func WithSpellDataAPCoef(table SpellDataTable, coef float64) SpellDataTable {
	return applyAPCoef(table, func(SpellData) float64 { return coef }, apDirect)
}

func WithSpellDataPeriodicAPCoef(table SpellDataTable, coef float64) SpellDataTable {
	return applyAPCoef(table, func(SpellData) float64 { return coef }, apPeriodic)
}

// Every rank in the table has to be named. A ladder that gains a rank then fails loudly instead of
// scaling the new one off nothing, which is the failure a map would otherwise hide.
func WithSpellDataAPCoefs(table SpellDataTable, coefs map[int32]float64) SpellDataTable {
	requireEveryRank(table, coefs)
	return applyAPCoef(table, func(r SpellData) float64 { return coefs[r.Rank] }, apDirect)
}

func WithSpellDataPeriodicAPCoefs(table SpellDataTable, coefs map[int32]float64) SpellDataTable {
	requireEveryRank(table, coefs)
	return applyAPCoef(table, func(r SpellData) float64 { return coefs[r.Rank] }, apPeriodic)
}

type apRole int

const (
	apDirect apRole = iota
	apPeriodic
)

func requireEveryRank(table SpellDataTable, coefs map[int32]float64) {
	for _, row := range table {
		if _, ok := coefs[row.Rank]; !ok {
			panic(fmt.Sprintf("spell %d rank %d has no AP coefficient in a per-rank map of %d",
				row.SpellID, row.Rank, len(coefs)))
		}
	}
	for rank := range coefs {
		found := false
		for _, row := range table {
			if row.Rank == rank {
				found = true
				break
			}
		}
		if !found {
			panic(fmt.Sprintf("AP coefficient given for rank %d, which the table does not have", rank))
		}
	}
}

func applyAPCoef(table SpellDataTable, coefOf func(SpellData) float64, role apRole) SpellDataTable {
	out := make(SpellDataTable, len(table))
	for i, row := range table {
		out[i] = row
		if role == apPeriodic {
			out[i].Periodic = withAPCoef(row.Periodic, coefOf(row), row.SpellID, row.Rank)
		} else {
			out[i].Direct = withAPCoef(row.Direct, coefOf(row), row.SpellID, row.Rank)
		}
	}
	return out
}

func withAPCoef(value SpellDataValue, coef float64, spellID, rank int32) SpellDataValue {
	if value == nil {
		panic(fmt.Sprintf("spell %d rank %d has no value to give an AP coefficient", spellID, rank))
	}
	if existing := value.APBonusCoefficient(); existing != 0 {
		panic(fmt.Sprintf("spell %d rank %d already has AP coefficient %v from the client DB", spellID, rank, existing))
	}

	switch v := value.(type) {
	case SpellDataFlat:
		v.APCoef = coef
		return v
	case SpellDataRange:
		v.APCoef = coef
		return v
	case SpellDataPeriodic:
		v.APCoef = coef
		return v
	}
	panic(fmt.Sprintf("spell %d rank %d has an unknown value shape %T", spellID, rank, value))
}

// A client effect's value for a caster of the given level: base + perLevel for every level past the
// spell's, capped at its max level, floored the way the Forever server floors it (Immolate r1 lands
// for 10 at level 5 from 8 + 0.7/level). The generated tables hold this at level 60 only, which is
// wrong for a rank cast below its cap. The epsilon keeps 0.2*5 from flooring to 0.99...
func LevelScaled(base, perLevel float64, spellLevel, maxLevel, level int32) float64 {
	return math.Floor(base + perLevel*float64(min(level, maxLevel)-spellLevel) + 1e-9)
}
