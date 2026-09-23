package core

import (
	"github.com/wowsims/classic/sim/core/proto"
	"github.com/wowsims/classic/sim/core/stats"
)

// Simulation embeds Environment, so sim.IsForever() resolves here too.
func (env *Environment) IsForever() bool {
	return env.Ruleset == proto.Ruleset_RulesetForever
}

// Periodic damage rolls for crits under the Forever ruleset. Spells that
// should keep ticking for flat damage opt out with SpellFlagNoPeriodicCrit.
func (dot *Dot) canCrit(sim *Simulation) bool {
	return sim.IsForever() && !dot.Spell.Flags.Matches(SpellFlagNoPeriodicCrit)
}

// Ticks roll against the caster's crit chance at the time of the tick instead of the
// chance snapshotted when the dot went up. Dots that are applied by hand, like Deep
// Wounds, never snapshot one at all, so rolling live is also the only way for them to
// crit at the right rate. The crit table follows the spell's defense type, not its school:
// Serpent Sting is Nature but the client files it Ranged (SpellCategories.DefenseType 3),
// so its ticks roll ranged crit, suppression and all, like the shot that applied it.
func (dot *Dot) critCheck(sim *Simulation, target *Unit, attackTable *AttackTable) bool {
	if dot.Spell.SchoolIndex == stats.SchoolIndexPhysical || dot.Spell.DefenseType == DefenseTypeMelee || dot.Spell.DefenseType == DefenseTypeRanged {
		return dot.Spell.PhysicalCritCheck(sim, attackTable)
	}
	return dot.Spell.MagicCritCheck(sim, target)
}

// Bonus healing on Forever gear carries a damage component with it, so that healing
// gear is not dead weight outside a raid. Hide of the Wild reads 42 healing and 14
// damage, which is the only published pair, so a third is the rate used here. It feeds
// SpellDamage rather than SpellPower because the damage half does not heal.
const ForeverHealingToSpellDamage = 1.0 / 3.0

func (character *Character) addHealingSpellDamage(equipStats stats.Stats) stats.Stats {
	equipStats[stats.SpellDamage] += equipStats[stats.HealingPower] * ForeverHealingToSpellDamage
	return equipStats
}

// Forever pays out hit and critical strike from gear against every kind of attack
// rather than splitting them into a melee and a spell pool. Attribute conversions are
// untouched: only the hit and crit an item spells out become universal.
//
// The database already pays Forever's generic hit and crit rating into both pools (a
// Classic ruleset needs that), so an item, suffix or enchant that carries it in both is
// counted once here: Bloodvine Vest's "+2% hit" is 2%, not 4%.
func (character *Character) unifyEquipHitAndCrit(equipStats stats.Stats) stats.Stats {
	hit := equipStats[stats.MeleeHit] + equipStats[stats.SpellHit]
	crit := equipStats[stats.MeleeCrit] + equipStats[stats.SpellCrit]
	for _, item := range character.Equipment {
		for _, s := range []stats.Stats{item.Stats, item.RandomSuffix.Stats, item.Enchant.Stats} {
			hit -= min(s[stats.MeleeHit], s[stats.SpellHit])
			crit -= min(s[stats.MeleeCrit], s[stats.SpellCrit])
		}
	}

	equipStats[stats.MeleeHit] = hit
	equipStats[stats.SpellHit] = hit
	equipStats[stats.MeleeCrit] = crit
	equipStats[stats.SpellCrit] = crit

	return equipStats
}

// Rage from a landed auto attack is flat on Forever: set by the weapon's speed and nothing
// else. Classic pays 7.5 x damage / conversion, so a crit is worth double a normal hit and a
// geared warrior is worth several times a levelling one; on Forever a swing is a swing.
//
// Measured from public beta combat logs by BrawnyBravo (issue #252): 63 clean pairs across
// nine warriors at levels 10-15, taking only consecutive auto-attack snapshots 0-4s apart
// with no ability used and no damage taken between them. Damage in the sample ran from 15 to
// 64 a hit, crits included, with no effect on the rage gained.
//
//	~2.1s one-hand   7.2-7.3      2.1 x 3.46 = 7.27
//	~2.5s one-hand   8.6-8.7      2.5 x 3.46 = 8.65
//	~3.2s two-hand  14.4          3.2 x 4.5  = 14.4
//	~3.3s two-hand  14.9          3.3 x 4.5  = 14.85
//	~3.5s two-hand  15.7          3.5 x 4.5  = 15.75
//
// wowsims/forever f9f9f21883 ("Rage follows the Forever combat logs") pins the one-hand factor
// to 3.46 from six more level 8-9 logs; it lands on the middle of both one-hand buckets above,
// where 3.5 sat 1.5% high. An off-hand swing pays half, before any off-hand multiplier (same
// commit).
const (
	ForeverRagePerSecondOneHand = 3.46
	ForeverRagePerSecondTwoHand = 4.5
	ForeverOffHandRageFactor    = 0.5
)

// Rage from a hit taken on Forever: its pre-armor damage x 10 / maximum health
// (wowsims/forever f9f9f21883). They fit the 10 to geared logs and flag it for an in-game
// test at higher levels (naked logs gave double) - re-measure it first if tank rage looks off.
const ForeverDamageTakenRageFactor = 10.0

// What a landed swing is worth. Base weapon speed, not the hasted interval: otherwise haste
// would buy swings and lose exactly as much rage per swing, which would make it rage-neutral.
// The sample was taken at a level where nobody had haste, so it cannot tell the two apart -
// this is the assumption, and it is the one worth re-measuring first.
func ForeverWhiteHitRage(weapon *Weapon) float64 {
	if weapon == nil || weapon.SwingSpeed == 0 {
		return 0
	}
	if weapon.TwoHand {
		return weapon.SwingSpeed * ForeverRagePerSecondTwoHand
	}
	return weapon.SwingSpeed * ForeverRagePerSecondOneHand
}

func ForeverDamageTakenRage(preArmorDamage, maxHealth float64) float64 {
	if maxHealth <= 0 {
		return 0
	}
	return preArmorDamage * ForeverDamageTakenRageFactor / maxHealth
}
