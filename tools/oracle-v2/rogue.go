// Rogue export: class spell names and the effects Go keeps in closures. Each formula mirrors
// the cited file of sim/rogue at the pinned revision.
package main

import (
	"time"

	"github.com/wowsims/forever/sim/core"
	"github.com/wowsims/forever/sim/core/dbcenums"
	"github.com/wowsims/forever/sim/core/proto"
	"github.com/wowsims/forever/sim/core/spelldata"
	"github.com/wowsims/forever/sim/rogue"
)

func init() {
	classExports[proto.Class_ClassRogue] = classExport{
		spells: rogueClassSpells, effects: rogueEffects, unrepresented: rogueUnrepresented,
	}
}

// sim/rogue/rogue.go class masks, by stable name.
var rogueClassSpells = []classSpellName{
	{rogue.RogueSpellAmbush, "ambush"}, {rogue.RogueSpellBackstab, "backstab"},
	{rogue.RogueSpellEviscerate, "eviscerate"}, {rogue.RogueSpellExposeArmor, "expose_armor"},
	{rogue.RogueSpellFeint, "feint"}, {rogue.RogueSpellGarrote, "garrote"},
	{rogue.RogueSpellGouge, "gouge"}, {rogue.RogueSpellRupture, "rupture"},
	{rogue.RogueSpellSinisterStrike, "sinister_strike"}, {rogue.RogueSpellSliceAndDice, "slice_and_dice"},
	{rogue.RogueSpellStealth, "stealth"}, {rogue.RogueSpellVanish, "vanish"},
	{rogue.RogueSpellHemorrhage, "hemorrhage"}, {rogue.RogueSpellPremeditation, "premeditation"},
	{rogue.RogueSpellPreparation, "preparation"}, {rogue.RogueSpellShadowstep, "shadowstep"},
	{rogue.RogueSpellAdrenalineRush, "adrenaline_rush"}, {rogue.RogueSpellBladeFlurry, "blade_flurry"},
	{rogue.RogueSpellColdBlood, "cold_blood"}, {rogue.RogueSpellMutilate, "mutilate"},
	{rogue.RogueSpellMutilateHit, "mutilate_hit"}, {rogue.RogueSpellGhostlyStrike, "ghostly_strike"},
	{rogue.RogueSpellInstantPoison, "instant_poison"}, {rogue.RogueSpellWoundPoison, "wound_poison"},
	{rogue.RogueSpellDeadlyPoison, "deadly_poison"}, {rogue.RogueSpellVenom, "venom"},
	{rogue.RogueSpellRiposte, "riposte"}, {rogue.RogueSpellKidneyShot, "kidney_shot"},
}

// The client rows sim/rogue reads. spellData is private to the package, so the ladders are
// restated with the ids of sim/rogue/spell_data_auto_gen.go at the pinned revision.
var (
	rogueAdrenalineRush            = spelldata.Ranked(13750)
	rogueBackstab                  = spelldata.Ranked(53, 2589, 2590, 2591, 8721, 11279, 11280, 11281, 25300)
	rogueBladeFlurry               = spelldata.Ranked(13877)
	rogueEviscerate                = spelldata.Ranked(2098, 6760, 6761, 6762, 8623, 8624, 11299, 11300, 31016)
	rogueImprovedPoisons           = spelldata.Talent(14113, 5)
	rogueImprovedSliceAndDice      = spelldata.Talent(14165, 3)
	roguePuncturingWounds          = spelldata.Talent(1224716, 3)
	roguePuncturingWoundsTriggered = spelldata.Ranked(1310710)
	rogueSinisterStrike            = spelldata.Ranked(1752, 1757, 1758, 1759, 1760, 8621, 11293, 11294)
)

// poisons.go: the imbue ids and Go literals.
const (
	rogueInstantImbueID = 26891
	rogueDeadlyImbueID  = 27186
)

func rogueEffects(agent core.Agent, character *core.Character) []map[string]any {
	r := agent.(rogue.RogueAgent).GetRogue()
	talents := r.Talents
	effects := []map[string]any{}

	// sinister_strike.go: the highest rank's base plus normalized main hand damage.
	if r.SinisterStrike != nil {
		row := rogueSinisterStrike.Highest()
		effects = append(effects, map[string]any{
			"kind": "sinister_strike", "spell_id": row.ID, "base_damage": row.DamageEffect().Average(core.CharacterLevel),
		})
	}
	// backstab.go: the base, the dagger and behind conditions, and Puncturing Wounds' combo point.
	if r.Backstab != nil {
		row := rogueBackstab.Highest()
		effects = append(effects, map[string]any{
			"kind": "backstab", "spell_id": row.ID, "base_damage": row.DamageEffect().Average(core.CharacterLevel),
			"main_hand_dagger":         r.HasDagger(core.MainHand),
			"extra_combo_point_chance": roguePuncturingWounds.EffectAt(2).ValueAt(talents.PuncturingWounds) / 100,
			"extra_combo_point_action": actionID(core.ActionID{SpellID: roguePuncturingWoundsTriggered.Highest().ID}),
		})
	}
	// eviscerate.go: the rolled base, the bonus a combo point, and 3% of attack power a point,
	// a Go literal.
	if r.Eviscerate != nil {
		row := rogueEviscerate.Highest()
		damage := row.DamageEffect()
		effects = append(effects, map[string]any{
			"kind": "eviscerate", "spell_id": row.ID, "damage_average": damage.Average(core.CharacterLevel),
			"damage_variance":              damage.Variance,
			"combo_point_damage":           float64(damage.PointsPerResource) + r.DeathmantleBonus,
			"attack_power_per_combo_point": 0.03,
		})
	}
	// slice_and_dice.go: the duration at each combo point count and the attack speed bonus.
	if r.SliceAndDice != nil {
		ladder := privateField(r, "sliceAndDiceDurations")
		durations := []int64{}
		for points := 0; points < ladder.Len(); points++ {
			duration := time.Duration(ladder.Index(points).Int())
			durations = append(durations, nanos(time.Duration(float64(duration+r.SliceAndDiceBonusDuration)*
				rogueImprovedSliceAndDice.MultiplierAt(talents.ImprovedSliceAndDice))))
		}
		effects = append(effects, map[string]any{
			"kind": "slice_and_dice", "spell_id": r.SliceAndDice.ActionID.SpellID, "aura": r.SliceAndDiceAura.Label,
			"durations_ns": durations, "melee_speed_multiplier": 1 + r.SliceAndDiceBonusFlat,
		})
	}
	// talents_combat.go registerBladeFlurry: the attack speed it attaches. Its extra hit needs a
	// second target.
	if r.BladeFlurry != nil {
		row := rogueBladeFlurry.Highest()
		effects = append(effects, map[string]any{
			"kind": "blade_flurry", "spell_id": row.ID, "aura": r.BladeFlurryAura.Label,
			"attack_speed_multiplier": 1 + row.Effect(dbcenums.A_MOD_MELEE_HASTE_3, 0).Average(core.CharacterLevel)/100,
		})
	}
	// talents_combat.go registerAdrenalineRush: the regeneration multiplier and the energy at or
	// below which the major cooldown fires, a Go literal.
	if r.AdrenalineRush != nil {
		row := rogueAdrenalineRush.Highest()
		effects = append(effects, map[string]any{
			"kind": "adrenaline_rush", "spell_id": row.ID, "aura": r.AdrenalineRushAura.Label,
			"regen_multiplier": 1 + row.Effect(dbcenums.A_MOD_POWER_REGEN_PERCENT, 3).Average(core.CharacterLevel)/100,
			"energy_threshold": 45.0,
		})
	}
	// rogue.go ApplyFinisher: Relentless Strikes' 20% a combo point for 25 energy, Go literals,
	// and Ruthlessness's chance.
	effects = append(effects, map[string]any{
		"kind": "rogue_finisher", "relentless_strikes": talents.RelentlessStrikes,
		"relentless_strikes_chance_per_point": 0.2, "relentless_strikes_energy": 25.0,
		"relentless_strikes_action": actionID(core.ActionID{SpellID: 14179}),
		"ruthlessness_chance":       privateField(r, "ruthlessnessChance").Float(),
		"ruthlessness_action":       actionID(core.ActionID{SpellID: 14161}),
	})
	// poisons.go: each imbue's weapon proc rolls its chance, raised by Improved Poisons, inside
	// the handler.
	bonus := rogueImprovedPoisons.Effect(dbcenums.A_ADD_FLAT_MODIFIER, int32(dbcenums.SPELLMOD_CHANCE_OF_SUCCESS)).FractionAt(talents.ImprovedPoisons)
	hands := func(imbue int32) []string {
		var mask core.ProcMask
		if r.Consumables.MhImbueId == imbue {
			mask |= core.ProcMaskMeleeMH
		}
		if r.Consumables.OhImbueId == imbue {
			mask |= core.ProcMaskMeleeOH
		}
		return procMaskNames(mask)
	}
	if r.InstantPoison != nil {
		effects = append(effects, map[string]any{
			"kind": "instant_poison", "trigger_aura": "Instant Poison", "spell_id": r.InstantPoison.ActionID.SpellID,
			"proc_mask": hands(rogueInstantImbueID), "proc_chance": 0.2 + bonus, "min_damage": 76.0, "max_damage": 100.0,
		})
	}
	if r.DeadlyPoison != nil {
		effects = append(effects, map[string]any{
			"kind": "deadly_poison", "trigger_aura": "Deadly Poison", "spell_id": r.DeadlyPoison.ActionID.SpellID,
			"tag": r.DeadlyPoison.ActionID.Tag, "proc_mask": hands(rogueDeadlyImbueID), "proc_chance": 0.3 + bonus,
			"tick_damage": 23.0,
		})
	}
	return effects
}

// Rogue behavior the exporter cannot describe.
func rogueUnrepresented(agent core.Agent, _ *core.Character) []string {
	r := agent.(rogue.RogueAgent).GetRogue()
	unrepresented := []string{}
	// Venom raises the poison chance while its aura is up, a running total the procs read.
	if r.Talents.Venom {
		unrepresented = append(unrepresented, "Venom's poison chance is unsupported")
	}
	if r.AdditiveEnergyRegenBonus != 0 {
		unrepresented = append(unrepresented, "an additive energy regeneration bonus is unsupported")
	}
	return unrepresented
}
