// Rogue export, continued: the Assassination and Subtlety spells and talents. Each formula
// mirrors the cited file of sim/rogue at the pinned revision.
package main

import (
	"github.com/wowsims/forever/sim/core"
	"github.com/wowsims/forever/sim/core/dbcenums"
	"github.com/wowsims/forever/sim/core/spelldata"
	"github.com/wowsims/forever/sim/rogue"
)

// More of sim/rogue/spell_data_auto_gen.go's ladders, restated with its ids.
var (
	rogueAmbush                = spelldata.Ranked(8676, 8724, 8725, 11267, 11268, 11269)
	rogueColdBlood             = spelldata.Ranked(14177)
	rogueCutthroat             = spelldata.Talent(462708, 5)
	rogueInitiative            = spelldata.Talent(13976, 3)
	rogueInitiativeTriggered   = spelldata.Ranked(13977)
	rogueMutilate              = spelldata.Ranked(1310707, 399956, 1241582, 1241584)
	rogueMutilateTriggered     = spelldata.Ranked(399960, 399961, 1241585, 1241586, 1241588, 1241590, 1310705, 1310706)
	roguePremeditation         = spelldata.Ranked(14183)
	rogueRupture               = spelldata.Ranked(1943, 8639, 8640, 11273, 11274, 11275)
	rogueSealFate              = spelldata.Talent(14186, 5)
	rogueThousandCutsTriggered = spelldata.Ranked(1310723)
)

// talents_assassination.go mutilateFlatDamage, keyed on the parent rank.
var rogueMutilateFlatDamage = map[int32]float64{1310707: 23, 399956: 33, 1241582: 48, 1241584: 67}

// Every class spell a mask names, as the stable names Rust reads.
func rogueMaskNames(mask int64) []string {
	names := []string{}
	for _, entry := range rogueClassSpells {
		if mask&entry.mask != 0 {
			names = append(names, entry.name)
		}
	}
	return names
}

// A talent proc trigger's export: the spellbook positions it hears, by matchesSpell, and the
// outcome it needs. Go's handler runs a spell batch window after the roll.
func rogueProc(character *core.Character, label string, handler string, chance float64, trigger core.ProcTrigger, outcome string, periodic bool) map[string]any {
	return map[string]any{
		"kind": "rogue_proc", "trigger_aura": label, "handler": handler, "proc_chance": chance,
		"spells": procTriggerSpells(character, trigger), "outcome": outcome, "periodic": periodic,
		"delay_ns": nanos(core.SpellBatchWindow),
	}
}

func rogueSpecEffects(r *rogue.Rogue, character *core.Character) []map[string]any {
	talents := r.Talents
	target := character.Env.Encounter.ActiveTargetUnits[0]
	effects := []map[string]any{}

	// stealth.go and vanish.go: Stealth before the pull, and Vanish, which stops the swings.
	effects = append(effects, map[string]any{
		"kind": "stealth", "aura": r.StealthAura.Label, "spell_id": r.StealthAura.ActionID.SpellID,
		"vanish_spell_id": r.Vanish.ActionID.SpellID,
	})
	// ambush.go: from Stealth, or under Cutthroat, with a main hand dagger.
	if r.Ambush != nil {
		row := rogueAmbush.Highest()
		cutthroat := ""
		if r.CutthroatAura != nil {
			cutthroat = r.CutthroatAura.Label
		}
		effects = append(effects, map[string]any{
			"kind": "ambush", "spell_id": row.ID, "base_damage": row.DamageEffect().Average(core.CharacterLevel),
			"main_hand_dagger": r.HasDagger(core.MainHand), "cutthroat_aura": cutthroat,
		})
	}
	// rupture.go: the tick and its step a combo point, the attack power share a point, a Go
	// literal, and Hemorrhage's multiplier, a Go literal, while its debuff is up.
	if r.Rupture != nil {
		row := rogueRupture.Highest()
		tick := row.PeriodicEffect()
		hemorrhage := ""
		if r.HemorrhageAuras != nil {
			hemorrhage = r.HemorrhageAuras.Get(target).Label
		}
		effects = append(effects, map[string]any{
			"kind": "rupture", "spell_id": row.ID, "tick_damage": tick.Average(core.CharacterLevel),
			"damage_per_combo_point": float64(tick.PointsPerResource),
			"base_tick_count":        int32(row.Duration() / tick.Period()),
			"attack_power_shares":    []float64{0, 0.01, 0.02, 0.03, 0.03, 0.03},
			"tick_can_crit":          row.PeriodicCanCrit(), "magic": row.DefenseTypeCore() == core.DefenseTypeMagic,
			"hemorrhage_aura": hemorrhage, "hemorrhage_multiplier": rogue.HemorrhageRuptureMultiplier,
		})
	}
	// talents_assassination.go registerMutilate: two combo points, then the off hand and main
	// hand hits, each harder while a lingering poison is on the target.
	if r.Mutilate != nil {
		row := rogueMutilate.ByID(rogue.MutilateSpellID)
		effects = append(effects, map[string]any{
			"kind": "mutilate", "spell_id": row.ID, "flat_damage": rogueMutilateFlatDamage[row.ID],
			"poison_bonus": row.EffectN(4).Average(core.CharacterLevel) / 100,
			"combo_points": int32(row.EnergizeEffect().Average(core.CharacterLevel)),
			"daggers":      r.HasDagger(core.MainHand) && r.HasDagger(core.OffHand),
			"weapon_share": rogueMutilateTriggered.EffectAt(2).ValueAt(1) / 100,
		})
	}
	// talents_assassination.go registerColdBlood: a crit bonus on the masked spells until one of
	// them hits.
	if r.ColdBlood != nil {
		row := rogueColdBlood.Highest()
		effects = append(effects, map[string]any{
			"kind": "cold_blood", "spell_id": row.ID, "aura": "Cold Blood",
			"crit_bonus":   row.Effect(dbcenums.A_ADD_FLAT_MODIFIER, int32(dbcenums.SPELLMOD_CRITICAL_CHANCE)).Average(core.CharacterLevel),
			"class_spells": rogueMaskNames(rogue.RogueSpellColdBlooded),
		})
	}
	// talents_subtlety.go registerPremeditation: combo points from Stealth.
	if r.Premeditation != nil {
		row := roguePremeditation.Highest()
		effects = append(effects, map[string]any{
			"kind": "premeditation", "spell_id": row.ID,
			"combo_points": int32(row.EnergizeEffect().Average(core.CharacterLevel)),
		})
	}
	// talents_subtlety.go registerPreparation: the cooldowns it resets, and it fires as a major
	// cooldown once Vanish is cooling down.
	if r.Preparation != nil {
		reset := []int32{}
		for _, spell := range []*core.Spell{r.ColdBlood, r.Shadowstep, r.Premeditation, r.Vanish} {
			if spell != nil {
				reset = append(reset, spell.ActionID.SpellID)
			}
		}
		effects = append(effects, map[string]any{
			"kind": "preparation", "spell_id": r.Preparation.ActionID.SpellID, "reset_spell_ids": reset,
		})
	}
	// talents_assassination.go registerSealFate: a crit from a builder adds a combo point.
	if talents.SealFate > 0 {
		trigger := core.ProcTrigger{SpellFlags: rogue.SpellFlagBuilder}
		effect := rogueProc(character, "Seal Fate Trigger", "combo_point", rogueSealFate.FractionAt(talents.SealFate), trigger, "crit", false)
		effect["action"] = actionID(core.ActionID{SpellID: 14195})
		effects = append(effects, effect)
	}
	// talents_subtlety.go registerInitiative: a landed Garrote or Ambush adds a combo point.
	if talents.Initiative > 0 {
		trigger := core.ProcTrigger{ClassSpellMask: rogue.RogueSpellGarrote | rogue.RogueSpellAmbush}
		effect := rogueProc(character, "Initiative Trigger", "combo_point", rogueInitiative.FractionAt(talents.Initiative), trigger, "landed", false)
		effect["action"] = actionID(core.ActionID{SpellID: rogueInitiativeTriggered.Highest().ID})
		effects = append(effects, effect)
	}
	// talents_subtlety.go registerCutthroat: a landed Backstab lets Ambush out of Stealth.
	if r.CutthroatAura != nil {
		trigger := core.ProcTrigger{ClassSpellMask: rogue.RogueSpellBackstab}
		effect := rogueProc(character, "Cutthroat Trigger", "activate", rogueCutthroat.FractionAt(talents.Cutthroat), trigger, "landed", false)
		effect["aura"] = r.CutthroatAura.Label
		effects = append(effects, effect)
	}
	// talents_subtlety.go registerThousandCuts: Rupture ticks stack a discount on the next
	// Backstab or Hemorrhage, which spends it.
	if r.ThousandCutsAura != nil {
		buff := rogueThousandCutsTriggered.Highest()
		trigger := core.ProcTrigger{ClassSpellMask: rogue.RogueSpellRupture}
		effect := rogueProc(character, "Thousand Cuts Trigger", "stack", 1, trigger, "any", true)
		effect["aura"] = r.ThousandCutsAura.Label
		effects = append(effects, effect)
		effects = append(effects, map[string]any{
			"kind": "thousand_cuts", "aura": r.ThousandCutsAura.Label,
			"cost_per_stack": int32(buff.Effect(dbcenums.A_ADD_FLAT_MODIFIER, int32(dbcenums.SPELLMOD_COST)).Average(core.CharacterLevel)),
			"class_spells":   rogueMaskNames(rogue.RogueSpellBackstab | rogue.RogueSpellHemorrhage),
		})
	}
	return effects
}
