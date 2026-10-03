// Hunter export: class spell names and the effects Go keeps in closures. Each formula
// mirrors the cited Go file at the pinned revision.
package main

import (
	"fmt"

	"github.com/wowsims/forever/sim/core"
	"github.com/wowsims/forever/sim/core/dbcenums"
	"github.com/wowsims/forever/sim/core/proto"
	"github.com/wowsims/forever/sim/core/spelldata"
	"github.com/wowsims/forever/sim/hunter"
)

func init() {
	classExports[proto.Class_ClassHunter] = classExport{
		spells: hunterClassSpells, effects: hunterEffects, unrepresented: hunterUnrepresented,
		statAuras: func(core.Agent, *core.Character) []string {
			return []string{"Aspect of the Hawk", "Aspect of the Beast"}
		},
	}
}

// Hunter class masks are Go-internal bit positions (sim/hunter/hunter.go). Export the stable class
// spell name instead of the bit so a Go reordering cannot silently change Rust behavior.
var hunterClassSpells = []classSpellName{
	{hunter.SpellMaskSpellRanged, "spell_ranged"}, {hunter.HunterSpellAutoShot, "auto_shot"},
	{hunter.HunterSpellAimedShot, "aimed_shot"}, {hunter.HunterSpellArcaneShot, "arcane_shot"},
	{hunter.HunterSpellAspectOfTheBeast, "aspect_of_the_beast"}, {hunter.HunterSpellAspectOfTheHawk, "aspect_of_the_hawk"},
	{hunter.HunterSpellAspectOfTheViper, "aspect_of_the_viper"}, {hunter.HunterSpellBestialWrath, "bestial_wrath"},
	{hunter.HunterSpellMultiShot, "multi_shot"}, {hunter.HunterSpellRapidFire, "rapid_fire"},
	{hunter.HunterSpellRaptorStrike, "raptor_strike"}, {hunter.HunterSpellRaptorStrikeQueue, "raptor_strike_queue"},
	{hunter.HunterSpellReadiness, "readiness"}, {hunter.HunterSpellScorpidSting, "scorpid_sting"},
	{hunter.HunterSpellSerpentSting, "serpent_sting"}, {hunter.HunterSpellSteadyShot, "steady_shot"},
	{hunter.HunterSpellVolley, "volley"}, {hunter.HunterPetDamage, "pet_damage"},
	{hunter.HunterSpellExplosiveTrap, "explosive_trap"}, {hunter.HunterSpellFreezingTrap, "freezing_trap"},
	{hunter.HunterSpellImmolationTrap, "immolation_trap"}, {hunter.HunterSpellLaceratingStrikes, "lacerating_strikes"},
	{hunter.HunterSpellMongooseBite, "mongoose_bite"}, {hunter.HunterSpellSniperShot, "sniper_shot"},
	{hunter.HunterSpellStriderKick, "strider_kick"}, {hunter.HunterSpellSummonHawk, "summon_hawk"},
	{hunter.HunterSpellWingClip, "wing_clip"},
}

// The client rows sim/hunter reads. spellData is private to the package, so the ladders are
// restated with the ids of sim/hunter/spell_data_auto_gen.go at the pinned revision.
var (
	hunterAimedShot        = spelldata.Ranked(19434, 20900, 20901, 20902, 20903, 20904)
	hunterSniperShot       = spelldata.Ranked(1310687, 1310785, 1310786)
	hunterSerpentSting     = spelldata.Ranked(1978, 13549, 13550, 13551, 13552, 13553, 13554, 13555, 25295)
	hunterAspectOfTheHawk  = spelldata.Ranked(13165, 14318, 14319, 14320, 14321, 14322, 25296)
	hunterQuickShots       = spelldata.Ranked(6150)
	hunterDeadlyAspects    = spelldata.Talent(19552, 5)
	hunterRapidFire        = spelldata.Ranked(3045)
	hunterSummonHawk       = spelldata.Ranked(1293241, 1293525, 1293526, 1293527)
	hunterSummonHawkSummon = spelldata.Ranked(1293248, 1312639)
)

// spelldata.Spell.TickOutcome's choice for a dot rolled at its hit: a crit only where the row
// states Periodic Can Crit, on the crit of the row's defense type.
func hunterTickOutcome(row *spelldata.Spell) string {
	magic := row.DefenseTypeCore() == core.DefenseTypeMagic
	switch {
	case row.PeriodicCanCrit() && magic:
		return "magic_crit"
	case row.PeriodicCanCrit():
		return "physical_crit"
	case magic:
		return "magic_hit"
	default:
		return "plain"
	}
}

func hunterSpellPosition(character *core.Character, spell *core.Spell) int {
	for i, candidate := range character.Spellbook {
		if candidate == spell {
			return i
		}
	}
	fail(fmt.Errorf("spell %s is not in the spellbook", spell.ActionID))
	return -1
}

func hunterEffects(agent core.Agent, character *core.Character) []map[string]any {
	h := agent.(hunter.HunterAgent).GetHunter()
	talents := h.Talents
	effects := []map[string]any{}

	// aimed_shot.go and sniper_shot.go: a normalized ranged weapon shot plus the rank's flat bonus,
	// rolled on the ranged hit table and dealt after travel.
	if h.AimedShot != nil {
		effects = append(effects, map[string]any{
			"kind": "aimed_shot", "spell_id": h.AimedShot.ActionID.SpellID,
			"flat_bonus": hunterAimedShot.Highest().DamageEffect().Average(core.CharacterLevel),
		})
	}
	if h.SniperShot != nil {
		effects = append(effects, map[string]any{
			"kind": "sniper_shot", "spell_id": h.SniperShot.ActionID.SpellID,
			"flat_bonus": hunterSniperShot.Highest().DamageEffect().Average(core.CharacterLevel),
		})
	}
	// multi_shot.go: one normalized shot per target, at most three; one target in scope.
	if h.MultiShot != nil {
		effects = append(effects, map[string]any{"kind": "multi_shot", "spell_id": h.MultiShot.ActionID.SpellID})
	}
	// serpent_sting.go: a ranged hit roll, then after travel a dot whose ticks add a share of
	// ranged attack power, a Go literal.
	if h.SerpentSting != nil {
		rank := hunterSerpentSting.Highest()
		effects = append(effects, map[string]any{
			"kind": "serpent_sting", "spell_id": h.SerpentSting.ActionID.SpellID,
			"tick_base": rank.PeriodicEffect().Average(core.CharacterLevel), "attack_power_share": 0.035,
			"tick_outcome": hunterTickOutcome(rank),
		})
	}
	// aspects.go: Aspect of the Hawk's ranged attack power is a stat aura; Deadly Aspects procs
	// Quick Shots on ranged autos.
	if aura := h.AspectOfTheHawkAura; aura != nil && h.AspectOfTheHawk != nil {
		effect := map[string]any{"kind": "aspect_of_the_hawk", "spell_id": h.AspectOfTheHawk.ActionID.SpellID, "aura": aura.Label}
		if talents.DeadlyAspects > 0 {
			quickShots := hunterQuickShots.Highest()
			effect["proc_aura"] = character.GetAura("Quick Shots").Label
			effect["haste_multiplier"] = 1 + quickShots.Effect(dbcenums.A_MOD_RANGED_HASTE, 0).Average(core.CharacterLevel)/100
			effect["proc_chance"] = hunterDeadlyAspects.EffectAt(1).FractionAt(talents.DeadlyAspects)
		}
		effects = append(effects, effect)
	}
	// rapid_fire.go: ranged and melee attack speed.
	if aura := h.RapidFireAura; aura != nil && h.RapidFire != nil {
		rank := hunterRapidFire.Highest()
		effects = append(effects, map[string]any{
			"kind": "rapid_fire", "spell_id": h.RapidFire.ActionID.SpellID, "aura": aura.Label,
			"haste_multiplier": 1 + rank.Effect(dbcenums.A_MOD_RANGED_HASTE, 0).Average(core.CharacterLevel)/100,
		})
	}
	// summon_hawk.go: a dive bomb on its rank's base plus a share of ranged attack power, a Go
	// literal, then a hawk dot in a free slot or the one with the least time left.
	if h.SummonHawk != nil {
		rank := hunterSummonHawk.Highest()
		hawks := []int{}
		for _, spell := range character.Spellbook {
			if spell.ActionID.SpellID == rank.ID && spell.ActionID.Tag > 0 {
				hawks = append(hawks, hunterSpellPosition(character, spell))
			}
		}
		if len(hawks) != int(rank.EffectN(3).BasePoints) {
			fail(fmt.Errorf("Summon Hawk registered %d hawks", len(hawks)))
		}
		effects = append(effects, map[string]any{
			"kind": "summon_hawk", "spell_id": rank.ID, "base_damage": rank.DamageEffect().Average(core.CharacterLevel),
			"attack_power_share": 0.05, "always_hits": rank.AlwaysHits(), "hawk_spells": hawks,
			"hawk_duration_ns": nanos(hunterSummonHawkSummon.ByID(1293248).Duration()),
		})
	}
	// talents_survival.go Defensive State: the trigger hears only attacks the player dodges.
	if character.GetAura("Defensive State - Trigger") != nil {
		effects = append(effects, map[string]any{
			"kind": "inert_listener", "unit": "player", "aura": "Defensive State - Trigger",
			"reason": "acts only on attacks the player dodges, and nothing attacks the player",
		})
	}
	return effects
}

// Hunter behavior the effects cannot describe.
func hunterUnrepresented(agent core.Agent, character *core.Character) []string {
	h := agent.(hunter.HunterAgent).GetHunter()
	notes := []string{}
	if h.Pet != nil {
		notes = append(notes, "hunter pets are unsupported")
	}
	if h.Talents.RapidRecuperation > 0 {
		notes = append(notes, "Rapid Recuperation is unsupported")
	}
	// summon_hawk.go deals periodic physical damage, which this multiplier would scale.
	if h.SummonHawk != nil && character.CurrentTarget.PseudoStats.PeriodicPhysicalDamageTakenMultiplier != 1 {
		notes = append(notes, "periodic physical damage taken multipliers are unsupported")
	}
	return notes
}
