package shared

import "fmt"

// What a modifier aura applies to, in index order. The client ships no name list, so these were read
// off the talents that use them and then checked against TrinityCore's SpellModOp (3.3.5) and
// cmangos-tbc's (2.4.3): all 23 agree on value, and on name too except 24 and 27, where cmangos says
// SPELL_BONUS_DAMAGE and MULTIPLE_VALUE. Every modifier effect in the generated tables uses one of
// these 23 - the gaps below are values the cores name and TBC never uses.
const (
	SPELLMOD_DAMAGE                = 0  // Fire Power, Piercing Ice, Contagion
	SPELLMOD_DURATION              = 1  // Permafrost, Improved Gouge, Brutal Impact
	SPELLMOD_THREAT                = 2  // Subtlety, Improved Drain Soul
	SPELLMOD_EFFECT1               = 3  // Arcane Potency, Improved Concentration Aura
	SPELLMOD_CHARGES               = 4  // Improved Shield Block, Improved Holy Shield
	SPELLMOD_RANGE                 = 5  // Arctic Reach, Flame Throwing, Grim Reach
	SPELLMOD_RADIUS                = 6  // Arctic Reach, Holy Reach, Booming Voice
	SPELLMOD_CRITICAL_CHANCE       = 7  // Arcane Impact, Improved Flamestrike, Incineration
	SPELLMOD_ALL_EFFECTS           = 8  // Frost Warding, Magic Attunement, Demonic Aegis
	SPELLMOD_NOT_LOSE_CASTING_TIME = 9  // Burning Soul, Fel Concentration, Intensity
	SPELLMOD_CASTING_TIME          = 10 // Improved Fireball, Improved Frostbolt
	SPELLMOD_COOLDOWN              = 11 // Improved Fire Blast, Improved Frost Nova, Ice Floes
	SPELLMOD_EFFECT2               = 12 // Malediction, Mana Feed
	SPELLMOD_COST                  = 14 // Frost Channeling, Cataclysm
	SPELLMOD_CRIT_DAMAGE_BONUS     = 15 // Ice Shards, Ruin, Vengeance
	SPELLMOD_RESIST_MISS_CHANCE    = 16 // Arcane Focus, Elemental Precision, Suppression
	SPELLMOD_CHANCE_OF_SUCCESS     = 18 // Improved Poisons, Improved Nature's Grasp
	SPELLMOD_ACTIVATION_TIME       = 19 // Improved Fire Totems
	SPELLMOD_DOT                   = 22 // Emberstorm, Contagion, Fire Power
	SPELLMOD_EFFECT3               = 23 // Improved Faerie Fire, Savage Fury
	SPELLMOD_BONUS_MULTIPLIER      = 24 // Empowered Arcane Missiles / Fireball / Frostbolt / Corruption
	SPELLMOD_VALUE_MULTIPLIER      = 27 // Improved Mana Shield
	SPELLMOD_RESIST_DISPEL_CHANCE  = 28 // Vile Poisons, Sanctified Seals
)

// Reads the ladder by points spent. Rank 0 is untaken and answers 0, where ByRank would panic.
func (t SpellDataTableOf[T]) ValueAt(rank int32) float64 {
	return ladderValue(t, rank, nil)
}

// The client states a percentage as an integer: 16, not 0.16.
func (t SpellDataTableOf[T]) FractionAt(rank int32) float64 {
	return t.ValueAt(rank) / 100
}

// The sign comes from the data: Improved Righteous Fury states -2/-4/-6, so rank 3 gives 0.94.
func (t SpellDataTableOf[T]) MultiplierAt(rank int32) float64 {
	return 1 + t.FractionAt(rank)
}

// The client's proc chance as a fraction, which is the form a ProcTrigger takes. Rank 0 is untaken
// and answers 0.
func (t SpellDataTableOf[T]) ProcChanceAt(rank int32) float64 {
	return ladderValue(t, rank, func(row SpellData) float64 { return float64(row.ProcChance) }) / 100
}

// For the case Effect cannot serve: two effects sharing an aura and misc value, as Tactical Mastery's
// two threat modifiers do. The index is the client's EffectIndex, not the slice position.
func (t SpellDataTableOf[T]) EffectAt(index int32) SpellDataEffectLadder[T] {
	pick := func(row SpellData) float64 {
		for _, e := range row.Effects {
			if e.Index == index {
				return e.Value
			}
		}
		panic(fmt.Sprintf("spell %d rank %d has no effect at index %d", row.SpellID, row.Rank, index))
	}
	return SpellDataEffectLadder[T]{table: t, pick: pick}
}

func (t SpellDataTableOf[T]) Effect(aura SpellDataAura, misc int32) SpellDataEffectLadder[T] {
	pick := func(row SpellData) float64 { return row.Effect(aura, misc).Value }
	return SpellDataEffectLadder[T]{table: t, pick: pick}
}

type SpellDataEffectLadder[T SpellDataRanked] struct {
	table SpellDataTableOf[T]
	pick  func(SpellData) float64
}

func (l SpellDataEffectLadder[T]) ValueAt(rank int32) float64 {
	return ladderValue(l.table, rank, l.pick)
}

func (l SpellDataEffectLadder[T]) FractionAt(rank int32) float64 {
	return l.ValueAt(rank) / 100
}

func (l SpellDataEffectLadder[T]) MultiplierAt(rank int32) float64 {
	return 1 + l.FractionAt(rank)
}

func ladderValue[T SpellDataRanked](table SpellDataTableOf[T], rank int32, pick func(SpellData) float64) float64 {
	if rank <= 0 {
		return 0
	}

	row, ok := any(table.ByRank(rank)).(SpellData)
	if !ok {
		panic(fmt.Sprintf("rank %d is not a SpellData, so it carries no effects or proc chance", rank))
	}
	if pick != nil {
		return pick(row)
	}

	// Nothing named means nothing to choose between - reading the first of several silently is the
	// bug this shape exists to prevent.
	if len(row.Effects) != 1 {
		panic(fmt.Sprintf("spell %d rank %d has %d effects - name the one you mean with Effect(aura, misc)",
			row.SpellID, rank, len(row.Effects)))
	}
	return row.Effects[0].Value
}
