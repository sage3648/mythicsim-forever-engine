// Warlock export: class spell names, client damage rows and the effects Go keeps in closures.
package main

import (
	"fmt"

	"github.com/wowsims/forever/sim/core"
	"github.com/wowsims/forever/sim/core/dbcenums"
	"github.com/wowsims/forever/sim/core/proto"
	"github.com/wowsims/forever/sim/core/simsignals"
	"github.com/wowsims/forever/sim/core/spelldata"
	"github.com/wowsims/forever/sim/core/stats"
	"github.com/wowsims/forever/sim/warlock"
)

func init() {
	classExports[proto.Class_ClassWarlock] = classExport{
		spells: warlockClassSpells, damageRows: warlockDamageRows, effects: warlockEffects,
		damageTakenModifiers: warlockDamageTakenModifiers,
	}
}

// Warlock class masks are Go-internal bit positions, exported by stable name.
var warlockClassSpells = []classSpellName{
	{warlock.WarlockSpellConflagrate, "conflagrate"}, {warlock.WarlockSpellShadowBolt, "shadow_bolt"},
	{warlock.WarlockSpellImmolate, "immolate"}, {warlock.WarlockSpellImmolateDot, "immolate_dot"},
	{warlock.WarlockSpellIncinerate, "incinerate"}, {warlock.WarlockSpellSoulFire, "soul_fire"},
	{warlock.WarlockSpellShadowBurn, "shadowburn"}, {warlock.WarlockSpellLifeTap, "life_tap"},
	{warlock.WarlockSpellCorruption, "corruption"}, {warlock.WarlockSpellCurseOfAgony, "bane_of_agony"},
	{warlock.WarlockSpellCurseOfElements, "curse_of_the_elements"}, {warlock.WarlockSpellDrainLife, "drain_life"},
	{warlock.WarlockSpellHellfire, "hellfire"}, {warlock.WarlockSpellImmolationAura, "immolation_aura"},
	{warlock.WarlockSpellSearingPain, "searing_pain"}, {warlock.WarlockSpellSummonDoomguard, "summon_doomguard"},
	{warlock.WarlockSpellDoomguardDoomBolt, "doomguard_doom_bolt"}, {warlock.WarlockSpellSummonImp, "summon_imp"},
	{warlock.WarlockSpellImpFireBolt, "imp_firebolt"}, {warlock.WarlockSpellSummonFelhunter, "summon_felhunter"},
	{warlock.WarlockSpellFelHunterShadowBite, "felhunter_shadow_bite"}, {warlock.WarlockSpellSummonSuccubus, "summon_succubus"},
	{warlock.WarlockSpellSuccubusLashOfPain, "succubus_lash_of_pain"}, {warlock.WarlockSpellVoidwalkerTorment, "voidwalker_torment"},
	{warlock.WarlockSpellSummonInfernal, "summon_infernal"}, {warlock.WarlockSpellRainOfFire, "rain_of_fire"},
	{warlock.WarlockSpellCurseOfDoom, "bane_of_doom"}, {warlock.WarlockSpellCurseOfRecklessness, "curse_of_recklessness"},
	{warlock.WarlockSpellCurseOfWeakness, "curse_of_weakness"}, {warlock.WarlockSpellSiphonLife, "siphon_life"},
	{warlock.WarlockSpellDrainSoul, "drain_soul"}, {warlock.WarlockSpellDeathCoil, "death_coil"},
	{warlock.WarlockSpellWrack, "wrack"},
}

// The ladders sim/warlock/spell_data_auto_gen.go names, for the spells and talents exported here.
var (
	wlShadowBoltLadder     = spelldata.Ranked(686, 695, 705, 1088, 1106, 7641, 11659, 11660, 11661, 25307)
	wlImmolateLadder       = spelldata.Ranked(348, 707, 1094, 2941, 11665, 11667, 11668, 25309)
	wlCorruptionLadder     = spelldata.Ranked(172, 6222, 6223, 7648, 11671, 11672, 25311)
	wlBaneOfAgonyLadder    = spelldata.Ranked(980, 1014, 6217, 11711, 11712, 11713)
	wlCurseOfElements      = spelldata.Ranked(440892, 1311676, 1311677, 1311680)
	wlConflagrateLadder    = spelldata.Ranked(1293817, 1293818, 17962, 18930, 18931, 18932)
	wlShadowburnLadder     = spelldata.Ranked(17877, 18867, 18868, 18869, 18870, 18871)
	wlSearingPainLadder    = spelldata.Ranked(5676, 17919, 17920, 17921, 17922, 17923)
	wlSoulFireLadder       = spelldata.Ranked(6353, 17924)
	wlLifeTapLadder        = spelldata.Ranked(1454, 1455, 1456, 11687, 11688, 11689)
	wlImprovedLifeTap      = spelldata.Talent(18182, 2)
	wlImprovedShadowBolt   = spelldata.Talent(17793, 5)
	wlShadowAndFlame       = spelldata.Talent(426316, 5)
	wlShadowAndFlameRows   = spelldata.Ranked(426311, 1293816)
	wlAmplifyCurse         = spelldata.Ranked(18288)
	wlImprovedShadowBoltOn = spelldata.Ranked(17794)
)

func warlockDamageRows(rows map[int32]*spelldata.Spell) {
	wlShadowBoltLadder.Each(func(_ int32, row *spelldata.Spell) { rows[row.ID] = row })
	for _, ladder := range []spelldata.Ladder{wlImmolateLadder, wlConflagrateLadder, wlShadowburnLadder, wlSearingPainLadder, wlSoulFireLadder} {
		if row := ladder.Highest(); row != nil {
			rows[row.ID] = row
		}
	}
}

// Dynamic damage taken modifiers the warlock effects describe: Improved Shadow Bolt registers one on
// every target (talents_destruction.go applyImprovedShadowBolt).
func warlockDamageTakenModifiers(agent core.Agent) int {
	if agent.(warlock.WarlockAgent).GetWarlock().Talents.ImprovedShadowBolt > 0 {
		return 1
	}
	return 0
}

// The periodic tick sim/warlock/warlock.go periodicTickOutcome picks; a physical crit is not exported.
func warlockTick(row *spelldata.Spell, unrepresented *[]string) map[string]any {
	if row.PeriodicCanCrit() && row.DefenseTypeCore() != core.DefenseTypeMagic {
		*unrepresented = append(*unrepresented, fmt.Sprintf("spell %d ticks with a physical crit roll", row.ID))
	}
	return periodicRank(row)
}

func withKind(kind string, fields map[string]any) map[string]any {
	fields["kind"] = kind
	return fields
}

// Effects whose parameters live in Go closures. Each formula mirrors the cited Go file at the
// pinned revision; Rust reads these values rather than client tables.
func warlockEffects(agent core.Agent, character *core.Character) []map[string]any {
	w := agent.(warlock.WarlockAgent).GetWarlock()
	talents := w.Talents
	target := w.Env.Encounter.ActiveTargetUnits[0]
	unrepresented := classNotes
	effects := []map[string]any{}

	// shadowbolt.go: every rank, the hit lands after travel.
	effects = append(effects, map[string]any{"kind": "shadow_bolt"})
	// immolate.go, corruption.go: a snapshot dot of the client's periodic effect.
	effects = append(effects, withKind("immolate", warlockTick(wlImmolateLadder.Highest(), unrepresented)))
	effects = append(effects, withKind("corruption", warlockTick(wlCorruptionLadder.Highest(), unrepresented)))
	// agony.go: the snapshot pays half the tick and every fourth tick adds that half back, Go literals.
	agony := withKind("bane_of_agony", warlockTick(wlBaneOfAgonyLadder.Highest(), unrepresented))
	agony["ramp_share"], agony["ramp_every_ticks"] = 0.5, int32(4)
	if talents.AmplifyCurse {
		agony["amplify"] = 1 + wlAmplifyCurse.EffectAt(1).FractionAt(1)
	}
	effects = append(effects, agony)
	if talents.AmplifyCurse { // talents_affliction.go registerAmplifyCurse
		effects = append(effects, map[string]any{"kind": "amplify_curse", "spell_id": wlAmplifyCurse.Highest().ID, "aura": "Amplify Curse"})
	}
	// curse_of_elements.go: core's debuff on the target, whose stat and damage taken changes the
	// exporter reads off the client row and checks against Go activating it.
	if aura := w.CurseOfElementsAuras.Get(target); aura != nil {
		effects = append(effects, curseOfElementsEffect(w, target, aura, unrepresented))
	}
	// lifetap.go: (base + Spirit) * (1 + Improved Life Tap) health into as much mana.
	lifeTap := wlLifeTapLadder.Highest()
	effects = append(effects, map[string]any{
		"kind": "life_tap", "spell_id": lifeTap.ID, "base_amount": lifeTap.EffectN(1).Average(core.CharacterLevel),
		"mana_multiplier": 1 + wlImprovedLifeTap.FractionAt(talents.ImprovedLifeTap),
	})
	if talents.Conflagrate { // conflagrate.go
		effects = append(effects, map[string]any{
			"kind": "conflagrate", "spell_id": wlConflagrateLadder.Highest().ID,
			"keep_immolate_chance": wlShadowAndFlame.EffectAt(2).FractionAt(talents.ShadowAndFlame), "rng_label": "Shadow and Flame",
		})
	}
	// shadowburn.go, searing_pain.go, soulfire.go: one client damage roll each.
	if talents.Shadowburn {
		effects = append(effects, map[string]any{"kind": "shadowburn"})
	}
	effects = append(effects, map[string]any{"kind": "searing_pain"})
	effects = append(effects, map[string]any{"kind": "soul_fire"})
	if talents.ImprovedShadowBolt > 0 { // talents_destruction.go applyImprovedShadowBolt
		effects = append(effects, map[string]any{
			"kind": "improved_shadow_bolt", "trigger_aura": "Improved Shadow Bolt Trigger",
			"aura": w.ImprovedShadowBoltAuras.Get(target).Label, "spell_id": wlImprovedShadowBoltOn.Highest().ID,
			"multiplier":     1 + wlImprovedShadowBolt.FractionAt(talents.ImprovedShadowBolt),
			"trigger_spells": procTriggerSpells(character, core.ProcTrigger{ClassSpellMask: warlock.WarlockSpellShadowBolt}),
		})
	}
	if talents.ShadowAndFlame > 0 { // talents_destruction.go applyShadowAndFlame
		effects = append(effects, map[string]any{
			"kind": "shadow_and_flame", "trigger_aura": "Shadow and Flame Trigger",
			"shadow_aura": "Shadow and Flame (Shadow)", "fire_aura": "Shadow and Flame (Fire)",
			"shadow_spell_id": wlShadowAndFlameRows.ByID(1293816).ID, "fire_spell_id": wlShadowAndFlameRows.ByID(426311).ID,
			"multiplier": 1 + wlShadowAndFlame.EffectAt(3).FractionAt(talents.ShadowAndFlame),
			"trigger_spells": procTriggerSpells(character, core.ProcTrigger{
				ClassSpellMask: warlock.WarlockSpellConflagrate | warlock.WarlockSpellShadowBurn,
			}),
			"shadow_spells": spellsMatching(character, warlock.WarlockSpellConflagrate),
		})
	}
	return effects
}

var resistanceStatNames = map[stats.Stat]string{
	stats.ArcaneResistance: "arcane", stats.FireResistance: "fire", stats.FrostResistance: "frost",
	stats.NatureResistance: "nature", stats.ShadowResistance: "shadow",
}

// Core's Curse of the Elements debuff (buffs/debuffs_auto_gen.go) parses the client row: a flat
// change to the resistances its mask names and a damage taken multiplier on its schools, applied
// on gain and undone on expire (spelldata parse_effects_table.go). A separate reset simulation
// activates the aura so the exported values are checked against what Go actually changes.
func curseOfElementsEffect(w *warlock.Warlock, target *core.Unit, aura *core.Aura, unrepresented *[]string) map[string]any {
	row := wlCurseOfElements.Highest()
	resistance := map[string]float64{}
	multipliers := map[string]float64{}
	for i := range row.Effects {
		e := &row.Effects[i]
		if !spelldata.AppliesAura(e.Type) {
			continue
		}
		value := e.Average(w.Level)
		switch e.Aura {
		case dbcenums.A_MOD_RESISTANCE:
			for stat, name := range resistanceStatNames {
				if e.Misc&int32(statSchool(stat)) != 0 {
					resistance[name] = value
				}
			}
		case dbcenums.A_MOD_DAMAGE_PERCENT_TAKEN:
			for index, name := range schoolNames {
				if index >= 2 && e.Misc&(1<<(index-1)) != 0 {
					multipliers[name] = 1 + value/100
				}
			}
		default:
			*unrepresented = append(*unrepresented, fmt.Sprintf("Curse of the Elements effect aura %d", e.Aura))
		}
	}
	for _, other := range target.GetAuras() {
		if other != aura && aura.Tag != "" && other.Tag == aura.Tag {
			*unrepresented = append(*unrepresented, fmt.Sprintf("target aura %s shares Curse of the Elements' category", other.Label))
		}
	}
	checkTargetAuraChanges(aura.Label, resistance, multipliers, unrepresented)
	return map[string]any{
		"kind": "curse_of_the_elements", "spell_id": row.ID, "aura": aura.Label,
		"resistance_delta": resistance, "school_damage_taken_multiplier": multipliers,
	}
}

func statSchool(stat stats.Stat) core.SpellSchool {
	switch stat {
	case stats.ArcaneResistance:
		return core.SpellSchoolArcane
	case stats.FireResistance:
		return core.SpellSchoolFire
	case stats.FrostResistance:
		return core.SpellSchoolFrost
	case stats.NatureResistance:
		return core.SpellSchoolNature
	case stats.ShadowResistance:
		return core.SpellSchoolShadow
	}
	return 0
}

// Activate a target aura in a separate reset simulation and require that it changes exactly the
// stated resistances, by the stated amounts, and the stated school damage taken multipliers.
func checkTargetAuraChanges(label string, resistance map[string]float64, multipliers map[string]float64, unrepresented *[]string) {
	simulation := core.NewSim(exportRequest, simsignals.CreateSignals())
	simulation.Reset()
	unit := simulation.Encounter.ActiveTargetUnits[0]
	beforeStats, beforePseudo := unit.GetStats(), unit.PseudoStats
	unit.GetAura(label).Activate(simulation)
	afterStats, afterPseudo := unit.GetStats(), unit.PseudoStats
	for index := 0; index < int(stats.SimStatsLen); index++ {
		stat := stats.Stat(index)
		want := beforeStats[stat]
		if name, ok := resistanceStatNames[stat]; ok {
			want += resistance[name]
		}
		if afterStats[stat] != want {
			*unrepresented = append(*unrepresented, fmt.Sprintf("%s changes target %s unexpectedly", label, stat.StatName()))
		}
	}
	for index, name := range schoolNames {
		want := beforePseudo.SchoolDamageTakenMultiplier[index]
		if multiplier, ok := multipliers[name]; ok {
			want *= multiplier
		}
		if afterPseudo.SchoolDamageTakenMultiplier[index] != want {
			*unrepresented = append(*unrepresented, fmt.Sprintf("%s changes target %s damage taken unexpectedly", label, name))
		}
	}
	afterPseudo.SchoolDamageTakenMultiplier = beforePseudo.SchoolDamageTakenMultiplier
	if afterPseudo != beforePseudo {
		*unrepresented = append(*unrepresented, fmt.Sprintf("%s changes other target pseudo stats", label))
	}
}
