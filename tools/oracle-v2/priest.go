// Priest export: class spell names, client damage rows, the Shadow effects Go keeps in
// closures and the Shadowfiend pet a build that never summons it keeps inert. Each formula
// mirrors the cited Go file at the pinned revision.
package main

import (
	"fmt"
	"math"

	"github.com/wowsims/forever/sim/core"
	"github.com/wowsims/forever/sim/core/dbcenums"
	"github.com/wowsims/forever/sim/core/proto"
	"github.com/wowsims/forever/sim/core/simsignals"
	"github.com/wowsims/forever/sim/core/spelldata"
	"github.com/wowsims/forever/sim/core/stats"
	"github.com/wowsims/forever/sim/priest"
)

func init() {
	classExports[proto.Class_ClassPriest] = classExport{
		spells: priestClassSpells, damageRows: priestDamageRows, effects: priestEffects, inertPet: priestInertPet,
		damageTakenModifiers: priestDamageTakenModifiers,
	}
	classSummonedPets = append(classSummonedPets, priestSummonedShadowfiend)
}

// talents_discipline.go applyPowerInfusion: the priest's own Power Infusion, a cooldown that
// activates the client-parsed Power Infusions aura, which multiplies the damage of the schools its
// mask names and healing dealt. The multipliers come from client data; a separate reset
// simulation checks they are all the aura changes, and that nothing else holds its category.
func priestPowerInfusionEffect(character *core.Character) map[string]any {
	note := func(condition bool, message string) {
		if condition {
			*classNotes = append(*classNotes, message)
		}
	}
	rank := priestPowerInfusion.Highest()
	damage := 1 + rank.Effect(dbcenums.A_MOD_DAMAGE_PERCENT_DONE, 126).Percent()
	healing := 1 + rank.Effect(dbcenums.A_MOD_HEALING_DONE_PERCENT, 126).Percent()
	aura := character.GetAuraByID(core.ActionID{SpellID: rank.ID})
	if aura == nil {
		note(true, "Power Infusion has no aura")
		return nil
	}
	for _, ee := range aura.ExclusiveEffects {
		note(privateField(ee.Category, "effects").Len() != 1, "Power Infusion shares its category")
	}
	simulation := core.NewSim(exportRequest, simsignals.CreateSignals())
	simulation.Reset()
	player := simulation.Raid.Parties[0].Players[0].GetCharacter()
	before := player.PseudoStats
	beforeStats := player.GetStats()
	player.GetAuraByID(core.ActionID{SpellID: rank.ID}).Activate(simulation)
	after := player.PseudoStats
	schools := []int{}
	for school := range before.SchoolDamageDealtMultiplier {
		if after.SchoolDamageDealtMultiplier[school] != before.SchoolDamageDealtMultiplier[school] {
			schools = append(schools, school)
			note(after.SchoolDamageDealtMultiplier[school] != before.SchoolDamageDealtMultiplier[school]*damage,
				"Power Infusion's school damage is not its client multiplier")
		}
	}
	note(after.HealingDealtMultiplier != before.HealingDealtMultiplier*healing,
		"Power Infusion's healing is not its client multiplier")
	after.SchoolDamageDealtMultiplier = before.SchoolDamageDealtMultiplier
	after.HealingDealtMultiplier = before.HealingDealtMultiplier
	note(after != before || player.GetStats() != beforeStats, "Power Infusion changes more than school damage and healing")
	return map[string]any{
		"kind": "power_infusion", "spell_id": rank.ID, "aura": aura.Label,
		"damage_multiplier": damage, "schools": schools, "healing_multiplier": healing,
	}
}

// shadowfiend.go: the Shadowfiend is summoned during a fight when the summon spell exists.
func priestSummonedShadowfiend(pet *core.Pet) bool {
	if pet.Name != "Shadowfiend" || pet.Owner == nil || pet.Owner.Class != proto.Class_ClassPriest {
		return false
	}
	return pet.Owner.GetSpell(core.ActionID{SpellID: priest.ShadowfiendRank.ID}) != nil
}

// shadowfiend.go and shadowfiend_pet.go: the summon enables the pet for its timeline aura's
// duration, and the pet inherits attack power from the priest's spell damage and shadow damage
// at each summon. Each enable activates the pet's mana restore aura, whose landed hits give
// the priest 5% of maximum mana.
func priestShadowfiendEffect(p *priest.Priest) map[string]any {
	note := func(condition bool, message string) {
		if condition {
			*classNotes = append(*classNotes, message)
		}
	}
	pet := &p.ShadowfiendPet.Pet
	// The inheritance: attack power from the sum of spell damage and shadow damage, bit for bit.
	inherit := petStatInheritance(pet)
	coefficient := inherit(stats.Stats{stats.SpellDamage: 1})[stats.AttackPower]
	for _, values := range [][2]float64{{1, 0}, {0, 1}, {3.7, 12.25}, {123.456, 0.1}, {812.3, 47.9}, {-12.5, 3}} {
		got := inherit(stats.Stats{stats.SpellDamage: values[0], stats.ShadowDamage: values[1]})
		want := stats.Stats{stats.AttackPower: (values[0] + values[1]) * coefficient}
		note(got != want, "the Shadowfiend's stat inheritance is not attack power from spell and shadow damage")
	}
	// The stats at a summon from a separate reset simulation, and the inherited stats line.
	simulation := core.NewSim(exportRequest, simsignals.CreateSignals())
	simulation.Reset()
	resetPriest := simulation.Raid.Parties[0].Players[0].(priest.PriestAgent).GetPriest()
	resetPet := resetPriest.ShadowfiendPet
	before := resetPet.GetStats()
	resetWithout := resetPet.GetStatsWithoutDeps()
	// Each attack power dependency adds a term that inheritance never changes.
	terms := []float64{}
	s := resetWithout
	for _, dep := range petDependencies(&resetPet.Pet) {
		note(dep.src == stats.AttackPower || (dep.src == dep.dst && dep.dst == stats.AttackPower),
			"the Shadowfiend's attack power feeds or scales a dependency")
		if dep.dst == stats.AttackPower {
			// Go adds the term to the attack power before it, as ApplyStatDependencies does.
			switch {
			case dep.step != 0:
				terms = append(terms, math.Floor(s[dep.src]/dep.step)*dep.step*dep.amount)
			case flooredGameStats[dep.src]:
				terms = append(terms, math.Floor(s[dep.src])*dep.amount)
			default:
				terms = append(terms, s[dep.src]*dep.amount)
			}
		}
		s = applyDependencies(s, []depTerm{dep})
	}
	owner := resetPriest.GetStats()
	resetPet.Enable(simulation, resetPet)
	after := resetPet.GetStats()
	// The inheritance Go stored, which Rust computes as the closure does. The conversion keeps
	// Go from fusing the product into a later sum.
	inherited := resetPet.GetInheritedStats()[stats.AttackPower]
	note(inherited != float64((owner[stats.SpellDamage]+owner[stats.ShadowDamage])*coefficient),
		"the Shadowfiend's inheritance at the summon is not attack power from spell and shadow damage")
	fold := func(base float64) float64 {
		for _, term := range terms {
			base += term
		}
		return base
	}
	note(fold(resetWithout[stats.AttackPower]) != before[stats.AttackPower],
		"the Shadowfiend's attack power does not follow its dependencies")
	note(fold(resetWithout[stats.AttackPower]+inherited) != after[stats.AttackPower],
		"the Shadowfiend's summoned attack power does not follow its inheritance")
	for stat := stats.Stat(0); stat < stats.SimStatsLen; stat++ {
		note(stat != stats.AttackPower && before[stat] != after[stat],
			fmt.Sprintf("the Shadowfiend's summon changes %s", stat.StatName()))
	}
	note(resetPet.ApplyStatDependencies(stats.Stats{stats.AttackPower: 123.25}) != stats.Stats{stats.AttackPower: 123.25},
		"the Shadowfiend's inherited stats line is not its attack power")
	// The pet's stats in Go's order, which its summon and dismissal lines print.
	order := []map[string]any{}
	for stat := stats.Stat(0); stat < stats.SimStatsLen; stat++ {
		if (before[stat] != 0 || stat == stats.AttackPower) && stat.StatName() != "none" {
			order = append(order, map[string]any{"stat": stat.StatName(), "value": before[stat]})
		}
	}
	restore := pet.GetAura("Shadowfiend Mana Restore")
	note(restore == nil, "the Shadowfiend has no mana restore aura")
	label := ""
	if restore != nil {
		label = restore.Label
	}
	return map[string]any{
		"kind": "shadowfiend", "spell_id": priest.ShadowfiendRank.ID, "aura": p.ShadowfiendAura.Label,
		"duration_ns": nanos(p.ShadowfiendAura.Duration), "pet": pet.Label,
		"attack_power_coefficient":  coefficient,
		"attack_power_without_deps": resetWithout[stats.AttackPower], "attack_power_dependency_terms": terms,
		"stats": order, "mana_restore_aura": label, "mana_restore_fraction": 0.05,
		"mana_restore_action_id": 401988,
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
	priestPowerInfusion          = spelldata.Ranked(10060)
	priestShadowform             = spelldata.Ranked(15473)
	priestHolyNovaHeal           = spelldata.Ranked(23455, 23458, 23459, 27803, 27804, 27805)
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
	// talents_holy.go registerHolyNovaSpell: every rank rolls its own row.
	priest.HolyNovaRankMap.Each(func(_ int32, row *spelldata.Spell) { rows[row.ID] = row })
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
	// talents_holy.go registerHolyNovaSpell: a rolled hit on each target, then the triggered heal
	// on each player of the priest's party, with the heal's own crit roll.
	if talents.HolyNova {
		ranks := []map[string]any{}
		priest.HolyNovaRankMap.Each(func(_ int32, row *spelldata.Spell) {
			heal := priestHolyNovaHeal.Rank(row.RankNumber())
			ranks = append(ranks, map[string]any{
				"spell_id": row.ID, "heal_spell_id": heal.ID, "heal_base": heal.HealEffect().Average(core.CharacterLevel),
			})
		})
		if len(character.Party.Players) != 1 {
			*classNotes = append(*classNotes, "Holy Nova heals a party of more than the priest")
		}
		if len(character.DynamicHealingTakenModifiers) != 0 {
			*classNotes = append(*classNotes, "dynamic healing taken modifiers on the priest are unsupported")
		}
		effects = append(effects, map[string]any{
			"kind": "holy_nova", "ranks": ranks,
			"healing_dealt_multiplier":       p.PseudoStats.HealingDealtMultiplier,
			"healing_taken_multiplier":       p.PseudoStats.HealingTakenMultiplier,
			"table_healing_dealt_multiplier": p.AttackTables[p.UnitIndex].HealingDealtMultiplier,
			"healing_power":                  p.GetStat(stats.HealingPower) + p.PseudoStats.BonusHealingTaken,
		})
	}
	if p.Talents.PowerInfusion {
		if effect := priestPowerInfusionEffect(character); effect != nil {
			effects = append(effects, effect)
		}
	}
	if p.Shadowfiend != nil {
		effects = append(effects, priestShadowfiendEffect(p))
	}
	// starshards.go, the Night Elf priest's racial: a hit roll, then a snapshotting channel.
	if len(spellsMatching(character, priest.PriestSpellStarshards)) > 0 {
		effects = append(effects, map[string]any{"kind": "starshards", "ranks": priestDotRanks(priest.StarshardsRankMap)})
	}
	if talents.MindFlay { // talents_shadow.go registerMindFlaySpell: a binary hit roll, then a channel
		effects = append(effects, map[string]any{"kind": "mind_flay", "ranks": priestDotRanks(priest.MindFlayRankMap)})
	}
	// talents_shadow.go applyShadowform. A helpful Holy cast no longer ends it (community #719); its
	// form refuses Holy Nova and Chastise through their cast requirements (#686).
	if aura := p.ShadowformAura; aura != nil {
		rank := priestShadowform.Highest()
		effects = append(effects, map[string]any{
			"kind": "shadowform", "spell_id": rank.ID, "aura": aura.Label,
			"damage_percent":  rank.Effect(dbcenums.A_MOD_DAMAGE_PERCENT_DONE, 32).Average(core.CharacterLevel) / 100,
			"cost_percent":    rank.Effect(dbcenums.A_MOD_POWER_COST_SCHOOL_PCT, 32).Average(core.CharacterLevel) / 100,
			"crit_multiplier": rank.Effect(dbcenums.A_ADD_PCT_MODIFIER, int32(dbcenums.SPELLMOD_CRIT_DAMAGE_BONUS)).Average(core.CharacterLevel) / 100,
			"school_spells":   priestModSpells(character, priest.PriestSpellsAll, core.SpellSchoolShadow),
			"crit_spells": priestModSpells(character, priest.PriestSpellMindBlast|priest.PriestSpellMindFlay|priest.PriestSpellShadowWordPain|
				priest.PriestSpellDevouringPlague|priest.PriestSpellShadowWordDeath, 0),
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
	// penance.go: every rank, on the shared category cooldown (community #678), fires its own
	// damage bolt, the first the cast's tooltip names, on application and a channel tick a second.
	if talents.Penance {
		ranks := []map[string]any{}
		priestPenance.Each(func(_ int32, rank *spelldata.Spell) {
			bolt := rank.Refs()[0]
			ranks = append(ranks, map[string]any{
				"spell_id": rank.ID, "tick_base": bolt.DamageEffect().Average(core.CharacterLevel), "tick_can_crit": true,
			})
		})
		effects = append(effects, map[string]any{"kind": "penance", "ranks": ranks})
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
	// dark_sacrifice.go: the Undead priest's racial, registered for Undead only. Each tick pays the
	// client base plus a fifth of Spirit; the cooldown manager uses it once the whole gain fits in
	// the mana bar.
	rank := priest.DarkSacrificeRank
	if darkSacrifice := character.GetSpell(core.ActionID{SpellID: rank.ID}); darkSacrifice != nil {
		effects = append(effects, map[string]any{
			"kind": "dark_sacrifice", "spell_id": rank.ID, "aura": darkSacrifice.SelfHot().Aura.Label,
			"tick_base": rank.ProcEnergizeEffect().Average(p.Level), "spirit_divisor": 5.0, "no_threat": rank.NoThreat(),
			"metrics_action_id": actionID(core.ActionID{SpellID: rank.ID}),
		})
	}
	return effects
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
