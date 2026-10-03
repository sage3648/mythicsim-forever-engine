// Priest export: class spell names, client damage rows, the Shadow effects Go keeps in
// closures and the Shadowfiend pet a build that never summons it keeps inert. Each formula
// mirrors the cited Go file at the pinned revision.
package main

import (
	"github.com/wowsims/forever/sim/core"
	"github.com/wowsims/forever/sim/core/dbcenums"
	"github.com/wowsims/forever/sim/core/proto"
	"github.com/wowsims/forever/sim/core/spelldata"
	"github.com/wowsims/forever/sim/priest"
)

func init() {
	classExports[proto.Class_ClassPriest] = classExport{
		spells: priestClassSpells, damageRows: priestDamageRows, effects: priestEffects, inertPet: priestInertPet,
		damageTakenModifiers: priestDamageTakenModifiers,
	}
}

var priestClassSpells = []classSpellName{
	{priest.PriestSpellDevouringPlague, "devouring_plague"}, {priest.PriestSpellDevouringPlagueDoT, "devouring_plague_dot"},
	{priest.PriestSpellDevouringPlagueHeal, "devouring_plague_heal"}, {priest.PriestSpellHolyNova, "holy_nova"},
	{priest.PriestSpellHolyFire, "holy_fire"}, {priest.PriestSpellMindBlast, "mind_blast"},
	{priest.PriestSpellMindFlay, "mind_flay"}, {priest.PriestSpellPenance, "penance"},
	{priest.PriestSpellPowerInfusion, "power_infusion"}, {priest.PriestSpellStarshards, "starshards"},
	{priest.PriestSpellShadowform, "shadowform"}, {priest.PriestSpellShadowWordDeath, "shadow_word_death"},
	{priest.PriestSpellShadowWordPain, "shadow_word_pain"}, {priest.PriestSpellShadowFiend, "shadowfiend"},
	{priest.PriestSpellVampiricEmbrace, "vampiric_embrace"}, {priest.PriestSpellFade, "fade"},
	{priest.PriestSpellSmite, "smite"}, {priest.PriestSpellChastise, "chastise"},
	{priest.PriestSpellConfoundingFlash, "confounding_flash"}, {priest.PriestSpellContingencyPlan, "contingency_plan"},
	{priest.PriestSpellDarkSacrifice, "dark_sacrifice"}, {priest.PriestSpellDivineGrace, "divine_grace"},
}

var (
	priestShadowWeaving          = spelldata.Talent(15257, 3)
	priestShadowWeavingTriggered = spelldata.Ranked(15258)
	priestInnerFocus             = spelldata.Ranked(14751)
	priestShadowform             = spelldata.Ranked(15473)
	priestEarlyDemise            = spelldata.Talent(1310076, 2)
	priestPenance                = spelldata.Ranked(402174, 1240720, 1240721, 1316995)
	priestPowerInLight           = spelldata.Talent(1309969, 5)
	priestSearingLight           = spelldata.Talent(14909, 2)
	priestSearingLightTriggered  = spelldata.Ranked(1284536)
)

// mind_blast.go and shadow_word_death.go register every rank, each rolling its own row.
func priestDamageRows(rows map[int32]*spelldata.Spell) {
	priest.MindBlastRankMap.Each(func(_ int32, row *spelldata.Spell) { rows[row.ID] = row })
	priest.ShadowWordDeathRankMap.Each(func(_ int32, row *spelldata.Spell) { rows[row.ID] = row })
	// smite.go and holy_fire.go: every rank rolls its own row.
	priest.SmiteRankMap.Each(func(_ int32, row *spelldata.Spell) { rows[row.ID] = row })
	priest.HolyFireRankMap.Each(func(_ int32, row *spelldata.Spell) { rows[row.ID] = row })
}

// Dynamic damage taken modifiers the priest effects describe: Power in Light registers one on
// every target (talents_discipline.go applyPowerInLight).
func priestDamageTakenModifiers(agent core.Agent) int {
	if agent.(priest.PriestAgent).GetPriest().Talents.PowerInLight > 0 {
		return 1
	}
	return 0
}

// The spellbook positions a spell modifier with a class mask and school applies to, by core
// shouldApply at the pinned revision for the fields priest modifiers set.
func priestModSpells(character *core.Character, mask int64, school core.SpellSchool) []int {
	positions := []int{}
	for i, spell := range character.Spellbook {
		switch {
		case spell.Flags.Matches(core.SpellFlagNoSpellMods):
		case mask > 0 && !spell.Matches(mask):
		case school > 0 && !school.Matches(spell.SpellSchool):
		default:
			positions = append(positions, i)
		}
	}
	return positions
}

// A priest dot rank: the base its ticks snapshot and whether priestTickOutcome rolls a crit,
// which reads only the client's Periodic Can Crit attribute.
func priestDotRank(row *spelldata.Spell) map[string]any {
	return map[string]any{
		"spell_id": row.ID, "tick_base": row.PeriodicEffect().Average(core.CharacterLevel), "tick_can_crit": row.PeriodicCanCrit(),
	}
}

func priestDotRanks(ladder spelldata.Ladder) []map[string]any {
	ranks := []map[string]any{}
	ladder.Each(func(_ int32, row *spelldata.Spell) { ranks = append(ranks, priestDotRank(row)) })
	return ranks
}

func priestEffects(agent core.Agent, character *core.Character) []map[string]any {
	p := agent.(priest.PriestAgent).GetPriest()
	talents := p.Talents
	effects := []map[string]any{}

	// mind_blast.go: a direct hit on the rank's own row.
	effects = append(effects, map[string]any{"kind": "mind_blast"})
	// shadow_word_death.go: Early Demise adds crit inside the 20% execute phase.
	effects = append(effects, map[string]any{
		"kind": "shadow_word_death", "early_demise_crit": priestEarlyDemise.EffectAt(1).ValueAt(talents.EarlyDemise),
	})
	// shadow_word_pain.go: a hit roll, then a snapshotting dot.
	effects = append(effects, map[string]any{"kind": "shadow_word_pain", "ranks": priestDotRanks(priest.ShadowWordPainRankMap)})
	// devouring_plague.go: Shadow Word: Pain's shape; each tick heals the priest for its damage.
	effects = append(effects, map[string]any{
		"kind": "devouring_plague", "ranks": priestDotRanks(priest.DevouringPlagueRankMap), "heal_metrics_tag": int32(1),
	})
	if talents.MindFlay { // talents_shadow.go registerMindFlaySpell: a binary hit roll, then a channel
		effects = append(effects, map[string]any{"kind": "mind_flay", "ranks": priestDotRanks(priest.MindFlayRankMap)})
	}
	if aura := p.ShadowformAura; aura != nil { // talents_shadow.go applyShadowform
		rank := priestShadowform.Highest()
		effects = append(effects, map[string]any{
			"kind": "shadowform", "spell_id": rank.ID, "aura": aura.Label,
			"damage_percent":  rank.Effect(dbcenums.A_MOD_DAMAGE_PERCENT_DONE, 32).Average(core.CharacterLevel) / 100,
			"cost_percent":    rank.Effect(dbcenums.A_MOD_POWER_COST_SCHOOL_PCT, 32).Average(core.CharacterLevel) / 100,
			"crit_multiplier": rank.Effect(dbcenums.A_ADD_PCT_MODIFIER, int32(dbcenums.SPELLMOD_CRIT_DAMAGE_BONUS)).Average(core.CharacterLevel) / 100,
			"school_spells":   priestModSpells(character, priest.PriestSpellsAll, core.SpellSchoolShadow),
			"crit_spells": priestModSpells(character, priest.PriestSpellMindBlast|priest.PriestSpellMindFlay|priest.PriestSpellShadowWordPain|
				priest.PriestSpellDevouringPlague|priest.PriestSpellShadowWordDeath, 0),
			"cancel_spells": priestShadowformCancels(character),
		})
	}
	if aura := p.InnerFocusAura; aura != nil { // talents_discipline.go applyInnerFocus
		rank := priestInnerFocus.Highest()
		spenders := []int{}
		for i, spell := range character.Spellbook {
			if spell.Matches(priest.PriestSpellsAll) {
				spenders = append(spenders, i)
			}
		}
		effects = append(effects, map[string]any{
			"kind": "inner_focus", "spell_id": rank.ID, "aura": aura.Label,
			"cost_percent": int32(rank.Effect(dbcenums.A_ADD_PCT_MODIFIER, int32(dbcenums.SPELLMOD_COST)).Average(core.CharacterLevel)),
			"crit_percent": rank.Effect(dbcenums.A_ADD_FLAT_MODIFIER, int32(dbcenums.SPELLMOD_CRITICAL_CHANCE)).Average(core.CharacterLevel),
			"crit_spells": priestModSpells(character,
				priest.PriestSpellsAll&^(priest.PriestSpellShadowWordDeath|priest.PriestSpellDevouringPlague|priest.PriestSpellShadowWordPain), 0),
			"spender_spells": spenders,
		})
	}
	if aura := p.ShadowWeavingAura; aura != nil { // talents_shadow.go applyShadowWeaving
		stack := priestShadowWeavingTriggered.Highest()
		trigger := core.ProcTrigger{
			Name: "Shadow Weaving Trigger", Callback: core.CallbackOnSpellHitDealt, ClassSpellMask: priest.PriestShadowSpells,
			Outcome: core.OutcomeLanded, ProcChance: priestShadowWeaving.FractionAt(talents.ShadowWeaving), TriggerImmediately: true,
		}
		effects = append(effects, map[string]any{
			"kind": "shadow_weaving", "trigger_aura": trigger.Name, "aura": aura.Label,
			"callbacks": callbackNames(trigger.Callback), "outcome": outcomeNames(trigger.Outcome),
			"trigger_immediately": trigger.TriggerImmediately, "proc_chance": trigger.ProcChance,
			"trigger_spells":   procTriggerSpells(character, trigger),
			"damage_per_stack": stack.Effect(dbcenums.A_MOD_SCHOOL_MASK_DAMAGE_FROM_CASTER, 32).Average(core.CharacterLevel) / 100,
			"damage_spells":    priestModSpells(character, priest.PriestSpellsAll, core.SpellSchoolShadow),
		})
	}
	// smite.go: a cast on the rank's own row.
	effects = append(effects, map[string]any{"kind": "smite"})
	// holy_fire.go: the hit rolls, a landed hit applies the snapshotting dot, then it is dealt.
	effects = append(effects, map[string]any{"kind": "holy_fire", "ranks": priestDotRanks(priest.HolyFireRankMap)})
	if talents.Penance { // penance.go: the bolt on application and a channel tick a second
		rank := priestPenance.Highest()
		bolt := spelldata.Find(1316993)
		effects = append(effects, map[string]any{
			"kind": "penance", "spell_id": rank.ID, "tick_base": bolt.DamageEffect().Average(core.CharacterLevel), "tick_can_crit": true,
		})
	}
	if talents.PowerInLight > 0 { // talents_discipline.go applyPowerInLight
		holyFire := []int{}
		for i, spell := range character.Spellbook {
			for _, known := range p.HolyFire {
				if spell == known {
					holyFire = append(holyFire, i)
				}
			}
		}
		effects = append(effects, map[string]any{
			"kind": "power_in_light", "multiplier": priestPowerInLight.MultiplierAt(talents.PowerInLight),
			"spells":           spellsMatching(character, priest.PriestSpellSmite|priest.PriestSpellPenance),
			"holy_fire_spells": holyFire,
		})
	}
	if aura := p.SearingLightAura; aura != nil { // talents_holy.go applySearingLight
		free := priestSearingLightTriggered.Highest()
		trigger := core.ProcTrigger{
			Name: "Searing Light Trigger", Callback: core.CallbackOnPeriodicDamageDealt, ClassSpellMask: priest.PriestSpellHolyFire,
			ProcChance: priestSearingLight.EffectAt(2).FractionAt(talents.SearingLight), TriggerImmediately: true,
		}
		effects = append(effects, map[string]any{
			"kind": "searing_light", "trigger_aura": trigger.Name, "aura": aura.Label,
			"callbacks": callbackNames(trigger.Callback), "outcome": outcomeNames(trigger.Outcome),
			"trigger_immediately": trigger.TriggerImmediately, "proc_chance": trigger.ProcChance,
			"trigger_spells":   procTriggerSpells(character, trigger),
			"cost_percent_add": free.Effect(dbcenums.A_ADD_PCT_MODIFIER, int32(dbcenums.SPELLMOD_COST)).Average(core.CharacterLevel) / 100,
			"cost_spells":      priestModSpells(character, priest.PriestSpellHolyNova, 0),
			"cancel_spells":    spellsMatching(character, priest.PriestSpellHolyNova),
		})
	}
	// dark_sacrifice.go: each tick pays the client base plus a fifth of Spirit; the cooldown
	// manager uses it once the whole gain fits in the mana bar.
	rank := priest.DarkSacrificeRank
	effects = append(effects, map[string]any{
		"kind": "dark_sacrifice", "spell_id": rank.ID, "aura": character.GetSpell(core.ActionID{SpellID: rank.ID}).SelfHot().Aura.Label,
		"tick_base": rank.ProcEnergizeEffect().Average(p.Level), "spirit_divisor": 5.0,
		"metrics_action_id": actionID(core.ActionID{SpellID: rank.ID}),
	})
	return effects
}

// Shadowform's OnCastComplete ends it on a helpful Holy cast.
func priestShadowformCancels(character *core.Character) []int {
	positions := []int{}
	for i, spell := range character.Spellbook {
		if spell.SpellSchool.Matches(core.SpellSchoolHoly) && spell.Flags.Matches(core.SpellFlagHelpful) {
			positions = append(positions, i)
		}
	}
	return positions
}

// shadowfiend_pet.go: every priest registers the Shadowfiend pet, but shadowfiend.go only
// registers the spell that enables it when the class options ask for it. Without that spell
// nothing summons the pet: it is never enabled on start, its auto attacks and auras never
// run, and Go only resets, dismisses and reports it.
func priestInertPet(agent core.Agent, pet *core.Pet) string {
	p := agent.(priest.PriestAgent).GetPriest()
	if pet != &p.ShadowfiendPet.Pet || p.Shadowfiend != nil || pet.EnabledOnStart() {
		return ""
	}
	return "no registered spell summons the Shadowfiend"
}
