// Shaman export: class spell names, client damage rows and the effects Go keeps in closures.
package main

import (
	"github.com/wowsims/forever/sim/core"
	"github.com/wowsims/forever/sim/core/dbcenums"
	"github.com/wowsims/forever/sim/core/proto"
	"github.com/wowsims/forever/sim/core/spelldata"
	"github.com/wowsims/forever/sim/shaman"
)

func init() {
	classExports[proto.Class_ClassShaman] = classExport{
		spells: shamanClassSpells, damageRows: shamanDamageRows, effects: shamanEffects, unrepresented: shamanUnrepresented,
	}
}

// Shaman class masks are Go-internal bit positions (sim/shaman/shaman.go). Export the stable class
// spell name instead of the bit so a Go reordering cannot silently change Rust behavior.
var shamanClassSpells = []classSpellName{
	{shaman.SpellMaskFlameShockDirect, "flame_shock_direct"}, {shaman.SpellMaskFlameShockDot, "flame_shock_dot"},
	{shaman.SpellMaskLightningBolt, "lightning_bolt"}, {shaman.SpellMaskLightningBoltOverload, "lightning_bolt_overload"},
	{shaman.SpellMaskChainLightning, "chain_lightning"}, {shaman.SpellMaskChainLightningOverload, "chain_lightning_overload"},
	{shaman.SpellMaskEarthShock, "earth_shock"}, {shaman.SpellMaskLightningShield, "lightning_shield"},
	{shaman.SpellMaskMagmaTotem, "magma_totem"}, {shaman.SpellMaskSearingTotem, "searing_totem"},
	{shaman.SpellMaskFireNova, "fire_nova"}, {shaman.SpellMaskFlametongueTotem, "flametongue_totem"},
	{shaman.SpellMaskStormstrikeCast, "stormstrike_cast"}, {shaman.SpellMaskStormstrikeDamage, "stormstrike_damage"},
	{shaman.SpellMaskEarthShield, "earth_shield"}, {shaman.SpellMaskFrostShock, "frost_shock"},
	{shaman.SpellMaskFlametongueWeapon, "flametongue_weapon"}, {shaman.SpellMaskWindfuryWeapon, "windfury_weapon"},
	{shaman.SpellMaskFrostbrandWeapon, "frostbrand_weapon"}, {shaman.SpellMaskRockbiterWeapon, "rockbiter_weapon"},
	{shaman.SpellMaskElementalMastery, "elemental_mastery"}, {shaman.SpellMaskShamanisticRage, "shamanistic_rage"},
	{shaman.SpellMaskBasicTotem, "basic_totem"}, {shaman.SpellMaskShieldSelfProc, "shield_self_proc"},
	{shaman.SpellMaskLavaBurst, "lava_burst"},
}

// The client rows sim/shaman reads. spellData is private to the package, so the ladders are
// restated with the ids of sim/shaman/spell_data_auto_gen.go at the pinned revision.
var (
	shamanLavaBurst        = spelldata.Ranked(408490, 1238299, 1238300)
	shamanFlameShock       = spelldata.Ranked(8050, 8052, 8053, 10447, 10448, 29228)
	shamanFireNova         = spelldata.Ranked(408341, 408342, 408343, 408344, 408345)
	shamanFireNovaHit      = spelldata.Ranked(8349, 8502, 8503, 11306, 11307, 408423, 408424, 408426, 408427, 408428)
	shamanSearingTotem     = spelldata.Ranked(3599, 6363, 6364, 6365, 10437, 10438)
	shamanSearingAttack    = spelldata.Ranked(3606, 6350, 6351, 6352, 10435, 10436)
	shamanElementalFocus   = spelldata.Ranked(16164)
	shamanClearcasting    = spelldata.Ranked(16246)
)

// Shaman spell rows whose ApplyEffects roll a client damage effect: every Lightning Bolt and Chain
// Lightning rank (lightning_bolt.go, chain_lightning.go), Lava Burst's and Flame Shock's highest
// rank (lava_burst.go, shocks.go). Overloads roll their parent's row; tagged spells take none here.
func shamanDamageRows(rows map[int32]*spelldata.Spell) {
	shaman.LightningBoltRankMap.Each(func(_ int32, row *spelldata.Spell) { rows[row.ID] = row })
	shaman.ChainLightningRankMap.Each(func(_ int32, row *spelldata.Spell) { rows[row.ID] = row })
	if row := shamanLavaBurst.Highest(); row != nil {
		rows[row.ID] = row
	}
	if row := shamanFlameShock.Highest(); row != nil {
		rows[row.ID] = row
	}
}

// Effects whose parameters live in Go closures. Each formula mirrors the cited Go file at the
// pinned revision; Rust reads these values rather than client tables.
func shamanEffects(agent core.Agent, character *core.Character) []map[string]any {
	sham := agent.(shaman.ShamanAgent).GetShaman()
	talents := sham.Talents
	effects := []map[string]any{}
	// lightning_bolt.go: the overload rolls on landing, before the bolt deals its damage.
	effects = append(effects, map[string]any{
		"kind": "lightning_bolt", "overload_chance": sham.GetOverloadChance(), "overload_tag": shaman.CastTagLightningOverload,
		"rng_label": "Lightning Bolt Elemental Overload",
	})
	// chain_lightning.go: a third of the overload chance per hit, and a Go literal bounce reduction.
	effects = append(effects, map[string]any{
		"kind": "chain_lightning", "overload_chance": sham.GetOverloadChance(), "overload_tag": shaman.CastTagLightningOverload,
		"rng_label": "Chain Lightning Elemental Overload", "bounce_reduction": 0.7, "bounce_bonus": sham.ChainLightningBounceBonus,
	})
	// shocks.go registerFlameShockSpell: the dot snapshots its tick and rolls the family table's outcome.
	flameShock := shamanFlameShock.Highest()
	tick := flameShock.PeriodicEffect()
	effects = append(effects, map[string]any{
		"kind": "flame_shock", "spell_id": flameShock.ID, "tick_base": tick.Average(core.CharacterLevel),
		"tick_can_crit": flameShock.PeriodicCanCrit() && flameShock.DefenseTypeCore() == core.DefenseTypeMagic,
	})
	if talents.LavaBurst { // lava_burst.go: the bonus against a target burning with Flame Shock.
		row := shamanLavaBurst.Highest()
		effects = append(effects, map[string]any{
			"kind": "lava_burst", "spell_id": row.ID, "flame_shock_bonus": 1 + row.EffectN(2).Percent(),
		})
	}
	// fire_totems.go registerFireNovaSpell: one hit on each target from the nova's damage row average.
	effects = append(effects, map[string]any{
		"kind": "fire_nova", "spell_id": shamanFireNova.Highest().ID,
		"base_damage": shamanFireNovaHit.ByID(408428).DamageEffect().Average(core.CharacterLevel),
	})
	// fire_totems.go registerSearingTotemSpell: a target dot whose ticks cast the attack. Its cast
	// first takes down the other fire totems in cancelFireTotems order; Totem of Wrath is never set.
	effects = append(effects, map[string]any{
		"kind": "searing_totem", "spell_id": shamanSearingTotem.Highest().ID, "attack_spell_id": shamanSearingAttack.Highest().ID,
		"attack_damage":    shamanSearingAttack.Highest().DamageEffect().Average(core.CharacterLevel),
		"magma_totem_aura": sham.MagmaTotem.AOEDot().Aura.Label, "flametongue_totem_aura": sham.FlametongueTotemAura.Label,
	})
	if talents.ElementalFocus { // talents_elemental.go applyElementalFocus
		clearcasting := shamanClearcasting.Highest()
		effects = append(effects, map[string]any{
			"kind": "elemental_focus", "trigger_aura": "Elemental Focus", "aura": "Clearcasting",
			"proc_chance":      float64(shamanElementalFocus.Rank(1).ProcChance) / 100,
			"cost_percent_add": clearcasting.Effect(dbcenums.A_ADD_PCT_MODIFIER, int32(dbcenums.SPELLMOD_COST)).Percent(),
			"max_stacks":       int32(clearcasting.ProcCharges),
		})
	}
	return effects
}

// Shaman behavior the exporter cannot describe.
func shamanUnrepresented(agent core.Agent, _ *core.Character) []string {
	sham := agent.(shaman.ShamanAgent).GetShaman()
	unrepresented := []string{}
	// shields.go startShieldProcPeriodicAction: a periodic self hit at encounter start.
	if sham.SelfBuffs.ShieldProcrate > 0 {
		unrepresented = append(unrepresented, "shaman shield proc rate is unsupported")
	}
	// shocks.go periodicTickOutcome: a physical crit roll on Flame Shock ticks.
	if row := shamanFlameShock.Highest(); row.PeriodicCanCrit() && row.DefenseTypeCore() != core.DefenseTypeMagic {
		unrepresented = append(unrepresented, "Flame Shock ticks roll a physical crit")
	}
	return unrepresented
}
