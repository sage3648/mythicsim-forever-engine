// Rogue export, continued: the Assassination and Subtlety spells and talents. Each formula
// mirrors the cited file of sim/rogue at the pinned revision.
package main

import (
	"math"
	"time"

	"github.com/wowsims/forever/sim/core"
	"github.com/wowsims/forever/sim/core/buffs"
	"github.com/wowsims/forever/sim/core/dbcenums"
	"github.com/wowsims/forever/sim/core/proto"
	"github.com/wowsims/forever/sim/core/spelldata"
	"github.com/wowsims/forever/sim/rogue"
)

// More of sim/rogue/spell_data_auto_gen.go's ladders, restated with its ids.
var (
	rogueAmbush                = spelldata.Ranked(8676, 8724, 8725, 11267, 11268, 11269)
	rogueColdBlood             = spelldata.Ranked(14177)
	rogueGarrote               = spelldata.Ranked(703, 8631, 8632, 8633, 11289, 11290)
	rogueHemorrhage            = spelldata.Ranked(16511)
	rogueHackAndSlash          = spelldata.Talent(13960, 5)
	rogueImprovedPoisonsSpecs  = spelldata.Talent(14113, 5)
	rogueVenom                 = spelldata.Ranked(1310703)
	rogueQuietus               = spelldata.Talent(1310728, 5)
	rogueKidneyShot            = spelldata.Ranked(408, 8643)
	rogueImprovedExposeArmor   = spelldata.Talent(14168, 2)
	rogueRiposte               = spelldata.Ranked(14251)
	rogueImprovedKidneyShot    = spelldata.Talent(14174, 2)
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

// talents_subtlety.go registerQuietus registers one execute phase callback a fight.
func rogueExecuteCallbacks(agent core.Agent) int {
	if agent.(rogue.RogueAgent).GetRogue().Talents.Quietus > 0 {
		return 1
	}
	return 0
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
	// literal, and the multiplier Hemorrhage's debuff gives the rogue's Rupture ticks while it is up,
	// read from the damage taken from caster effect of the client row (talents_subtlety.go).
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
			"hemorrhage_aura": hemorrhage, "hemorrhage_multiplier": 1 + rogueHemorrhage.Highest().Effect(dbcenums.A_MOD_SPELL_DAMAGE_FROM_CASTER, 0).Percent(),
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
	// them hits, except Mutilate's hand strikes, which take the crit without spending it
	// (community #690).
	if r.ColdBlood != nil {
		row := rogueColdBlood.Highest()
		effects = append(effects, map[string]any{
			"kind": "cold_blood", "spell_id": row.ID, "aura": "Cold Blood",
			"crit_bonus":         row.Effect(dbcenums.A_ADD_FLAT_MODIFIER, int32(dbcenums.SPELLMOD_CRITICAL_CHANCE)).Average(core.CharacterLevel),
			"class_spells":       rogueMaskNames(rogue.RogueSpellColdBlooded),
			"spend_class_spells": rogueMaskNames(rogue.RogueSpellColdBlooded &^ rogue.RogueSpellMutilateHit),
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
	// talents_subtlety.go registerPreparation: it finishes the cooldown of every other rogue
	// spell that has one, in spellbook order, and it fires as a major cooldown once Vanish is
	// cooling down.
	if r.Preparation != nil {
		reset := []int{}
		for i, spell := range character.Spellbook {
			if spell != r.Preparation && spell.ClassSpellMask&rogue.RogueSpellsAll != 0 && spell.CD.Timer != nil {
				reset = append(reset, i)
			}
		}
		effects = append(effects, map[string]any{
			"kind": "preparation", "spell_id": r.Preparation.ActionID.SpellID, "reset_spells": reset,
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
	// talents_combat.go registerHackAndSlash: on axes and swords, a landed hit from that hand
	// grants an extra main hand attack at once.
	if mask := r.GetProcMaskForTypes(proto.WeaponType_WeaponTypeAxe, proto.WeaponType_WeaponTypeSword); talents.HackAndSlash > 0 && mask != core.ProcMaskUnknown {
		trigger := core.ProcTrigger{ProcMask: mask}
		effect := rogueProc(character, "Hack and Slash", "extra_attack", rogueHackAndSlash.EffectAt(3).ValueAt(talents.HackAndSlash)/100, trigger, "landed", false)
		effect["delay_ns"] = int64(0)
		effects = append(effects, effect)
	}
	// poisons.go registerWoundPoisonSpell: a magic hit roll that stacks the healing debuff.
	if r.WoundPoison != nil {
		var mask core.ProcMask
		if r.Consumables.MhImbueId == 27188 {
			mask |= core.ProcMaskMeleeMH
		}
		if r.Consumables.OhImbueId == 27188 {
			mask |= core.ProcMaskMeleeOH
		}
		bonus := rogueImprovedPoisonsSpecs.Effect(dbcenums.A_ADD_FLAT_MODIFIER, int32(dbcenums.SPELLMOD_CHANCE_OF_SUCCESS)).FractionAt(talents.ImprovedPoisons)
		effects = append(effects, map[string]any{
			"kind": "wound_poison", "trigger_aura": "Wound Poison", "spell_id": r.WoundPoison.ActionID.SpellID,
			"proc_mask": procMaskNames(mask), "proc_chance": 0.3 + bonus, "debuff_aura": r.WoundPoisonDebuffAuras.Get(target).Label,
		})
	}
	// talents_assassination.go registerVenom: a finisher whose aura, on the Slice and Dice ladder
	// without Improved Slice and Dice, raises poison damage and the poison chance.
	if r.Venom != nil {
		row := rogueVenom.Highest()
		ladder := privateField(r, "sliceAndDiceDurations")
		durations := []int64{}
		for points := 0; points < ladder.Len(); points++ {
			durations = append(durations, ladder.Index(points).Int())
		}
		effects = append(effects, map[string]any{
			"kind": "venom", "spell_id": row.ID, "aura": r.VenomAura.Label, "durations_ns": durations,
			"damage_bonus": row.Effect(dbcenums.A_ADD_PCT_MODIFIER, int32(dbcenums.SPELLMOD_DAMAGE)).Average(core.CharacterLevel) / 100,
			"chance_bonus": row.Effect(dbcenums.A_ADD_FLAT_MODIFIER, int32(dbcenums.SPELLMOD_CHANCE_OF_SUCCESS)).Average(core.CharacterLevel) / 100,
			"class_spells": rogueMaskNames(rogue.RogueSpellPoisons),
		})
	}
	// talents_subtlety.go registerGhostlyStrike: main hand weapon damage, its weapon share in the
	// spell's multiplier, and a dodge buff, a stat aura when the target tanks the player.
	if r.GhostlyStrike != nil {
		dodge := ""
		if aura := character.GetAura("Ghostly Strike Buff"); aura != nil {
			dodge = aura.Label
		}
		effects = append(effects, map[string]any{
			"kind": "ghostly_strike", "spell_id": r.GhostlyStrike.ActionID.SpellID, "dodge_aura": dodge,
		})
	}
	// talents_subtlety.go registerHemorrhage: normalized main hand damage and its debuff.
	if r.Hemorrhage != nil {
		effects = append(effects, map[string]any{
			"kind": "hemorrhage", "spell_id": r.Hemorrhage.ActionID.SpellID, "debuff_aura": r.HemorrhageAuras.Get(target).Label,
		})
	}
	// garrote.go: from Stealth, behind the target unless Dirty Deeds, a bleed of the tick and 3% of
	// attack power, a Go literal, read again at each tick.
	if r.Garrote != nil {
		row := rogueGarrote.Highest()
		effects = append(effects, map[string]any{
			"kind": "garrote", "spell_id": row.ID, "tick_damage": row.PeriodicEffect().Average(core.CharacterLevel),
			"attack_power_share": 0.03, "tick_can_crit": row.PeriodicCanCrit(),
			"magic": row.DefenseTypeCore() == core.DefenseTypeMagic, "dirty_deeds": talents.DirtyDeeds > 0,
		})
	}
	// kidney_shot.go: a finisher whose stun lasts a second and a second a combo point, scaled by
	// the target's stun duration multiplier, and raises the rogue's damage on the target by
	// Improved Kidney Shot. A stun immune target ignores it. A stun also pauses the target's
	// swings, which only matter when it tanks the player.
	if spell := character.GetSpell(core.ActionID{SpellID: 8643}); spell != nil {
		stun := ""
		for _, aura := range target.GetAuras() {
			if aura.ActionID == spell.ActionID && aura.Tag == core.StunAuraTag {
				stun = aura.Label
			}
		}
		effects = append(effects, map[string]any{
			"kind": "kidney_shot", "spell_id": spell.ActionID.SpellID, "target_stun_immune": target.PseudoStats.StunImmune,
			"stun_aura": stun, "base_duration_ns": nanos(rogueKidneyShot.Highest().Duration()),
			"duration_per_combo_point_ns": nanos(time.Second), "stun_duration_multiplier": target.PseudoStats.StunDurationMultiplier,
			"damage_taken_multiplier": 1 + rogueImprovedKidneyShot.ValueAt(talents.ImprovedKidneyShot)/100,
			"target_tanks_player":     target.CurrentTarget == &character.Unit || target.SecondaryTarget == &character.Unit,
		})
	}
	effects = append(effects, rogueExposeArmorEffects(r, character)...)
	// talents_subtlety.go registerQuietus: an execute phase callback that raises Sinister Strike,
	// Ghostly Strike and Hemorrhage once the target reaches 35%.
	if aura := character.GetAura("Quietus"); aura != nil {
		effects = append(effects, map[string]any{
			"kind": "quietus", "aura": aura.Label, "execute_phase": int32(35),
			"damage_bonus": rogueQuietus.EffectAt(1).FractionAt(talents.Quietus),
			"class_spells": rogueMaskNames(rogue.RogueSpellQuietus),
		})
	}
	// talents_combat.go registerRiposte: its trigger hears parries the player makes, and nothing
	// attacks the player unless a target tanks it. A parry readies Riposte, a main hand weapon
	// strike that spends the ready aura.
	if aura := character.GetAura("Riposte Trigger"); aura != nil {
		if target.CurrentTarget == &character.Unit || target.SecondaryTarget == &character.Unit {
			effects = append(effects, map[string]any{
				"kind": "riposte", "spell_id": rogueRiposte.Highest().ID, "trigger_aura": aura.Label,
				"ready_aura": character.GetAura("Riposte Ready").Label,
			})
		} else {
			effects = append(effects, map[string]any{
				"kind": "inert_listener", "unit": "player", "aura": aura.Label,
				"reason": "acts only on attacks the player parries, and nothing attacks the player",
			})
		}
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

// expose_armor.go: a finisher whose debuff bids 450 armor a combo point, the raid debuff's
// value over five, and takes that much armor off the target while it holds the target's major
// armor category. Improved Expose Armor hands combo points back on a five point spend. The
// category comes with the target's armor at each stack count of the raid's Sunder Armor, read
// from separate reset simulations. A permanent member, such as the raid's own Expose Armor,
// that holds the category from the reset blocks the debuff for good, and the export carries
// its bid instead.
func rogueExposeArmorEffects(r *rogue.Rogue, character *core.Character) []map[string]any {
	if r.ExposeArmor == nil || exportRequest == nil {
		return nil
	}
	target := character.Env.Encounter.ActiveTargetUnits[0]
	aura := r.ExposeArmorAuras.Get(target)
	if len(aura.ExclusiveEffects) != 1 {
		*classNotes = append(*classNotes, "Expose Armor holds several exclusive effects")
		return nil
	}
	category := aura.ExclusiveEffects[0].Category
	effect := map[string]any{
		"kind": "expose_armor", "spell_id": r.ExposeArmor.ActionID.SpellID, "aura": aura.Label,
		"armor_per_combo_point": math.Abs(buffs.ExposeArmorValue(0)) / 5,
		"points_back":           int32(rogueImprovedExposeArmor.EffectAt(2).ValueAt(r.Talents.ImprovedExposeArmor)),
		"points_back_action":    actionID(core.ActionID{SpellID: 14169}),
	}
	if blockedForGood(aura) {
		effect["blocking_priority"] = category.GetActiveEffect().Priority
		return []map[string]any{effect}
	}
	if active := category.GetActiveEffect(); active != nil {
		*classNotes = append(*classNotes, "the major armor category holds "+active.Aura.Label+" from the reset")
		return nil
	}
	exclusive := exclusiveCategoryEffect(target, "target", category.Name)
	if exclusive == nil {
		return nil
	}
	armor := []float64{targetArmorWithStacks(exportRequest, aura.Label, 0)}
	for _, member := range exclusive["members"].([]map[string]any) {
		label := member["aura"].(string)
		if other := target.GetAura(label); other != nil && other.MaxStacks > 0 {
			if len(armor) > 1 {
				*classNotes = append(*classNotes, "the major armor category holds several stacking debuffs")
				return nil
			}
			for stacks := int32(1); stacks <= other.MaxStacks; stacks++ {
				armor = append(armor, targetArmorWithStacks(exportRequest, label, stacks))
			}
			member["per_stack"] = stackBid(exportRequest, label, category.Name)
		}
	}
	exclusive["armor_by_stacks"] = armor
	return []map[string]any{effect, exclusive}
}
