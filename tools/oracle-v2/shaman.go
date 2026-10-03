// Shaman export: class spell names, client damage rows and the effects Go keeps in closures.
package main

import (
	"fmt"
	"time"

	"github.com/wowsims/forever/sim/core"
	"github.com/wowsims/forever/sim/core/buffs"
	"github.com/wowsims/forever/sim/core/dbcenums"
	"github.com/wowsims/forever/sim/core/proto"
	"github.com/wowsims/forever/sim/core/spelldata"
	"github.com/wowsims/forever/sim/core/stats"
	"github.com/wowsims/forever/sim/shaman"
)

func init() {
	classExports[proto.Class_ClassShaman] = classExport{
		spells: shamanClassSpells, damageRows: shamanDamageRows, effects: shamanEffects, unrepresented: shamanUnrepresented,
		swingReplacementKeepsSwing: shamanSwingReplacementKeepsSwing, playerEffects: shamanPlayerEffects, statAuras: shamanStatAuras,
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
	shamanClearcasting     = spelldata.Ranked(16246)
	shamanEarthShock       = spelldata.Ranked(8042, 8044, 8045, 8046, 10412, 10413, 10414)
	shamanStrengthOfEarth  = spelldata.Ranked(8075, 8160, 8161, 10442, 25361)
	shamanStormstrike      = spelldata.Ranked(17364)
	shamanDevastation      = spelldata.Talent(30160, 3)
	shamanFlurry           = spelldata.Talent(16256, 5)
	shamanFlurryBuff       = spelldata.Ranked(16257)
	shamanImpStormstrike   = spelldata.Talent(1223031, 2)
	shamanImpStormBuff     = spelldata.Ranked(1238931)
	shamanMaelstrom        = spelldata.Talent(408498, 5)
	shamanMaelstromBuff    = spelldata.Ranked(408505)
	shamanFarseer          = spelldata.Ranked(425336)
	shamanElementalWeapons = spelldata.Talent(16266, 3)
	shamanRockbiter        = spelldata.Ranked(10400, 15567, 15568, 15569, 16311, 16312, 16313)
	shamanFlametongueProc  = spelldata.Ranked(8026, 8028, 8029, 10444, 10445, 16343, 16344, 29469, 29470).ByID(16344)
	shamanFrostbrandProc   = spelldata.Ranked(8034, 8037, 10458, 16352, 16353)
	shamanFrostShock       = spelldata.Ranked(8056, 8058, 10472, 10473)
	shamanMagmaTotem       = spelldata.Ranked(8190, 10585, 10586, 10587)
	shamanMagmaPulse       = spelldata.Ranked(8187, 8188, 10579, 10580, 10581, 10582, 10583, 10584).ByID(10581)
	shamanLightningShield  = spelldata.Ranked(324, 325, 905, 945, 8134, 10431, 10432)
	shamanShieldOrb        = spelldata.Ranked(26363, 26364, 26365, 26366, 26367, 26369, 26370, 26545).ByID(26363)
	shamanGraceOfAir       = spelldata.Ranked(8835, 10627, 25359)
	shamanFlametongueTotem = spelldata.Ranked(8227, 8249, 10526, 16387)
	shamanManaSpring       = spelldata.Ranked(5675, 10495, 10496, 10497)
	shamanWindfuryProc     = spelldata.Ranked(8233, 8236, 10484, 16361)
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
	if row := shamanEarthShock.Highest(); row != nil {
		rows[row.ID] = row
	}
	if row := shamanFrostShock.Highest(); row != nil {
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
		"duration_ns": nanos(shamanSearingTotem.Highest().Duration()),
	})
	// shocks.go registerEarthShockSpell: a binary hit from the highest rank's damage roll.
	effects = append(effects, map[string]any{"kind": "earth_shock", "spell_id": shamanEarthShock.Highest().ID})
	// shocks.go registerFrostShockSpell: the same shape on the Frost school.
	effects = append(effects, map[string]any{"kind": "frost_shock", "spell_id": shamanFrostShock.Highest().ID})
	effects = append(effects, shamanImbueEffects(sham, character)...)
	effects = append(effects, shamanTotemEffects(sham, character)...)
	// totems.go registerStrengthOfEarthTotemSpell: the earth totem's aura; its Strength reaches the
	// fight through the class's stat auras.
	if aura := character.GetAura("Strength Of Earth Totem (Self)"); aura != nil {
		effects = append(effects, map[string]any{
			"kind": "strength_of_earth_totem", "spell_id": shamanStrengthOfEarth.Highest().ID, "aura": aura.Label,
			"duration_ns": nanos(shamanStrengthOfEarth.Highest().Duration()),
		})
	}
	if talents.Stormstrike { // stormstrike.go: the target debuff raises this shaman's lightning damage.
		row := shamanStormstrike.Highest()
		effects = append(effects, map[string]any{
			"kind": "stormstrike", "spell_id": row.ID, "aura": "Stormstrike-" + character.Label,
			"damage_multiplier": 1 + row.Effect(dbcenums.A_MOD_SPELL_DAMAGE_FROM_CASTER, 0).Percent(),
			"has_main_hand":     character.HasMHWeapon(), "has_off_hand": character.HasOHWeapon(),
		})
	}
	if talents.ElementalDevastation > 0 { // talents_elemental.go applyElementalDevastation
		effects = append(effects, map[string]any{
			"kind": "elemental_devastation", "trigger_aura": "Elemental Devastation Trigger", "aura": "Elemental Devastation",
			"melee_crit": shamanDevastation.Effect(dbcenums.A_DUMMY, 0).ValueAt(talents.ElementalDevastation),
		})
	}
	if talents.Flurry > 0 { // talents_enhancement.go applyFlurry: a Go literal 500 ms charge cooldown.
		effects = append(effects, map[string]any{
			"kind": "flurry", "trigger_aura": "Flurry Trigger", "aura": "Flurry",
			"melee_speed_multiplier": shamanFlurry.MultiplierAt(talents.Flurry), "charge_icd_ns": nanos(500 * time.Millisecond),
			"max_stacks": int32(shamanFlurryBuff.Highest().ProcCharges),
		})
	}
	if talents.Stormstrike && talents.ImprovedStormstrike > 0 { // talents_enhancement.go applyImprovedStormstrike
		effects = append(effects, map[string]any{
			"kind": "improved_stormstrike", "trigger_aura": "Improved Stormstrike Trigger", "aura": "Improved Stormstrike",
			"reset_aura":                "Improved Stormstrike Reset",
			"proc_chance":               shamanImpStormstrike.EffectAt(1).FractionAt(talents.ImprovedStormstrike),
			"spirit_regen_rate_casting": shamanImpStormBuff.Highest().Effect(dbcenums.A_MOD_MANA_REGEN_INTERRUPT, 0).Percent(),
		})
	}
	// talents_enhancement.go applyMaelstromWeapon: 2 PPM a point, a Go literal, rolled per hand.
	if trigger := character.GetAura("Maelstrom Weapon Trigger"); talents.MaelstromWeapon > 0 && trigger != nil && trigger.Dpm != nil {
		effects = append(effects, map[string]any{
			"kind": "maelstrom_weapon", "trigger_aura": trigger.Label, "aura": "Maelstrom Weapon",
			"per_stack":  shamanMaelstrom.EffectAt(1).FractionAt(talents.MaelstromWeapon),
			"max_stacks": int32(5),
			"chances": dpmChances(character, trigger.Dpm, nil, func(spell *core.Spell) bool {
				return spell.ProcMask.Matches(core.ProcMaskMelee) && !spell.Flags.Matches(core.SpellFlagProc)
			}),
		})
	}
	// weapon_imbues.go RegisterRockbiterImbue: a permanent temporary stats aura, already in the
	// prepared stats, that logs its gain and loss.
	if aura := character.GetAura("Rockbiter Weapon"); aura != nil {
		bonus := stats.Stats{stats.AttackPower: shamanRockbiter.Highest().EffectN(1).Average(core.CharacterLevel) *
			(1 + shamanElementalWeapons.EffectAt(1).FractionAt(talents.ElementalWeapons))}
		effects = append(effects, map[string]any{
			"kind": "rockbiter_weapon", "aura": aura.Label,
			"gain_log":   fmt.Sprintf("Gained %s from %s.", bonus.FlatString(), aura.ActionID),
			"expire_log": fmt.Sprintf("Lost %s from fading %s.", bonus.FlatString(), aura.ActionID),
		})
	}
	if talents.RageOfTheFarseer { // talents_enhancement.go applyRageOfTheFarseer
		row := shamanFarseer.Highest()
		effects = append(effects, map[string]any{
			"kind": "rage_of_the_farseer", "spell_id": row.ID, "aura": "Rage of the Farseer",
			"melee_speed_multiplier": 1 + row.Effect(dbcenums.A_MOD_MELEE_RANGED_HASTE_2, 0).Percent(),
		})
	}
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

// fire_totems.go, totems.go and shields.go: the totems and the shield the shaman may cast.
func shamanTotemEffects(sham *shaman.Shaman, character *core.Character) []map[string]any {
	effects := []map[string]any{}
	// registerMagmaTotemSpell: an area dot on the shaman whose pulses roll hit and crit on each
	// target from the pulse's damage row average.
	effects = append(effects, map[string]any{
		"kind": "magma_totem", "spell_id": shamanMagmaTotem.Highest().ID,
		"pulse_damage": shamanMagmaPulse.DamageEffect().Average(core.CharacterLevel),
		"duration_ns":  nanos(shamanMagmaTotem.Highest().Duration()),
	})
	// registerLightningShieldSpell: the cast puts up every charge. Its trigger hears only the
	// shield self proc, which needs a proc rate the exporter rejects, so the orb never fires.
	if aura := character.GetAura("Lightning Shield"); aura != nil {
		effects = append(effects, map[string]any{
			"kind": "lightning_shield", "spell_id": shamanLightningShield.Highest().ID, "aura": aura.Label,
			"charges": int32(shamanLightningShield.Highest().ProcCharges),
		})
	}
	// registerGraceOfAirTotemSpell: the air totem's aura, whose Agility is a class stat aura.
	if aura := character.GetAura("Grace Of Air Totem (Self)"); aura != nil {
		effects = append(effects, map[string]any{
			"kind": "grace_of_air_totem", "spell_id": shamanGraceOfAir.Highest().ID, "aura": aura.Label,
			"duration_ns":     nanos(shamanGraceOfAir.Highest().Duration()),
			"party_air_totem": character.GetAura("Windfury Totem") != nil || character.GetAura("Grace of Air Totem (External)") != nil,
		})
	}
	// registerManaSpringTotemSpell: the water totem's aura, whose MP5 is a class stat aura.
	if aura := character.GetAura("Mana Spring Totem (Self)"); aura != nil {
		effects = append(effects, map[string]any{
			"kind": "mana_spring_totem", "spell_id": shamanManaSpring.Highest().ID, "aura": aura.Label,
			"duration_ns": nanos(shamanManaSpring.Highest().Duration()),
		})
	}
	// registerFlametongueTotemSpell and buffs/flametongue_totem.go: the totem's aura turns on the
	// trigger, which casts the hit off landed main hand autos, unless a main hand Flametongue Weapon
	// or the party's totem holds the benefit.
	if aura := sham.FlametongueTotemAura; aura != nil {
		trigger := spelldata.ProcTrigger(character, spelldata.MustFind(15036), nil)
		trigger.ProcMask &= core.ProcMaskMeleeMH
		attack := -1
		for i, spell := range character.Spellbook {
			if spell.ActionID == (core.ActionID{SpellID: 16389}) {
				attack = i
			}
		}
		weapon := character.MainHand()
		effects = append(effects, map[string]any{
			"kind": "flametongue_totem", "spell_id": shamanFlametongueTotem.Highest().ID, "aura": aura.Label,
			"trigger_aura": buffs.FlametongueTotemTriggerLabel, "attack_spell": attack,
			"attack_deals_damage": weapon != nil && weapon.SwingSpeed != 0,
			"attack_damage":       buffs.FlametongueTotemBaseDamage(weapon.SwingSpeed),
			"trigger_spells":      procTriggerSpells(character, trigger), "trigger_outcome": outcomeNames(trigger.Outcome),
			"disabled_by_weapon": character.GetAura("Flametongue Imbue ItemSlotMainHand") != nil,
			"duration_ns":        nanos(shamanFlametongueTotem.Highest().Duration()),
			"party_totem":        character.GetAura("Flametongue Totem") != nil,
		})
	}
	return effects
}

// weapon_imbues.go: Flametongue and Frostbrand Weapon, weapon procs that cast an imbue hit.
func shamanImbueEffects(sham *shaman.Shaman, character *core.Character) []map[string]any {
	effects := []map[string]any{}
	// RegisterFlametongueImbue: one trigger and one hit spell for each imbued weapon, main hand first,
	// registered in that order, the hit from the weapon's speed held to 1.3 to 4.0.
	flametongue := []map[string]any{}
	hits := []int{}
	for i, spell := range character.Spellbook {
		if spell.ActionID == (core.ActionID{SpellID: shamanFlametongueProc.ID}) {
			hits = append(hits, i)
		}
	}
	for _, hand := range []struct {
		label  string
		weapon *core.Item
		mask   core.ProcMask
	}{
		{"Flametongue Imbue ItemSlotMainHand", character.MainHand(), core.ProcMaskMeleeMH},
		{"Flametongue Imbue ItemSlotOffHand", character.OffHand(), core.ProcMaskMeleeOH},
	} {
		if character.GetAura(hand.label) == nil {
			continue
		}
		if len(flametongue) >= len(hits) {
			fail(fmt.Errorf("%s has no hit spell", hand.label))
		}
		speed := min(max(hand.weapon.SwingSpeed, 1.3), 4)
		flametongue = append(flametongue, map[string]any{
			"trigger_aura": hand.label, "spell": hits[len(flametongue)], "deals_damage": hand.weapon.SwingSpeed != 0,
			"base_damage":    speed * shamanFlametongueProc.EffectN(1).Average(core.CharacterLevel) / 100,
			"trigger_spells": procTriggerSpells(character, core.ProcTrigger{ProcMask: hand.mask, IsWeaponProc: true}),
		})
	}
	if len(flametongue) > 0 {
		effects = append(effects, map[string]any{"kind": "flametongue_weapon", "hands": flametongue})
	}
	// RegisterWindfuryImbue: a weapon proc with its own cooldown that grants charges of attack
	// power and two extra attacks of the hand that procced it; landed autos spend the charges a
	// spell batch window later.
	if trigger := character.GetAura("Windfury Imbue"); trigger != nil && trigger.Dpm != nil {
		ap := character.GetAura("Windfury Weapon Attack Power")
		extra, offHand := -1, -1
		for i, spell := range character.Spellbook {
			switch spell.ActionID {
			case core.ActionID{OtherID: proto.OtherAction_OtherActionAttack, Tag: shamanWindfuryProc.Highest().ID}:
				extra = i
			case core.ActionID{OtherID: proto.OtherAction_OtherActionAttack, Tag: 2}:
				offHand = i
			}
		}
		var mask core.ProcMask
		if character.MainHand().TempEnchant == 283 {
			mask |= core.ProcMaskMeleeMH
		}
		if character.OffHand().TempEnchant == 283 {
			mask |= core.ProcMaskMeleeOH
		}
		bonus := stats.Stats{stats.AttackPower: sham.WindfuryAPBonus * (1 + shamanElementalWeapons.EffectAt(3).FractionAt(sham.Talents.ElementalWeapons))}
		if ap == nil || extra < 0 || trigger.Icd == nil {
			fail(fmt.Errorf("Windfury Weapon is incomplete"))
		}
		effects = append(effects, map[string]any{
			"kind": "windfury_weapon", "trigger_aura": trigger.Label,
			"trigger_spells": procTriggerSpells(character, core.ProcTrigger{ProcMask: mask, IsWeaponProc: true}),
			"chances": dpmChances(character, trigger.Dpm, nil, func(spell *core.Spell) bool {
				return !spell.Flags.Matches(core.SpellFlagSuppressWeaponProcs)
			}),
			"main_hand_spells": procTriggerSpells(character, core.ProcTrigger{ProcMask: core.ProcMaskMeleeMH, IsWeaponProc: true}),
			"ap_aura":          ap.Label, "extra_spell": extra, "off_hand_spell": offHand,
			// A main hand imbue outbids the party Windfury Totem's category effect.
			"blocks_windfury_totem": mask.Matches(core.ProcMaskMeleeMH),
			"spend_spells":          procTriggerSpells(character, core.ProcTrigger{ProcMask: core.ProcMaskMeleeMHAuto | core.ProcMaskMeleeOHAuto}),
			"ap_gain_log":           fmt.Sprintf("Gained %s from %s.", bonus.FlatString(), ap.ActionID),
			"ap_expire_log":         fmt.Sprintf("Lost %s from fading %s.", bonus.FlatString(), ap.ActionID),
		})
	}
	// RegisterFrostbrandImbue: 8 procs a minute, a Go literal, from the hands it imbues.
	if trigger := character.GetAura("Frostbrand Imbue"); trigger != nil && trigger.Dpm != nil {
		row := shamanFrostbrandProc.Highest()
		effects = append(effects, map[string]any{
			"kind": "frostbrand_weapon", "trigger_aura": trigger.Label, "spell_id": row.ID,
			"base_damage": row.DamageEffect().Average(core.CharacterLevel),
			"chances": dpmChances(character, trigger.Dpm, nil, func(spell *core.Spell) bool {
				return !spell.Flags.Matches(core.SpellFlagSuppressWeaponProcs)
			}),
		})
	}
	return effects
}

// The class auras whose gain and loss change stats through AddStatsDynamic.
func shamanStatAuras(_ core.Agent, _ *core.Character) []string {
	return []string{"Strength Of Earth Totem (Self)", "Grace Of Air Totem (Self)", "Mana Spring Totem (Self)",
		"Windfury Weapon Attack Power"}
}

// enhancement.go ApplySyncType: every sync type's replacement returns the main hand swing it
// is given; the weapon_sync effect describes how it moves the off hand swing first.
func shamanSwingReplacementKeepsSwing(_ core.Agent, player *proto.Player) bool {
	return player.GetEnhancementShaman().GetOptions() != nil
}

// enhancement.go ApplySyncType: the main hand swing replacement that moves the off hand
// swing, with Flurry's charge cooldown a Go literal.
func shamanPlayerEffects(character *core.Character, player *proto.Player) []map[string]any {
	options := player.GetEnhancementShaman().GetOptions()
	if options == nil {
		return nil
	}
	sync := ""
	switch options.SyncType {
	case proto.ShamanSyncType_Auto:
		sync = "auto"
		if character.MainHand().SwingSpeed != character.OffHand().SwingSpeed {
			sync = "none"
		}
	case proto.ShamanSyncType_SyncMainhandOffhandSwings:
		sync = "sync"
	case proto.ShamanSyncType_DelayOffhandSwings:
		sync = "delay"
	default:
		return nil
	}
	return []map[string]any{{"kind": "weapon_sync", "sync": sync, "flurry_icd_ns": nanos(500 * time.Millisecond)}}
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
