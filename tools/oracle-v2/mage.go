// Mage export: class spell names, client damage rows and the effects Go keeps in closures.
package main

import (
	"time"

	"github.com/wowsims/forever/sim/core"
	"github.com/wowsims/forever/sim/core/dbcenums"
	"github.com/wowsims/forever/sim/core/proto"
	"github.com/wowsims/forever/sim/core/spelldata"
	"github.com/wowsims/forever/sim/mage"
)

func init() {
	classExports[proto.Class_ClassMage] = classExport{spells: mageClassSpells, damageRows: mageDamageRows, effects: mageEffects}
}

// Mage class masks are Go-internal bit positions. Export the stable class spell name
// instead of the bit so a Go reordering cannot silently change Rust behavior.
var mageClassSpells = []classSpellName{
	{mage.MageSpellArcaneBlast, "arcane_blast"}, {mage.MageSpellArcaneExplosion, "arcane_explosion"},
	{mage.MageSpellArcanePower, "arcane_power"}, {mage.MageSpellArcaneMissilesCast, "arcane_missiles_cast"},
	{mage.MageSpellArcaneMissilesTick, "arcane_missiles_tick"}, {mage.MageSpellBlastWave, "blast_wave"},
	{mage.MageSpellBlizzard, "blizzard"}, {mage.MageSpellColdSnap, "cold_snap"},
	{mage.MageSpellConeOfCold, "cone_of_cold"}, {mage.MageSpellEvocation, "evocation"},
	{mage.MageSpellFireBlast, "fire_blast"}, {mage.MageSpellFireball, "fireball"},
	{mage.MageSpellFlamestrike, "flamestrike"}, {mage.MageSpellFlamestrikeDot, "flamestrike_dot"},
	{mage.MageSpellFrostArmor, "frost_armor"}, {mage.MageSpellFrostbolt, "frostbolt"},
	{mage.MageSpellFrostNova, "frost_nova"}, {mage.MageSpellIceBarrier, "ice_barrier"},
	{mage.MageSpellIceBlock, "ice_block"}, {mage.MageSpellIceLance, "ice_lance"},
	{mage.MageSpellIgnite, "ignite"}, {mage.MageSpellMageArmor, "mage_armor"},
	{mage.MageSpellManaGems, "mana_gems"}, {mage.MageSpellMoltenArmor, "molten_armor"},
	{mage.MageSpellPresenceOfMind, "presence_of_mind"}, {mage.MageSpellPyroblast, "pyroblast"},
	{mage.MageSpellPyroblastDot, "pyroblast_dot"}, {mage.MageSpellScorch, "scorch"},
	{mage.MageSpellManaGem, "mana_gem"}, {mage.MageSpellCombustion, "combustion"},
	{mage.MageSpellImprovedBlizzard, "improved_blizzard"}, {mage.MageSpellFrostfireBolt, "frostfire_bolt"},
}

// Mage spell rows whose ApplyEffects roll a client damage effect. The ladders and the
// rank pairing mirror sim/mage/frostbolt.go, ice_lance.go and arcane_missiles.go.
var (
	frostboltLadder     = spelldata.Ranked(116, 205, 837, 7322, 8406, 8407, 8408, 10179, 10180, 10181, 25304)
	iceLanceLadder      = spelldata.Ranked(1312002, 400640, 1240044, 1240045, 1240046, 1240047)
	missilesLadder      = spelldata.Ranked(5143, 5144, 5145, 8416, 8417, 10211, 10212, 25345)
	missileTicksLadder  = spelldata.Ranked(7268, 7269, 7270, 8419, 8418, 10273, 10274, 25346)
	arcaneBlastLadder   = spelldata.Ranked(400574, 1239696, 1239697, 1239699, 1239700)
	arcaneBlastBuff     = spelldata.Ranked(400573)
	arcanePower         = spelldata.Ranked(12042)
	presenceOfMind      = spelldata.Ranked(12043)
	igniteTriggered     = spelldata.Ranked(412538)
	igniteTalent        = spelldata.Talent(11119, 5)
	fireBlastLadder     = spelldata.Ranked(2136, 2137, 2138, 8412, 8413, 10197, 10199)
	scorchLadder        = spelldata.Ranked(2948, 8444, 8445, 8446, 10205, 10206, 10207)
	improvedScorch      = spelldata.Talent(11095, 3)
	fireVulnerability   = spelldata.Ranked(22959)
	masterOfElements    = spelldata.Talent(29074, 3)
	pyroblastLadder     = spelldata.Ranked(11366, 12505, 12522, 12523, 12524, 12525, 12526, 18809)
	heatingUpTriggered  = spelldata.Ranked(400625)
	combustion          = spelldata.Ranked(11129)
	combustionTriggered = spelldata.Ranked(28682)
	fireballLadder      = spelldata.Ranked(133, 143, 145, 3140, 8400, 8401, 8402, 10148, 10149, 10150, 10151, 25306)
	frostfireLadder     = spelldata.Ranked(401502, 1237312, 1237313)
	arcaneConcentration = spelldata.Talent(11213, 5)
	clearcastingTrigger = spelldata.Ranked(12536)
	fingersOfFrost      = spelldata.Talent(400647, 2)
	fingersTriggered    = spelldata.Ranked(400669)
	shatter             = spelldata.Talent(11170, 3)
	wintersChill        = spelldata.Talent(11180, 5)
	wintersChillTrigger = spelldata.Ranked(12579)
	evocationLadder     = spelldata.Ranked(12051)
	agateTriggered      = spelldata.Ranked(5405)
	jadeTriggered       = spelldata.Ranked(10052)
	citrineTriggered    = spelldata.Ranked(10057)
	rubyTriggered       = spelldata.Ranked(10058)
)

func mageDamageRows(rows map[int32]*spelldata.Spell) {
	frostboltLadder.Each(func(_ int32, row *spelldata.Spell) { rows[row.ID] = row })
	missileTicksLadder.Each(func(_ int32, row *spelldata.Spell) { rows[row.ID] = row })
	if ice := iceLanceLadder.Highest(); ice != nil {
		rows[ice.ID] = ice
	}
	if blast := arcaneBlastLadder.Highest(); blast != nil {
		rows[blast.ID] = blast
	}
	if fireBlast := fireBlastLadder.Highest(); fireBlast != nil {
		rows[fireBlast.ID] = fireBlast
	}
	scorchLadder.Each(func(_ int32, row *spelldata.Spell) { rows[row.ID] = row })
	fireballLadder.Each(func(_ int32, row *spelldata.Spell) { rows[row.ID] = row })
	frostfireLadder.Each(func(_ int32, row *spelldata.Spell) { rows[row.ID] = row })
	if pyroblast := pyroblastLadder.Highest(); pyroblast != nil {
		rows[pyroblast.ID] = pyroblast
	}
}

// Effects whose parameters live in Go closures. Each formula mirrors the cited Go file
// at the pinned revision; Rust reads these values rather than client tables.
func mageEffects(agent core.Agent, character *core.Character) []map[string]any {
	m := agent.(mage.MageAgent).GetMage()
	talents := m.Talents
	effects := []map[string]any{}
	unit := &character.Unit
	if talents.ArcaneConcentration > 0 { // talents_arcane.go registerArcaneConcentration
		effects = append(effects, map[string]any{
			"kind": "arcane_concentration", "talent_rank": talents.ArcaneConcentration,
			"proc_chance": arcaneConcentration.EffectAt(1).FractionAt(talents.ArcaneConcentration),
			"icd_ns":      nanos(arcaneConcentration.Highest().ICD()), "trigger_aura": "Arcane Concentration",
			"aura": "Clearcasting", "aura_duration_ns": nanos(clearcastingTrigger.Highest().Duration()),
		})
	}
	if talents.MissileBarrage { // talents_arcane.go registerMissileBarrage: Go literals
		effects = append(effects, map[string]any{
			"kind": "missile_barrage", "trigger_aura": "Missile Barrage Trigger", "aura": "Missile Barrage",
			"arcane_blast_chance": 0.40, "bolt_chance": 0.20, "rng_label": "Missile Barrage",
			"cost_percent_add": -1.0, "tick_length_delta_ns": nanos(-500 * time.Millisecond),
		})
	}
	if talents.FingersOfFrost > 0 { // talents_frost.go registerFingersOfFrost
		effects = append(effects, map[string]any{
			"kind": "fingers_of_frost", "talent_rank": talents.FingersOfFrost, "shatter_rank": talents.Shatter,
			"proc_chance":  fingersOfFrost.EffectAt(2).FractionAt(talents.FingersOfFrost),
			"max_stacks":   int32(fingersOfFrost.EffectAt(1).ValueAt(talents.FingersOfFrost)),
			"shatter_crit": shatter.ValueAt(talents.Shatter), "duration_ns": nanos(fingersTriggered.Highest().Duration()),
			"trigger_aura": "Fingers of Frost Trigger", "aura": "Fingers of Frost",
		})
	}
	if talents.WintersChill > 0 { // talents_frost.go registerWinterChill
		effects = append(effects, map[string]any{
			"kind": "winters_chill", "talent_rank": talents.WintersChill,
			"proc_chance":    wintersChill.EffectAt(2).FractionAt(talents.WintersChill),
			"max_stacks":     int32(wintersChill.EffectAt(1).ValueAt(talents.WintersChill)),
			"crit_per_stack": wintersChillTrigger.Highest().EffectN(1).Average(core.CharacterLevel),
			"duration_ns":    nanos(wintersChillTrigger.Highest().Duration()),
			"trigger_aura":   "Winters Chill Talent", "aura": "Winter's Chill",
		})
	}
	if talents.IceLance { // ice_lance.go
		effects = append(effects, map[string]any{
			"kind": "ice_lance", "spell_id": iceLanceLadder.Highest().ID, "frozen_multiplier": mage.IceLanceFrozenMultiplier,
		})
	}
	if talents.ColdSnap { // cold_snap.go
		effects = append(effects, map[string]any{"kind": "cold_snap", "spell_id": spelldata.Ranked(12472).Highest().ID})
	}
	// evocation.go
	evocation := evocationLadder.Highest()
	effects = append(effects, map[string]any{
		"kind": "evocation", "spell_id": evocation.ID, "regen_aura": "Evocation Regen", "channel_aura": "Evocation",
		"regen_multiplier": evocation.Effect(dbcenums.A_MOD_POWER_REGEN_PERCENT, 0).Average(core.CharacterLevel) / 100,
	})
	// arcane_missiles.go: channel rank N fires tick rank N.
	missiles := []map[string]any{}
	missilesLadder.Each(func(rank int32, row *spelldata.Spell) {
		missiles = append(missiles, map[string]any{"channel_spell_id": row.ID, "tick_spell_id": missileTicksLadder.Rank(rank).ID})
	})
	effects = append(effects, map[string]any{"kind": "arcane_missiles", "ranks": missiles})
	// frostbolt.go
	effects = append(effects, map[string]any{"kind": "frostbolt"})
	if talents.ArcaneBlast { // arcane_blast.go and arcane_charge.go
		buff := arcaneBlastBuff.Highest()
		effects = append(effects, map[string]any{
			"kind": "arcane_blast", "spell_id": arcaneBlastLadder.Highest().ID, "aura": "Arcane Blast",
			"damage_per_stack": buff.Effect(dbcenums.A_ADD_PCT_MODIFIER, int32(dbcenums.SPELLMOD_DAMAGE)).Average(core.CharacterLevel) / 100,
			"cost_per_stack":   buff.Effect(dbcenums.A_ADD_PCT_MODIFIER, int32(dbcenums.SPELLMOD_COST)).Average(core.CharacterLevel) / 100,
		})
	}
	// fire_blast.go
	effects = append(effects, map[string]any{"kind": "fire_blast"})
	// fireball.go: every rank; the hit lands after travel, then its dot snapshots and ticks.
	fireballs := []map[string]any{}
	fireballLadder.Each(func(_ int32, row *spelldata.Spell) {
		fireballs = append(fireballs, map[string]any{
			"spell_id": row.ID, "tick_base": row.PeriodicEffect().Average(core.CharacterLevel),
			"tick_can_crit": row.PeriodicCanCrit() && row.DefenseTypeCore() == core.DefenseTypeMagic,
		})
	})
	effects = append(effects, map[string]any{"kind": "fireball", "ranks": fireballs})
	// frostfire_bolt.go: Fireball's shape with a Frostfire school, every rank.
	frostfires := []map[string]any{}
	frostfireLadder.Each(func(_ int32, row *spelldata.Spell) {
		frostfires = append(frostfires, map[string]any{
			"spell_id": row.ID, "tick_base": row.PeriodicEffect().Average(core.CharacterLevel),
			"tick_can_crit": row.PeriodicCanCrit() && row.DefenseTypeCore() == core.DefenseTypeMagic,
		})
	})
	effects = append(effects, map[string]any{"kind": "frostfire_bolt", "ranks": frostfires})
	// scorch.go: every rank; Improved Scorch stacks Fire Vulnerability on the mage itself.
	scorch := map[string]any{"kind": "scorch"}
	if talents.ImprovedScorch > 0 {
		scorch["improved_scorch"] = map[string]any{
			"aura": "Fire Vulnerability", "proc_chance": improvedScorch.FractionAt(talents.ImprovedScorch),
			"damage_per_stack": fireVulnerability.Highest().EffectN(1).Average(core.CharacterLevel) / 100,
		}
	}
	effects = append(effects, scorch)
	if talents.Pyroblast { // pyroblast.go: Fireball's shape, highest rank only
		row := pyroblastLadder.Highest()
		effects = append(effects, map[string]any{
			"kind": "pyroblast", "spell_id": row.ID, "tick_base": row.PeriodicEffect().Average(core.CharacterLevel),
			"tick_can_crit": row.PeriodicCanCrit() && row.DefenseTypeCore() == core.DefenseTypeMagic,
		})
	}
	if talents.Combustion { // combustion.go
		buff := combustionTriggered.Highest()
		effects = append(effects, map[string]any{
			"kind": "combustion", "spell_id": combustion.Highest().ID, "aura": "Combustion",
			"crit_per_stack": buff.Effect(dbcenums.A_ADD_FLAT_MODIFIER, int32(dbcenums.SPELLMOD_CRITICAL_CHANCE)).Average(core.CharacterLevel),
			"max_crits":      int32(combustion.Highest().ProcCharges),
		})
	}
	if talents.HeatingUp { // talents_fire.go registerHotStreak
		buff := heatingUpTriggered.Highest()
		effects = append(effects, map[string]any{
			"kind": "heating_up", "aura": "Heating Up", "trigger_aura": "Heating Up Trigger",
			"cast_time_per_stack": buff.Effect(dbcenums.A_ADD_PCT_MODIFIER, int32(dbcenums.SPELLMOD_CASTING_TIME)).Percent(),
		})
	}
	if talents.MasterOfElements > 0 { // talents_fire.go registerMasterOfElements
		effects = append(effects, map[string]any{
			"kind": "master_of_elements", "trigger_aura": "Master of Elements",
			"refund":            masterOfElements.FractionAt(talents.MasterOfElements),
			"metrics_action_id": actionID(core.ActionID{SpellID: masterOfElements.Highest().ID}),
		})
	}
	if talents.Ignite > 0 { // talents_fire.go registerIgnite: fire spell crits feed a dot
		effects = append(effects, map[string]any{
			"kind": "ignite", "trigger_aura": "Ignite Talent", "spell_id": igniteTriggered.Highest().ID,
			"share":     igniteTalent.FractionAt(talents.Ignite),
			"num_ticks": int32(igniteTriggered.Highest().Duration() / (2 * time.Second)),
		})
	}
	if talents.ArcanePower { // arcane_power.go
		rank := arcanePower.Highest()
		effects = append(effects, map[string]any{
			"kind": "arcane_power", "spell_id": rank.ID, "aura": "Arcane Power",
			"damage":           rank.Effect(dbcenums.A_ADD_PCT_MODIFIER, int32(dbcenums.SPELLMOD_DAMAGE)).Average(core.CharacterLevel) / 100,
			"cost_percent_add": rank.Effect(dbcenums.A_ADD_PCT_MODIFIER, int32(dbcenums.SPELLMOD_COST)).Average(core.CharacterLevel) / 100,
		})
	}
	if talents.PresenceOfMind { // presence_of_mind.go
		rank := presenceOfMind.Highest()
		effects = append(effects, map[string]any{
			"kind": "presence_of_mind", "spell_id": rank.ID, "aura": "Presence of Mind",
			"cast_time_percent": rank.Effect(dbcenums.A_ADD_PCT_MODIFIER, int32(dbcenums.SPELLMOD_CASTING_TIME)).Average(core.CharacterLevel) / 100,
		})
	}
	// mana_gems.go: smaller gems wait for larger ones; all share the conjured cooldown.
	gems := []map[string]any{}
	for _, gem := range []struct {
		item int32
		row  *spelldata.Spell
	}{{5514, agateTriggered.Highest()}, {5513, jadeTriggered.Highest()}, {8007, citrineTriggered.Highest()}, {8008, rubyTriggered.Highest()}} {
		gems = append(gems, map[string]any{"item_id": gem.item, "mana": gem.row.EnergizeEffect().Average(core.CharacterLevel)})
	}
	effects = append(effects, map[string]any{"kind": "mana_gems", "gems": gems, "regen_window_seconds": 2.0})
	// armors.go: the regen part is already in the prepared pseudo stats.
	if hasAura(unit, "Mage Armor") {
		effects = append(effects, map[string]any{"kind": "mage_armor", "aura": "Mage Armor"})
	}
	return effects
}
