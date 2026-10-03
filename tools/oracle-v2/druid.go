// Druid export: class spell names, client damage rows and the Balance effects Go keeps in
// closures. Each formula mirrors the cited Go file at the pinned revision.
package main

import (
	"reflect"
	"time"

	"github.com/wowsims/forever/sim/core"
	"github.com/wowsims/forever/sim/core/dbcenums"
	"github.com/wowsims/forever/sim/core/proto"
	"github.com/wowsims/forever/sim/core/spelldata"
	"github.com/wowsims/forever/sim/druid"
)

func init() {
	classExports[proto.Class_ClassDruid] = classExport{spells: druidClassSpells, damageRows: druidDamageRows, effects: druidEffects}
}

var druidClassSpells = []classSpellName{
	{druid.DruidSpellEntanglingRoots, "entangling_roots"}, {druid.DruidSpellClaw, "claw"},
	{druid.DruidSpellDemoralizingRoar, "demoralizing_roar"}, {druid.DruidSpellFaerieFire, "faerie_fire"},
	{druid.DruidSpellFaerieFireFeral, "faerie_fire_feral"}, {druid.DruidSpellHurricane, "hurricane"},
	{druid.DruidSpellFerociousBite, "ferocious_bite"}, {druid.DruidSpellFrenziedRegeneration, "frenzied_regeneration"},
	{druid.DruidSpellInnervate, "innervate"}, {druid.DruidSpellInsectSwarm, "insect_swarm"},
	{druid.DruidSpellLacerate, "lacerate"}, {druid.DruidSpellPrimalBite, "primal_bite"},
	{druid.DruidSpellMaul, "maul"}, {druid.DruidSpellMoonfireInitial, "moonfire"},
	{druid.DruidSpellMoonfireDoT, "moonfire_dot"}, {druid.DruidSpellRake, "rake"},
	{druid.DruidSpellRavage, "ravage"}, {druid.DruidSpellRip, "rip"}, {druid.DruidSpellShred, "shred"},
	{druid.DruidSpellStarfire, "starfire"}, {druid.DruidSpellSwipe, "swipe"}, {druid.DruidSpellThorns, "thorns"},
	{druid.DruidSpellWrath, "wrath"}, {druid.DruidSpellEnrage, "enrage"},
	{druid.DruidSpellShiftingPower, "shifting_power"}, {druid.DruidSpellCatForm, "cat_form"},
	{druid.DruidSpellBearForm, "bear_form"}, {druid.DruidSpellMoonkinForm, "moonkin_form"},
	{druid.DruidSpellHealingTouch, "healing_touch"}, {druid.DruidSpellRegrowth, "regrowth"},
	{druid.DruidSpellLifebloom, "lifebloom"}, {druid.DruidSpellRejuvenation, "rejuvenation"},
	{druid.DruidSpellTranquility, "tranquility"}, {druid.DruidSpellMarkOfTheWild, "mark_of_the_wild"},
	{druid.DruidSpellSwiftmend, "swiftmend"}, {druid.DruidSpellCenarionWard, "cenarion_ward"},
	{druid.DruidSpellRevive, "revive"},
}

var (
	starfireLadder      = spelldata.Ranked(2912, 8949, 8950, 8951, 9875, 9876, 25298)
	wrathLadder         = spelldata.Ranked(5176, 5177, 5178, 5179, 5180, 6780, 8905, 9912)
	moonfireLadder      = spelldata.Ranked(8921, 8924, 8925, 8926, 8927, 8928, 8929, 9833, 9834, 9835)
	insectSwarmLadder   = spelldata.Ranked(5570, 24974, 24975, 24976, 24977)
	innervateLadder     = spelldata.Ranked(29166)
	moonkinFormLadder   = spelldata.Ranked(24858)
	omenOfClarity       = spelldata.Ranked(16864)
	omenClearcasting    = spelldata.Ranked(16870)
	naturesGraceTrigger = spelldata.Ranked(16886)
	eclipseTalent       = spelldata.Talent(408248, 3)
	eclipseTriggered    = spelldata.Ranked(408255)
)

// starfire.go and wrath.go register every rank; moonfire.go only the highest.
func druidDamageRows(rows map[int32]*spelldata.Spell) {
	starfireLadder.Each(func(_ int32, row *spelldata.Spell) { rows[row.ID] = row })
	wrathLadder.Each(func(_ int32, row *spelldata.Spell) { rows[row.ID] = row })
	if moonfire := moonfireLadder.Highest(); moonfire != nil {
		rows[moonfire.ID] = moonfire
	}
}

// Every DruidSpell field of the druid, by spell, with the forms druid.RegisterSpell lets it
// be cast in. Spells registered with Unit.RegisterSpell have no form check.
func druidFormMasks(d *druid.Druid) map[*core.Spell]druid.DruidForm {
	masks := map[*core.Spell]druid.DruidForm{}
	record := func(value reflect.Value) {
		if value.IsNil() {
			return
		}
		spell := value.Interface().(*druid.DruidSpell)
		masks[spell.Spell] = spell.FormMask
	}
	value := reflect.ValueOf(d).Elem()
	spellType := reflect.TypeOf(&druid.DruidSpell{})
	for i := 0; i < value.NumField(); i++ {
		field := value.Field(i)
		switch {
		case field.Type() == spellType:
			record(field)
		case field.Kind() == reflect.Slice && field.Type().Elem() == spellType:
			for j := 0; j < field.Len(); j++ {
				record(field.Index(j))
			}
		}
	}
	return masks
}

func formNames(form druid.DruidForm) []string {
	names := []string{}
	for _, entry := range []struct {
		form druid.DruidForm
		name string
	}{{druid.Humanoid, "humanoid"}, {druid.Bear, "bear"}, {druid.Cat, "cat"}, {druid.Moonkin, "moonkin"}, {druid.Tree, "tree"}} {
		if form.Matches(entry.form) {
			names = append(names, entry.name)
		}
	}
	return names
}

// The spellbook positions of spells whose class mask is in a set.
func spellsMatching(character *core.Character, mask int64) []int {
	positions := []int{}
	for i, spell := range character.Spellbook {
		if spell.Matches(mask) {
			positions = append(positions, i)
		}
	}
	return positions
}

// The spellbook positions of spells a proc trigger listens to, by core ProcTrigger.matchesSpell
// at the pinned revision; every check reads a static spell property.
func procTriggerSpells(character *core.Character, trigger core.ProcTrigger) []int {
	positions := []int{}
	for i, spell := range character.Spellbook {
		eligible := trigger.CanProcFromProcs || !spell.Flags.Matches(core.SpellFlagProc)
		if trigger.IsWeaponProc {
			eligible = !spell.Flags.Matches(core.SpellFlagSuppressWeaponProcs)
		}
		switch {
		case !eligible:
		case trigger.SpellFlags != core.SpellFlagNone && !spell.Flags.Matches(trigger.SpellFlags):
		case trigger.SpellFlagsExclude != core.SpellFlagNone && spell.Flags.Matches(trigger.SpellFlagsExclude):
		case trigger.ClassSpellMask > 0 && trigger.ClassSpellMask&spell.ClassSpellMask == 0:
		case !trigger.ClassFlags.IsZero() && !spell.MatchesFlags(trigger.ClassFlags):
		case trigger.ClassSpellsOnly && spell.ClassSpellMask == 0 && spell.ClassFlags.IsZero():
		case trigger.ProcMaskExclude != core.ProcMaskUnknown && spell.ProcMask.Matches(trigger.ProcMaskExclude):
		case trigger.ProcMask != core.ProcMaskUnknown && !spell.ProcMask.Matches(trigger.ProcMask):
		default:
			positions = append(positions, i)
		}
	}
	return positions
}

func callbackNames(callback core.AuraCallback) []string {
	names := []string{}
	for _, entry := range []struct {
		callback core.AuraCallback
		name     string
	}{
		{core.CallbackOnSpellHitDealt, "on_spell_hit_dealt"}, {core.CallbackOnSpellHitTaken, "on_spell_hit_taken"},
		{core.CallbackOnPeriodicDamageDealt, "on_periodic_damage_dealt"}, {core.CallbackOnHealDealt, "on_heal_dealt"},
		{core.CallbackOnPeriodicHealDealt, "on_periodic_heal_dealt"}, {core.CallbackOnCastComplete, "on_cast_complete"},
		{core.CallbackOnApplyEffects, "on_apply_effects"}, {core.CallbackOnPeriodicDamageTaken, "on_periodic_damage_taken"},
	} {
		if callback.Matches(entry.callback) {
			names = append(names, entry.name)
		}
	}
	return names
}

func outcomeNames(outcome core.HitOutcome) []string {
	names := []string{}
	for bit := 0; bit < 32; bit++ {
		flag := core.HitOutcome(uint32(1) << bit)
		if outcome&flag != 0 {
			names = append(names, flag.String())
		}
	}
	return names
}

func periodicRank(row *spelldata.Spell) map[string]any {
	return map[string]any{
		"spell_id": row.ID, "tick_base": row.PeriodicEffect().Average(core.CharacterLevel),
		"tick_can_crit": row.PeriodicCanCrit() && row.DefenseTypeCore() == core.DefenseTypeMagic,
	}
}

func druidEffects(agent core.Agent, character *core.Character) []map[string]any {
	d := agent.(druid.DruidAgent).GetDruid()
	talents := d.Talents
	effects := []map[string]any{}
	unit := &character.Unit

	// druid.go RegisterSpell: the forms each spell may be cast in. forms.go: the form the druid
	// starts each fight in.
	masks := druidFormMasks(d)
	forms := []map[string]any{}
	for i, spell := range character.Spellbook {
		if mask, ok := masks[spell]; ok {
			forms = append(forms, map[string]any{"spell": i, "forms": formNames(mask)})
		}
	}
	effects = append(effects, map[string]any{"kind": "druid_forms", "starting_form": formNames(d.StartingForm), "spells": forms})
	if aura := unit.GetAura("Moonkin Form"); aura != nil { // forms.go RegisterMoonkinFormSpell
		effects = append(effects, map[string]any{"kind": "moonkin_form", "spell_id": moonkinFormLadder.Highest().ID, "aura": aura.Label})
	}
	// starfire.go and wrath.go: a direct hit, Wrath's after travel.
	effects = append(effects, map[string]any{"kind": "starfire"}, map[string]any{"kind": "wrath"})
	// moonfire.go: the hit casts the tagged dot spell when it lands.
	moonfire := moonfireLadder.Highest()
	effects = append(effects, map[string]any{"kind": "moonfire", "rank": periodicRank(moonfire)})
	if talents.InsectSwarm { // insect_swarm.go: a binary hit roll, then the dot and its debuff
		rank := insectSwarmLadder.Highest()
		target := character.Env.Encounter.ActiveTargetUnits[0]
		dotLabel := d.InsectSwarm.Dot(target).Aura.Label
		debuff := ""
		for _, aura := range target.GetAuras() {
			if aura.ActionID == (core.ActionID{SpellID: rank.ID}) && aura.Label != dotLabel {
				debuff = aura.Label
			}
		}
		effects = append(effects, map[string]any{"kind": "insect_swarm", "rank": periodicRank(rank), "debuff_aura": debuff})
	}
	// innervate.go and buffs/drivers.go AttachInnervateRegen: Go literals.
	if aura := unit.GetAura("Innervates (Player)"); aura != nil {
		regen := aura.ActionID
		regen.Tag = -2
		effects = append(effects, map[string]any{
			"kind": "innervate", "spell_id": innervateLadder.Highest().ID, "aura": aura.Label,
			"spirit_regen_multiplier": 5.0, "regen_metrics_action_id": actionID(regen),
		})
	}
	// omen_of_clarity.go: the trigger's spells, outcome and cooldown come from the client row as
	// spelldata.ProcTrigger resolves them; the chance is the cast time share of two a minute.
	clearcasting := omenClearcasting.Highest()
	omen := omenOfClarity.Highest()
	moonkin := moonkinFormLadder.Highest()
	trigger := spelldata.ProcTrigger(character, omen, nil, spelldata.Chance(1))
	effects = append(effects, map[string]any{
		"kind": "omen_of_clarity", "trigger_aura": trigger.Name, "aura": "Clearcasting",
		"callbacks": callbackNames(trigger.Callback), "outcome": outcomeNames(trigger.Outcome),
		"require_damage_dealt": trigger.RequireDamageDealt, "trigger_spells": procTriggerSpells(character, trigger),
		"trigger_immediately": trigger.TriggerImmediately, "proc_chance": trigger.ProcChance,
		"icd_ns": nanos(omen.ICD()), "ppm": 2.0, "gcd_ns": nanos(core.GCDDefault),
		"moonkin_chance_multiplier":   1 + moonkin.Effect(dbcenums.A_ADD_PCT_MODIFIER, int32(dbcenums.SPELLMOD_CHANCE_OF_SUCCESS)).Percent(),
		"moonkin_cooldown_multiplier": 1 + moonkin.Effect(dbcenums.A_ADD_PCT_MODIFIER, int32(dbcenums.SPELLMOD_PROC_COOLDOWN)).Percent(),
		"cost_spells":                 spellsMatching(character, clearcastingMask),
		"cost_percent_add":            clearcasting.Effect(dbcenums.A_ADD_PCT_MODIFIER, int32(dbcenums.SPELLMOD_COST)).Percent(),
	})
	if talents.NaturesGrace { // talents_balance.go applyNaturesGrace
		triggered := naturesGraceTrigger.Highest()
		effects = append(effects, map[string]any{
			"kind": "natures_grace", "trigger_aura": "Nature's Grace Trigger", "aura": "Nature's Grace",
			"haste_multiplier": 1 + triggered.Effect(dbcenums.A_MOD_CASTING_SPEED_NOT_STACK, 0).BaseValue()/100,
			"gcd_reduction_ns": nanos(time.Duration(float64(core.GCDDefault) * triggered.Effect(dbcenums.A_ADD_PCT_MODIFIER, int32(dbcenums.SPELLMOD_GLOBAL_COOLDOWN)).Percent())),
			"gcd_spells": spellsMatching(character, druid.DruidSpellEntanglingRoots|druid.DruidSpellFaerieFire|druid.DruidSpellHurricane|
				druid.DruidSpellInsectSwarm|druid.DruidSpellMoonfire|druid.DruidSpellStarfire|druid.DruidSpellThorns|druid.DruidSpellWrath|
				druid.DruidSpellHealingTouch|druid.DruidSpellRegrowth|druid.DruidSpellRejuvenation|druid.DruidSpellTranquility|druid.DruidSpellMarkOfTheWild),
			"trigger_spells": procTriggerSpells(character, core.ProcTrigger{ClassSpellMask: druid.DruidDamagingSpells}),
		})
	}
	if talents.Eclipse > 0 { // talents_balance.go applyEclipse: a Go literal of two charges a Wrath
		effects = append(effects, map[string]any{
			"kind": "eclipse", "trigger_aura": "Eclipse Trigger", "aura": "Eclipse",
			"cast_time_reduction_ns": nanos(time.Millisecond * time.Duration(eclipseTalent.EffectAt(2).ValueAt(talents.Eclipse))),
			"charges_per_wrath":      int32(2), "duration_ns": nanos(eclipseTriggered.Highest().Duration()),
		})
	}
	return effects
}

// omen_of_clarity.go clearcastingSpells.
const clearcastingMask = druid.DruidSpellClaw | druid.DruidSpellEntanglingRoots | druid.DruidSpellDemoralizingRoar | druid.DruidSpellHurricane |
	druid.DruidSpellFerociousBite | druid.DruidSpellInsectSwarm | druid.DruidSpellLacerate | druid.DruidSpellPrimalBite | druid.DruidSpellMaul |
	druid.DruidSpellMoonfire | druid.DruidSpellRake | druid.DruidSpellRavage | druid.DruidSpellRip | druid.DruidSpellShred | druid.DruidSpellStarfire |
	druid.DruidSpellSwipe | druid.DruidSpellThorns | druid.DruidSpellHealingTouch | druid.DruidSpellRegrowth | druid.DruidSpellLifebloom |
	druid.DruidSpellRejuvenation | druid.DruidSpellTranquility | druid.DruidSpellSwiftmend
