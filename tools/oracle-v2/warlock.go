// Warlock export: class spell names, client damage rows and the effects Go keeps in closures.
package main

import (
	"fmt"
	"time"

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
		damageTakenModifiers: warlockDamageTakenModifiers, inertPet: warlockInertPet,
		// talents_affliction.go registerAmplifyCurse registers its cast without a class mask.
		// talents_demonology.go applyDemonicBrand registers each demon's brand hit without one.
		unmaskedSpells: map[core.ActionID]string{{SpellID: 18288}: "amplify_curse",
			{SpellID: 1293697}: "demonic_brand", {SpellID: 1293698}: "demonic_brand",
			// talents_destruction.go applyBaneOfHavoc builds its cast from the client row alone.
			{SpellID: 1225228}: "bane_of_havoc"},
	}
	// pets.go: every demon is registered at construction and only the summoned one is enabled,
	// at reset; the sim has no summon spells.
	resetOnlyPetClasses[proto.Class_ClassWarlock] = true
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
	wlNightfall            = spelldata.Talent(18094, 2)
	wlNightfallTriggered   = spelldata.Ranked(17941)
	wlImprovedShadowBoltOn = spelldata.Ranked(17794)
	wlDecimation           = spelldata.Talent(440870, 2)
	wlBaneOfDoomLadder     = spelldata.Ranked(603)
	wlSiphonLifeLadder     = spelldata.Ranked(18265, 18879, 18880, 18881)
	wlDrainLifeLadder      = spelldata.Ranked(689, 699, 709, 7651, 11699, 11700)
	wlSoulSiphon           = spelldata.Talent(17804, 3)
	wlIncinerateLadder     = spelldata.Ranked(412758, 1293812, 1293813)
	wlWrackLadder          = spelldata.Ranked(1316697)
	wlBaneOfHavoc          = spelldata.Ranked(1225228)
	wlDemonicEnergies      = spelldata.Talent(1225214, 2)
	wlDemonicSacrificeOn   = spelldata.Ranked(18789, 18790, 18791, 18792)
)

func warlockDamageRows(rows map[int32]*spelldata.Spell) {
	wlShadowBoltLadder.Each(func(_ int32, row *spelldata.Spell) { rows[row.ID] = row })
	for _, ladder := range []spelldata.Ladder{wlImmolateLadder, wlConflagrateLadder, wlShadowburnLadder, wlSearingPainLadder, wlSoulFireLadder, wlIncinerateLadder} {
		if row := ladder.Highest(); row != nil {
			rows[row.ID] = row
		}
	}
}

// pets.go registers all four demons; only the summoned one is enabled on start, and the sim has no
// summon spell to enable another.
func warlockInertPet(agent core.Agent, pet *core.Pet) string {
	if pet.EnabledOnStart() {
		return ""
	}
	return "the warlock summoned another demon or none"
}

// Dynamic damage taken modifiers the warlock effects describe: Improved Shadow Bolt registers one on
// every target (talents_destruction.go applyImprovedShadowBolt).
func warlockDamageTakenModifiers(agent core.Agent) int {
	talents := agent.(warlock.WarlockAgent).GetWarlock().Talents
	count := 0
	if talents.ImprovedShadowBolt > 0 {
		count++
	}
	// wrack.go registerWrack: the bonus on Corruption and Bane of Agony ticks.
	if talents.Wrack {
		count++
	}
	if count > 0 {
		return count
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
	// doom.go: one snapshot tick a minute on, on the bane slot.
	effects = append(effects, withKind("bane_of_doom", warlockTick(wlBaneOfDoomLadder.Highest(), unrepresented)))
	// drain_life.go: a channeled dot scaled by Soul Siphon, which counts every registered
	// Affliction aura on the target (warlock.go AfflictionCount), and healing for each tick.
	drain := withKind("drain_life", warlockTick(wlDrainLifeLadder.Highest(), unrepresented))
	soulSiphon := 1.0
	if talents.SoulSiphon > 0 {
		soulSiphon = 1 + wlSoulSiphon.FractionAt(talents.SoulSiphon)*min(w.AfflictionCount(target), 3)
	}
	drain["soul_siphon"] = soulSiphon
	drain["self_healing_multiplier"] = w.PseudoStats.SelfHealingMultiplier
	effects = append(effects, drain)
	if talents.Wrack { // wrack.go: a channel scaled by Soul Siphon and a bonus on two dots' ticks
		row := wlWrackLadder.Highest()
		wrack := withKind("wrack", warlockTick(row, unrepresented))
		wrack["soul_siphon"] = soulSiphon
		wrack["dot_bonus"] = 1 + row.EffectN(2).Percent()
		wrack["dot_spells"] = spellsMatching(character, warlock.WarlockSpellCorruption|warlock.WarlockSpellCurseOfAgony)
		effects = append(effects, wrack)
	}
	if talents.BaneOfHavoc { // talents_destruction.go applyBaneOfHavoc
		effects = append(effects, map[string]any{
			"kind": "bane_of_havoc", "spell_id": wlBaneOfHavoc.Highest().ID,
			"aura": "Bane of Havoc-" + w.Label, "copy_aura": "Bane of Havoc - Copy",
		})
	}
	if talents.Incinerate { // incinerate.go: the client roll, raised on a burning target
		effects = append(effects, map[string]any{
			"kind": "incinerate", "immolate_bonus": 1 + wlIncinerateLadder.Highest().EffectN(2).Percent(),
		})
	}
	if talents.SiphonLife { // siphon_life.go: Corruption's shape, healing for each tick
		siphon := withKind("siphon_life", warlockTick(wlSiphonLifeLadder.Highest(), unrepresented))
		siphon["self_healing_multiplier"] = w.PseudoStats.SelfHealingMultiplier
		effects = append(effects, siphon)
	}
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
	tap := map[string]any{
		"kind": "life_tap", "spell_id": lifeTap.ID, "base_amount": lifeTap.EffectN(1).Average(core.CharacterLevel),
		"mana_multiplier": 1 + wlImprovedLifeTap.FractionAt(talents.ImprovedLifeTap),
	}
	// Demonic Energies hands the summoned demon a share of the restore.
	if share := wlDemonicEnergies.EffectAt(2).FractionAt(talents.DemonicEnergies); share > 0 && w.ActivePet != nil {
		tap["pet_mana_share"] = share
	}
	effects = append(effects, tap)
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
	if talents.Nightfall > 0 { // talents_affliction.go applyNightfall
		effects = append(effects, map[string]any{
			"kind": "nightfall", "trigger_aura": "Nightfall", "aura": "Shadow Trance",
			"aura_spell_id": wlNightfallTriggered.Highest().ID,
			"proc_chance":   wlNightfall.FractionAt(talents.Nightfall), "rng_label": "Nightfall",
			"trigger_spells": procTriggerSpells(character, core.ProcTrigger{ClassSpellMask: warlock.WarlockNightfallSpells}),
			"consume_spells": procTriggerSpells(character, core.ProcTrigger{ClassSpellMask: warlock.WarlockSpellShadowBolt}),
			"modded_spells":  spellsMatching(character, warlock.WarlockSpellShadowBolt), "cast_time_percent": -1.0,
		})
	}
	// pets.go: the summoned demon casts the first of its abilities it can afford above MinMana,
	// and otherwise waits 100 ms, a Go literal.
	if pet := w.ActivePet; pet != nil {
		autocast := []int{}
		for _, ability := range pet.AutoCastAbilities {
			for i, spell := range pet.Spellbook {
				if spell == ability {
					autocast = append(autocast, i)
				}
			}
		}
		effects = append(effects, map[string]any{
			"kind": "warlock_pet", "pet": pet.Label, "min_mana": pet.MinMana, "autocast_spells": autocast,
			"wait_ns": nanos(100 * time.Millisecond),
		})
	}
	if w.Imp != nil { // pets.go registerFireboltSpell: impFireboltEffect, unexported, mirrored here
		effect := spelldata.Effect{BasePoints: 44, PPL: 0.6000000238418579, Variance: 0.11363636702, SpellLevel: 58, MaxLevel: 63}
		average := effect.Average(core.CharacterLevel)
		effects = append(effects, map[string]any{
			"kind": "firebolt", "min_damage": average * (1 - effect.Variance/2), "max_damage": average * (1 + effect.Variance/2),
		})
	}
	if w.Succubus != nil { // pets.go registerLashOfPainSpell: a Go literal base
		effects = append(effects, map[string]any{"kind": "lash_of_pain", "base_damage": 50.0})
	}
	// talents_demonology.go applyFelEnergy: the Voidwalker's sacrifice restores a share of
	// maximum mana every period, from a periodic action its permanent aura starts.
	if aura := w.GetAura("Demonic Sacrifice"); aura != nil && aura.ActionID.SpellID == 18792 {
		row := wlDemonicSacrificeOn.ByID(18792)
		effects = append(effects, map[string]any{
			"kind": "fel_energy", "aura": aura.Label, "spell_id": row.ID,
			"mana_fraction": row.EffectN(1).Percent(), "period_ns": nanos(row.EffectN(1).Period()),
		})
	}
	if talents.Decimation > 0 { // talents_demonology.go applyDecimation
		points := talents.Decimation
		effects = append(effects, map[string]any{
			"kind": "decimation", "trigger_aura": "Decimation Trigger", "aura": w.DecimationAura.Label,
			"execute_phase": int32(35),
			"trigger_spells": procTriggerSpells(character, core.ProcTrigger{
				ClassSpellMask: warlock.WarlockSpellShadowBolt | warlock.WarlockSpellSearingPain,
			}),
			"damage_spells":     spellsMatching(character, warlock.WarlockSpellShadowBolt|warlock.WarlockSpellSearingPain),
			"damage_done_flat":  wlDecimation.EffectAt(4).FractionAt(points),
			"cast_spells":       spellsMatching(character, warlock.WarlockSpellSoulFire),
			"cast_time_percent": wlDecimation.EffectAt(1).FractionAt(points),
		})
	}
	if talents.DemonicBrand > 0 && !w.Options.SacrificeSummon { // talents_demonology.go applyDemonicBrand
		brand := map[string]any{
			"kind": "demonic_brand", "trigger_aura": "Demonic Brand Trigger",
			"target_aura": w.DemonicBrandAuras.Get(target).Label, "charges": w.DemonicBrandAuras.Get(target).MaxStacks,
			"trigger_spells": procTriggerSpells(character, core.ProcTrigger{ClassSpellMask: warlock.WarlockSpellSearingPain}),
		}
		if pet := w.ActivePet; pet != nil {
			brandID, power := int32(1293697), stats.ShadowDamage
			if pet == w.Imp {
				brandID, power = 1293698, stats.FireDamage
			}
			for i, spell := range pet.Spellbook {
				if spell.ActionID.SpellID == brandID {
					brand["brand_spell"] = i
				}
			}
			// The hit's roll and spell power share are Go literals.
			levelBonus := float64(core.CharacterLevel-26) * 1.5
			brand["pet"], brand["marker_aura"], brand["consumer_aura"] = pet.Label, pet.DemonicBrandAura.Label, "Demonic Brand consumer"
			brand["min_damage"], brand["max_damage"], brand["spell_power_coefficient"] = levelBonus+14, levelBonus+17, 0.078
			brand["school_power_stat"] = power.StatName()
		}
		effects = append(effects, brand)
	}
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
