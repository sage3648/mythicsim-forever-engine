// Hunter export: class spell names and the effects Go keeps in closures. Each formula
// mirrors the cited Go file at the pinned revision.
package main

import (
	"fmt"
	"reflect"
	"time"
	"unsafe"

	"github.com/wowsims/forever/sim/core"
	"github.com/wowsims/forever/sim/core/buffs"
	"github.com/wowsims/forever/sim/core/dbcenums"
	"github.com/wowsims/forever/sim/core/proto"
	"github.com/wowsims/forever/sim/core/spelldata"
	"github.com/wowsims/forever/sim/hunter"
	"google.golang.org/protobuf/reflect/protoreflect"
)

func init() {
	classExports[proto.Class_ClassHunter] = classExport{
		spells: hunterClassSpells, effects: hunterEffects, unrepresented: hunterUnrepresented,
		swingReplacementKeepsSwing: hunterSwingReplacementKeepsSwing,
		// talents_beast_mastery.go registerIntimidation registers its cast without a class mask.
		unmaskedSpells: map[core.ActionID]string{{SpellID: 19577}: "intimidation"},
		statAuras: func(core.Agent, *core.Character) []string {
			return []string{"Aspect of the Hawk", "Aspect of the Beast"}
		},
	}
	// items.go Renataki's Charm of Beasts: its use resets the cooldowns of the shots it names.
	classItemUseEffects[19953] = hunterRenatakisCharm
	// pet.go: the pet is enabled on start and never summoned after; its rotation may disable it
	// once the fight passes its uptime, which the hunter_pet effect describes.
	resetOnlyPetClasses[proto.Class_ClassHunter] = true
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
	hunterAimedShot         = spelldata.Ranked(19434, 20900, 20901, 20902, 20903, 20904)
	hunterSniperShot        = spelldata.Ranked(1310687, 1310785, 1310786)
	hunterArcaneShot        = spelldata.Ranked(3044, 14281, 14282, 14283, 14284, 14285, 14286, 14287)
	hunterSerpentSting      = spelldata.Ranked(1978, 13549, 13550, 13551, 13552, 13553, 13554, 13555, 25295)
	hunterAspectOfTheHawk   = spelldata.Ranked(13165, 14318, 14319, 14320, 14321, 14322, 25296)
	hunterQuickShots        = spelldata.Ranked(6150)
	hunterDeadlyAspects     = spelldata.Talent(19552, 5)
	hunterRapidFire         = spelldata.Ranked(3045)
	hunterSummonHawk        = spelldata.Ranked(1293241, 1293525, 1293526, 1293527)
	hunterSummonHawkSummon  = spelldata.Ranked(1293248, 1312639)
	hunterFrenzy            = spelldata.Talent(19621, 5)
	hunterAspectOfTheBeast  = spelldata.Ranked(13161, 1299445, 1299446, 1299447)
	hunterQuickStrikes      = spelldata.Ranked(1299448)
	hunterRaptorStrike      = spelldata.Ranked(2973, 14260, 14261, 14262, 14263, 14264, 14265, 14266)
	hunterMongooseBite      = spelldata.Ranked(1495, 14269, 14270, 14271)
	hunterWingClip          = spelldata.Ranked(2974, 14267, 14268)
	hunterLacerating        = spelldata.Ranked(1310536)
	hunterImmolationTrap    = spelldata.Ranked(13795, 14302, 14303, 14304, 14305)
	hunterImmolationEffect  = spelldata.Ranked(13797, 14298, 14299, 14300, 14301)
	hunterExposePrey        = spelldata.Talent(1310532, 2)
	hunterResourcefulBuff   = spelldata.Ranked(1242688)
	hunterRapidRecuperation = spelldata.Talent(1223987, 2)
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
	// arcane_shot.go: the rank's flat damage from client data plus a share of ranged attack
	// power, a Go literal fitted to beta logs, on the ranged hit and crit table after travel.
	if h.ArcaneShot != nil {
		effects = append(effects, map[string]any{
			"kind": "arcane_shot", "spell_id": h.ArcaneShot.ActionID.SpellID,
			"base_damage":     hunterArcaneShot.Highest().DamageEffect().Average(core.CharacterLevel),
			"rap_coefficient": 0.11,
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
	if h.Pet != nil {
		effects = append(effects, hunterPetEffects(h, character)...)
	}
	effects = append(effects, hunterMeleeEffects(h, character)...)
	// talents_marksmanship.go registerRapidRecuperation: Serpent Sting's hit, a spell batch window
	// later and if it landed, grants casting regeneration, which Go adds to the pseudo stat
	// without recomputing the regeneration rates.
	if trigger := character.GetAura("Rapid Recuperation Trigger"); trigger != nil {
		effects = append(effects, map[string]any{
			"kind": "rapid_recuperation", "trigger_aura": trigger.Label,
			"aura":  character.GetAura("Rapid Recuperation").Label,
			"regen": hunterRapidRecuperation.EffectAt(1).FractionAt(h.Talents.RapidRecuperation),
		})
	}
	effects = append(effects, hunterSetManaProcs(character)...)
	// talents_survival.go Defensive State: the trigger hears only attacks the player dodges.
	if character.GetAura("Defensive State - Trigger") != nil {
		effects = append(effects, map[string]any{
			"kind": "inert_listener", "unit": "player", "aura": "Defensive State - Trigger",
			"reason": "acts only on attacks the player dodges, and nothing attacks the player",
		})
	}
	return effects
}

// raptor_strike.go TryRaptorStrike returns the swing it is given unless a Raptor Strike is
// queued, and only the queue spell, which only a rotation casts, queues one. A rotation that
// never names the queue spell keeps every swing.
func hunterSwingReplacementKeepsSwing(agent core.Agent, player *proto.Player) bool {
	h := agent.(hunter.HunterAgent).GetHunter()
	if h.RaptorStrike == nil {
		return true
	}
	queue := h.RaptorStrike.ActionID.WithTag(3)
	named := false
	var visit func(message protoreflect.Message)
	visit = func(message protoreflect.Message) {
		if id, ok := message.Interface().(*proto.ActionID); ok {
			if core.ProtoToActionID(id) == queue {
				named = true
			}
			return
		}
		message.Range(func(field protoreflect.FieldDescriptor, value protoreflect.Value) bool {
			switch {
			case field.IsList() && field.Message() != nil:
				for i := 0; i < value.List().Len(); i++ {
					visit(value.List().Get(i).Message())
				}
			case field.IsMap():
				value.Map().Range(func(_ protoreflect.MapKey, entry protoreflect.Value) bool {
					if field.MapValue().Message() != nil {
						visit(entry.Message())
					}
					return true
				})
			case field.Message() != nil:
				visit(value.Message())
			}
			return true
		})
	}
	if rotation := player.GetRotation(); rotation != nil {
		visit(rotation.ProtoReflect())
	}
	return !named
}

// items.go Renataki's Charm of Beasts: Aimed Shot's, Multi-Shot's and Arcane Shot's cooldowns,
// as the hunter has them, reset at once; the major cooldown waits for one of them to be cooling.
func hunterRenatakisCharm(agent core.Agent, spell *core.Spell) map[string]any {
	h := agent.(hunter.HunterAgent).GetHunter()
	shots := []int32{}
	for _, shot := range []*core.Spell{h.AimedShot, h.MultiShot, h.ArcaneShot} {
		if shot != nil {
			shots = append(shots, shot.ActionID.SpellID)
		}
	}
	return map[string]any{"kind": "renatakis_charm", "item_id": spell.ActionID.ItemID, "shots": shots}
}

// Set bonus proc triggers that restore mana: common/forever/item_sets_classic.go Beaststalker
// Armor (5) and hunter/item_sets.go Beastmaster Armor (4), 5% on landed white hits for 200, and
// Cryptstalker Armor (6), every ranged crit for 50, Go literals; each handler waits a spell
// batch window and restores mana to a character with a mana bar.
func hunterSetManaProcs(character *core.Character) []map[string]any {
	procs := []struct {
		label   string
		trigger core.ProcTrigger
		outcome string
		mana    float64
		metrics int32
	}{
		{"Beaststalker Armor 5P", core.ProcTrigger{ProcMask: core.ProcMaskWhiteHit, ProcChance: 0.05}, "landed", 200, 450577},
		{"Beastmaster Armor 4P", core.ProcTrigger{ProcMask: core.ProcMaskWhiteHit, ProcChance: 0.05}, "landed", 200, 450577},
		{"Cryptstalker Armor 6P", core.ProcTrigger{ProcMask: core.ProcMaskRanged, ProcChance: 1}, "crit", 50, 28753},
	}
	effects := []map[string]any{}
	for _, proc := range procs {
		if character.GetAura(proc.label) == nil {
			continue
		}
		effects = append(effects, map[string]any{
			"kind": "hunter_set_mana_proc", "trigger_aura": proc.label,
			"spells": procTriggerSpells(character, proc.trigger), "outcome": proc.outcome,
			"proc_chance": proc.trigger.ProcChance, "mana": proc.mana,
			"metrics_action_id": actionID(core.ActionID{SpellID: proc.metrics}),
			"delay_ns":          nanos(core.SpellBatchWindow),
		})
	}
	return effects
}

// Hunter behavior the effects cannot describe.
func hunterUnrepresented(agent core.Agent, character *core.Character) []string {
	h := agent.(hunter.HunterAgent).GetHunter()
	notes := []string{}
	// summon_hawk.go deals periodic physical damage, which this multiplier would scale.
	if h.SummonHawk != nil && character.CurrentTarget.PseudoStats.PeriodicPhysicalDamageTakenMultiplier != 1 {
		notes = append(notes, "periodic physical damage taken multipliers are unsupported")
	}
	return notes
}

// pet_abilities.go: each literal ability's rolled range and hit table, by the pet spell's ID.
var hunterPetAbilityRolls = map[int32]struct {
	roll    [2]float64
	outcome string
}{
	17261: {[2]float64{81, 99}, "melee_special"}, // newBite
	3009:  {[2]float64{43, 59}, "melee_special"}, // newClaw
	25012: {[2]float64{86, 98}, "magic"},         // newLightningBreath
}

// pet_abilities.go NewPetAbility: the client rows of the abilities built from them.
var (
	hunterPetStrikeRows = []spelldata.Ladder{ // newPetStrike
		spelldata.Ranked(24423, 24577, 24578, 24579),                  // Demoralizing Screech
		spelldata.Ranked(1264735, 1264736, 1264739, 1264741, 1264742), // Pinch
		spelldata.Ranked(1264758, 1264927, 1264929, 1264930, 1264933), // Dismember
		spelldata.Ranked(1265054, 1265055, 1265056, 1265057, 1265058), // Mine!
	}
	hunterPetBleedRows = []spelldata.Ladder{ // newPetBleed
		spelldata.Ranked(1265065, 1265066, 1265067, 1265068, 1265069), // Savage Rend
		spelldata.Ranked(1265038, 1265039, 1265040, 1265041, 1265042), // Tendon Rip
		spelldata.Ranked(1265843, 1265878, 1265880, 1265881, 1265883), // Web
	}
	hunterThunderstomp = spelldata.Ranked(26090, 26187, 26188, 1264455)
	hunterDustCloud    = spelldata.Ranked(1265899, 1265901, 1265902, 1265903, 1265904)
	hunterSwipe        = spelldata.Ranked(1264494, 1264497, 1264498, 1264501, 1264502)
)

// The row among the ladders' highest ranks with this spell ID.
func hunterPetRow(ladders []spelldata.Ladder, id int32) *spelldata.Spell {
	for _, ladder := range ladders {
		if row := ladder.Highest(); row.ID == id {
			return row
		}
	}
	return nil
}

// spelldata Effect.Roll as a hit's range: no draw without a variance.
func hunterPetRowStrike(id int32, effect *spelldata.Effect, outcome string) map[string]any {
	average := effect.Average(core.CharacterLevel)
	return map[string]any{
		"kind": "hunter_pet_strike", "spell_id": id, "outcome": outcome, "draws": effect.Variance != 0,
		"min_damage": average * (1 - effect.Variance/2), "max_damage": average * (1 + effect.Variance/2),
		"average": average, "variance": effect.Variance,
	}
}

// The effect of one pet damage ability, or nil when Rust has none.
func hunterPetAbility(spell *core.Spell) map[string]any {
	id := spell.ActionID.SpellID
	if spell.ActionID.Tag != 0 {
		return nil
	}
	// newScorpidPoison: a melee special hit roll, then a dot whose stack Apply resets, so each
	// landed cast is one stack of the Go literal tick.
	if id == 24587 {
		return map[string]any{"kind": "hunter_pet_scorpid_poison", "spell_id": id, "tick_base": 5.0}
	}
	if literal, ok := hunterPetAbilityRolls[id]; ok {
		return map[string]any{
			"kind": "hunter_pet_strike", "spell_id": id, "outcome": literal.outcome, "draws": true,
			"min_damage": literal.roll[0], "max_damage": literal.roll[1],
		}
	}
	if row := hunterPetRow(hunterPetStrikeRows, id); row != nil {
		if spell.SpellSchool != core.SpellSchoolPhysical {
			return nil
		}
		return hunterPetRowStrike(id, row.DamageEffect(), "melee_special")
	}
	// newThunderstomp: a cleave of magic hits, one roll each, so one hit on a single target.
	if row := hunterThunderstomp.Highest(); row.ID == id {
		return hunterPetRowStrike(id, row.DamageEffect(), "magic")
	}
	// newSwipe: its cast condition needs three active targets.
	if row := hunterSwipe.Highest(); row.ID == id {
		return map[string]any{"kind": "hunter_pet_swipe", "spell_id": id, "min_targets": 3}
	}
	if row := hunterPetRow(hunterPetBleedRows, id); row != nil && spell.Dot(spell.Unit.CurrentTarget) != nil {
		hit := "melee_special"
		if spell.DefenseType == core.DefenseTypeRanged {
			hit = "ranged"
		}
		// spelldata TickOutcomeHitRolled: a crit roll the row allows, else a plain tick.
		tick := "plain"
		if row.PeriodicCanCrit() {
			tick = "physical_crit"
			if spell.DefenseType == core.DefenseTypeMagic {
				tick = "magic_crit"
			}
		}
		return map[string]any{
			"kind": "hunter_pet_bleed", "spell_id": id, "hit": hit, "tick_outcome": tick,
			"tick_base": row.PeriodicEffect().Average(core.CharacterLevel),
		}
	}
	return nil
}

func hunterPetSpellPosition(pet *hunter.HunterPet, spell *core.Spell) int {
	for i, candidate := range pet.Spellbook {
		if candidate == spell {
			return i
		}
	}
	return -1
}

// pet.go ExecuteCustomRotation and pet_abilities.go, and the pet auras of Intimidation, Bestial
// Wrath and Frenzy (talents_beast_mastery.go).
func hunterPetEffects(h *hunter.Hunter, character *core.Character) []map[string]any {
	pet := h.Pet
	effects := []map[string]any{}
	abilities := map[string]int{}
	rotation := "default"
	switch h.Options.PetType {
	case proto.HunterOptions_Cat:
		rotation = "cat"
	case proto.HunterOptions_Scorpid:
		rotation = "scorpid"
	}
	for _, spell := range pet.Spellbook {
		id := spell.ActionID
		if spell.ClassSpellMask != hunter.HunterPetDamage {
			continue
		}
		effect := hunterPetAbility(spell)
		if effect == nil {
			*classNotes = append(*classNotes, fmt.Sprintf("pet ability %s is unsupported", id))
			continue
		}
		effects = append(effects, effect)
		abilities[fmt.Sprint(id.SpellID)] = hunterPetSpellPosition(pet, spell)
	}
	// newDustCloud: no damage mask; a landed melee special hit puts the target's armor down while
	// its aura holds, and the pet casts it again once it falls off.
	{
		row := hunterDustCloud.Highest()
		for _, spell := range pet.Spellbook {
			if spell.ActionID != (core.ActionID{SpellID: row.ID}) {
				continue
			}
			parsed := spelldata.DryRun(row, spelldata.IgnoreStacks())
			aura := pet.CurrentTarget.GetAura(row.Name)
			if aura == nil || len(parsed.Skipped) != 0 || len(parsed.Applied) != 1 || parsed.Applied[0].Kind != "stat Armor" {
				*classNotes = append(*classNotes, "Dust Cloud's effects are unsupported")
				continue
			}
			effects = append(effects, map[string]any{
				"kind": "hunter_pet_dust_cloud", "spell_id": row.ID, "aura": aura.Label, "armor": parsed.Applied[0].Value,
			})
		}
	}
	// The ability slots, as positions in the pet's spellbook, or -1 for an empty slot.
	slot := func(name string) int {
		field := reflect.ValueOf(pet).Elem().FieldByName(name)
		ptr := reflect.NewAt(field.Type(), unsafe.Pointer(field.UnsafeAddr())).Elem()
		if ptr.IsNil() {
			return -1
		}
		return hunterPetSpellPosition(pet, ptr.Interface().(*core.Spell))
	}
	effects = append(effects, map[string]any{
		"kind": "hunter_pet", "pet": pet.Label, "rotation": rotation,
		"special_ability": slot("specialAbility"), "focus_dump": slot("focusDump"), "extra_ability": slot("extraAbility"),
		"wait_ns": nanos(500 * time.Millisecond), "melee_range": core.MaxMeleeRange, "move_to": core.MaxMeleeRange - 1,
		// pet.go Reset: the share of the fight before ExecuteCustomRotation disables the pet.
		"uptime": min(1, max(0, h.Options.PetUptime)),
	})
	if aura := pet.GetAura("Intimidation"); aura != nil {
		for _, spell := range character.Spellbook {
			if spell.ActionID == aura.ActionID {
				effects = append(effects, map[string]any{
					"kind": "intimidation", "spell_id": spell.ActionID.SpellID, "aura": aura.Label,
					"crit_bonus": 100.0,
				})
			}
		}
	}
	if aura := pet.BestialWrathAura; aura != nil {
		effects = append(effects, map[string]any{
			"kind": "bestial_wrath", "spell_id": aura.ActionID.SpellID, "aura": aura.Label, "damage_multiplier": 1.5,
		})
	}
	if trigger := pet.GetAura("Frenzy"); trigger != nil {
		frenzy := pet.GetAura("Frenzy Effect")
		effects = append(effects, map[string]any{
			"kind": "frenzy", "trigger_aura": trigger.Label, "aura": frenzy.Label,
			"proc_chance": hunterFrenzy.FractionAt(h.Talents.Frenzy), "speed_multiplier": 1.3,
			"delay_ns": nanos(core.SpellBatchWindow),
		})
	}
	return effects
}

// The melee hunter: Aspect of the Beast, Raptor Strike, Mongoose Bite with Lacerating Strikes,
// Strider Kick, Wing Clip, Immolation Trap, Resourcefulness and Expose Prey.
func hunterMeleeEffects(h *hunter.Hunter, character *core.Character) []map[string]any {
	talents := h.Talents
	effects := []map[string]any{}
	// aspects.go: Aspect of the Beast's attack power is a stat aura; Deadly Aspects procs Quick
	// Strikes on landed melee white hits.
	if aura := h.AspectOfTheBeastAura; aura != nil && h.AspectOfTheBeast != nil {
		effect := map[string]any{"kind": "aspect_of_the_beast", "spell_id": h.AspectOfTheBeast.ActionID.SpellID, "aura": aura.Label}
		if talents.DeadlyAspects > 0 {
			rank := hunterQuickStrikes.Highest()
			effect["proc_aura"] = character.GetAura("Quick Strikes").Label
			effect["haste_multiplier"] = 1 + rank.Effect(dbcenums.A_MOD_MELEE_HASTE_3, 0).Average(core.CharacterLevel)/100
			effect["proc_chance"] = hunterDeadlyAspects.EffectAt(2).FractionAt(talents.DeadlyAspects)
		}
		effects = append(effects, effect)
	}
	// raptor_strike.go: the queue spell's aura makes the next main hand swing cast Raptor Strike,
	// whose hit adds the rank's flat damage to a main hand weapon swing.
	if h.RaptorStrike != nil && h.RaptorStrikeHit != nil {
		effects = append(effects, map[string]any{
			"kind": "raptor_strike", "spell_id": h.RaptorStrike.ActionID.SpellID,
			"queue_aura":  character.GetAura("Raptor Strike Queued").Label,
			"base_damage": hunterRaptorStrike.Highest().DamageEffect().Average(core.CharacterLevel),
			"melee_range": core.MaxMeleeRange,
		})
	}
	// mongoose_bite.go and lacerating_strikes.go.
	if h.MongooseBite != nil {
		effect := map[string]any{
			"kind": "mongoose_bite", "spell_id": h.MongooseBite.ActionID.SpellID, "aura": h.DefensiveState.Label,
			"base_damage": hunterMongooseBite.Highest().DamageEffect().Average(core.CharacterLevel),
		}
		if h.LaceratingStrikes != nil {
			effect["lacerating_share"] = 0.4
			effect["lacerating_tick_outcome"] = hunterTickOutcome(hunterLacerating.Highest())
		}
		effects = append(effects, effect)
	}
	if h.StriderKick != nil {
		effects = append(effects, map[string]any{"kind": "strider_kick", "spell_id": h.StriderKick.ActionID.SpellID})
	}
	if h.WingClip != nil {
		effects = append(effects, map[string]any{
			"kind": "wing_clip", "spell_id": h.WingClip.ActionID.SpellID,
			"base_damage": hunterWingClip.Highest().EffectN(2).Average(core.CharacterLevel),
		})
	}
	// traps.go: a magic hit roll without a hit count, dealt, then the dot when it landed.
	if h.ImmolationTrap != nil {
		rank := hunterImmolationTrap.Highest()
		effect := hunterImmolationEffect.Rank(rank.RankNumber())
		effects = append(effects, map[string]any{
			"kind": "immolation_trap", "spell_id": h.ImmolationTrap.ActionID.SpellID,
			"tick_base": effect.PeriodicEffect().Average(core.CharacterLevel),
		})
	}
	// talents_survival.go: Resourcefulness's crit trigger and its casting regeneration, and Expose
	// Prey's landed hits on a marked target opening the Mongoose Bite window.
	if trigger := character.GetAura("Resourcefulness Trigger"); trigger != nil {
		buff := hunterResourcefulBuff.Highest()
		effects = append(effects, map[string]any{
			"kind": "resourcefulness", "trigger_aura": trigger.Label, "aura": character.GetAura("Resourcefulness").Label,
			"proc_chance": 0.5 * float64(talents.Resourcefulness),
			"regen":       buff.Effect(dbcenums.A_MOD_MANA_REGEN_INTERRUPT, 0).Average(core.CharacterLevel) / 100,
		})
	}
	if trigger := character.GetAura("Expose Prey"); trigger != nil {
		marked := character.CurrentTarget.HasActiveAuraWithTag(buffs.HuntersMarkCategory)
		for _, aura := range character.CurrentTarget.GetAuras() {
			if aura.Tag == buffs.HuntersMarkCategory && aura.Duration != core.NeverExpires {
				*classNotes = append(*classNotes, "Expose Prey reads a Hunter's Mark that can change")
			}
		}
		effects = append(effects, map[string]any{
			"kind": "expose_prey", "trigger_aura": trigger.Label, "aura": h.DefensiveState.Label,
			"proc_chance": hunterExposePrey.FractionAt(talents.ExposePrey), "marked": marked,
		})
	}
	return effects
}
