// Weapon procs of common/itemhelpers/weaponprocs.go: a chance on hit that casts the spell of a
// client row on the unit hit. Each formula mirrors the cited Go file at the pinned revision.
package main

import (
	"fmt"

	"github.com/wowsims/forever/sim/core"
	"github.com/wowsims/forever/sim/core/spelldata"
)

// common/forever/items_weapons.go: weapons whose chance on hit casts a spell of the client's rows,
// registered through itemhelpers.CreateWeaponProcSpell over shared.SpellDataProcDamageSpell. The
// trigger aura is the weapon's name and " Proc".
var weaponDamageProcs = []struct {
	label  string
	damage int32
}{{"Barbaric Crossbow", 1291551}, {"Plaguefang", 1309315}, {"Wolfsbane", 1282503}, {"Venomstrike", 29653},
	{"Stinging Viper", 1291663}, {"Alcor's Sunrazor", 18833}, {"Bloodfist", 16433}, {"Bonechill Hammer", 18276},
	{"Darrowspike", 18276}, {"Coldrage Dagger", 1293790}, {"Glacial Blade", 18398}, {"Flame Wrath", 16559},
	{"Masterwork Stormhammer", 16921}, {"Electrified Dagger", 23592}, {"Shadowstrike", 21170},
	{"The Cruel Hand of Timmy", 17505}, {"Skullforge Reaver", 17484}}

// shared_utils.go damageOutcome by the runtime's name for a hit of the spell's defense type. A
// magic hit has no name: the effect's can_crit picks between the two magic appliers, as it does for
// every spell data damage proc.
func procHitOutcome(defense core.DefenseType, cannotCrit bool) string {
	switch {
	case defense == core.DefenseTypeMelee && cannotCrit:
		return "melee_special_hit"
	case defense == core.DefenseTypeMelee:
		return "melee_special_hit_and_crit"
	case defense == core.DefenseTypeRanged && cannotCrit:
		return "ranged_hit"
	case defense == core.DefenseTypeRanged:
		return "ranged_hit_and_crit"
	}
	return ""
}

// spelldata Spell.TickOutcome by the runtime's name: the tick of a dot the weapon proc casts rolls
// the hit again where the row's defense type is magic.
func procTickOutcome(row *spelldata.Spell) string {
	canCrit, magic := row.PeriodicCanCrit(), row.DefenseTypeCore() == core.DefenseTypeMagic
	switch {
	case canCrit && magic:
		return "tick_magic_hit_and_crit"
	case canCrit:
		return "tick_physical_crit"
	case magic:
		return "tick_magic_hit"
	}
	return "tick"
}

// The fields of a spell_data_damage_proc effect that the damage row and the spell registered for it
// decide, whoever listens: the spell, the row's direct hit and the table it rolls, the targets of an
// area or chain, and the damage over time the row carries. The reason the runtime cannot run the row,
// phrased for "<item>'s proc ...", is returned instead where there is one.
func procDamageShape(character *core.Character, damage *spelldata.Spell, spell int) (map[string]any, string) {
	direct, periodic := damage.DamageEffect(), damage.PeriodicDamageEffect()
	defense := character.Spellbook[spell].DefenseType
	outcome := procHitOutcome(defense, damage.CannotCrit())
	switch {
	case direct == spelldata.NilEffect && periodic == spelldata.NilEffect:
		return nil, "deals no damage"
	case damage.DebuffsTheTarget():
		return nil, "debuffs the target"
	case periodic != spelldata.NilEffect && (direct.HitsAnArea() || direct.ChainTargets > 1):
		return nil, "spreads and leaves a damage over time"
	case periodic != spelldata.NilEffect && direct == spelldata.NilEffect && damage.Speed != 0:
		return nil, "missile carries only a damage over time"
	case periodic != spelldata.NilEffect && procTickOutcome(damage) == "tick_magic_hit":
		return nil, "ticks roll a magic hit without a crit"
	case defense != core.DefenseTypeMagic && defense != core.DefenseTypeMelee && defense != core.DefenseTypeRanged:
		return nil, "rolls no known hit table"
	}
	shape := map[string]any{"spell": spell, "average": 0.0, "variance": 0.0, "can_crit": !damage.CannotCrit()}
	if direct != spelldata.NilEffect {
		shape["average"], shape["variance"] = direct.Average(character.Level), direct.Variance
		if outcome != "" {
			shape["outcome"] = outcome
		}
		switch {
		case direct.HitsAnArea():
			shape["area"] = map[string]any{"max_targets": int32(damage.MaxTargets), "splits": damage.SplitsDamage,
				"aoe_cap_multiplier": character.Env.Encounter.AOECapMultiplier()}
		case direct.ChainTargets > 1:
			shape["chain"] = map[string]any{"targets": int32(direct.ChainTargets), "amp": float64(direct.ChainAmp)}
		}
	}
	if periodic != spelldata.NilEffect {
		shape["periodic"] = map[string]any{"tick_base": periodic.Average(character.Level),
			"tick_outcome": procTickOutcome(damage), "with_direct": direct != spelldata.NilEffect}
	}
	return shape, ""
}

// weaponDamageProcEffects describes the weapon procs above. The spell is the one
// shared_utils.go spellDataProcDamageSpell builds, cast at once by the handler on the unit hit
// (procDamageHandler, TriggerImmediately): the row's direct hit rolled once, or for a chain once a
// target on the amount the previous jump keeps, on the table the spell's defense type names, then
// the damage over time the row carries on the targets it landed on. An area hit is calculated on
// every target, or on the row's cap of them from the target out, and a split one shares one roll.
// A hit with a missile speed is dealt after its travel. A row that also puts a debuff on the target
// is not described.
func weaponDamageProcEffects(simulation *core.Simulation, character *core.Character, unrepresented *[]string) []map[string]any {
	effects := []map[string]any{}
	for _, proc := range weaponDamageProcs {
		aura := character.GetAura(proc.label + " Proc")
		if aura == nil {
			continue
		}
		damage := spelldata.MustFind(proc.damage)
		spell := -1
		for i, registered := range character.Spellbook {
			if registered.ActionID == (core.ActionID{SpellID: damage.ID}) {
				spell = i
			}
		}
		if aura.Dpm == nil || aura.Icd != nil || spell < 0 {
			*unrepresented = append(*unrepresented, fmt.Sprintf("%s's proc has no proc manager or spell", proc.label))
			continue
		}
		shape, reason := procDamageShape(character, damage, spell)
		if reason != "" {
			*unrepresented = append(*unrepresented, fmt.Sprintf("%s's proc %s", proc.label, reason))
			continue
		}
		chances := dpmChances(character, aura.Dpm, simulation, func(spell *core.Spell) bool {
			return !spell.Flags.Matches(core.SpellFlagSuppressWeaponProcs)
		})
		triggers := []int{}
		for _, chance := range chances {
			triggers = append(triggers, chance.Spell)
		}
		effect := map[string]any{
			"kind": "spell_data_damage_proc", "trigger_aura": aura.Label, "trigger_spells": triggers,
			"landed_only": true, "require_damage": false, "proc_chance": 1.0, "chances": chances,
		}
		for key, value := range shape {
			effect[key] = value
		}
		effects = append(effects, effect)
	}
	return effects
}

// common/forever/items_weapons.go Flurry Axe: a weapon proc on landed hits, at the weapon's proc
// manager, that casts a spell whose effect is one extra main hand attack at once
// (AutoAttacks.ExtraMHAttack). The spell deals nothing of its own.
func flurryAxeEffects(simulation *core.Simulation, character *core.Character, unrepresented *[]string) []map[string]any {
	aura := character.GetAura("Flurry Axe Proc")
	if aura == nil {
		return nil
	}
	spell := -1
	for i, registered := range character.Spellbook {
		if registered.ActionID == (core.ActionID{SpellID: 18797}) {
			spell = i
		}
	}
	if aura.Dpm == nil || aura.Icd != nil || spell < 0 {
		*unrepresented = append(*unrepresented, "Flurry Axe's proc has no proc manager or spell")
		return nil
	}
	chances := dpmChances(character, aura.Dpm, simulation, func(spell *core.Spell) bool {
		return !spell.Flags.Matches(core.SpellFlagSuppressWeaponProcs)
	})
	return []map[string]any{{
		"kind": "extra_attack_proc", "trigger_aura": aura.Label, "proc_chance": 1.0, "attacks": int32(1),
		"chances": chances, "spell": spell,
	}}
}

// common/forever/items_weapons.go Sword of Zeal and Argent Avenger: weapons whose chance on hit
// activates an aura of the row on the wearer, registered through itemhelpers.CreateWeaponProcAura.
// The aura is the row's effects parsed (spelldata.ParseEffects), the trigger the weapon's name and
// " Proc". Bonereaver's Edge is the same through itemhelpers.CreateWeaponProcTrigger, whose
// handler activates the stacking aura core.MakeStackingAura builds and adds a stack.
var weaponAuraProcAuras = []string{"Sword of Zeal", "Argent Avenger", "Bonereaver's Edge"}

// The weapon procs above whose handler also adds a stack of the aura.
var weaponAuraProcStacks = map[string]bool{"Bonereaver's Edge": true}

// weaponAuraProcEffects describes those weapon procs: a weapon proc on landed hits at the weapon's
// proc manager whose handler, a spell batch window later, activates the aura and, for a weapon
// that stacks it, adds a stack. The aura logs nothing but the generic lines, and its stat changes
// are read as stat auras, by stack where the stats follow them.
func weaponAuraProcEffects(simulation *core.Simulation, character *core.Character, unrepresented *[]string) []map[string]any {
	effects := []map[string]any{}
	for _, name := range weaponAuraProcAuras {
		trigger := character.GetAura(name + " Proc")
		if trigger == nil {
			continue
		}
		aura := character.GetAura(name)
		stacks := weaponAuraProcStacks[name]
		if trigger.Dpm == nil || trigger.Icd != nil || aura == nil || (aura.MaxStacks > 0) != stacks {
			*unrepresented = append(*unrepresented, fmt.Sprintf("%s's proc is not a chance on hit for its aura", name))
			continue
		}
		effect := map[string]any{
			"kind": "stat_proc", "trigger_aura": trigger.Label, "rng_label": trigger.Label, "aura": aura.Label,
			"chances": dpmChances(character, trigger.Dpm, simulation, func(spell *core.Spell) bool {
				return !spell.Flags.Matches(core.SpellFlagSuppressWeaponProcs)
			}),
		}
		if stacks {
			effect["add_stack"] = true
		}
		effects = append(effects, effect)
	}
	return effects
}

// common/forever/items_weapons.go The Lobotomizer: a weapon proc on landed hits, at the weapon's
// proc manager, that casts Brain Damage (1290950), a physical spell of the melee defense type
// that rolls 200 to 300 on the magic hit table with a crit, at once on the unit hit.
func lobotomizerEffects(simulation *core.Simulation, character *core.Character, unrepresented *[]string) []map[string]any {
	aura := character.GetAura("The Lobotomizer Proc")
	if aura == nil {
		return nil
	}
	spell := -1
	for i, registered := range character.Spellbook {
		if registered.ActionID == (core.ActionID{SpellID: 1290950}) {
			spell = i
		}
	}
	if aura.Dpm == nil || aura.Icd != nil || spell < 0 {
		*unrepresented = append(*unrepresented, "The Lobotomizer's proc has no proc manager or spell")
		return nil
	}
	chances := dpmChances(character, aura.Dpm, simulation, func(spell *core.Spell) bool {
		return !spell.Flags.Matches(core.SpellFlagSuppressWeaponProcs)
	})
	triggers := []int{}
	for _, chance := range chances {
		triggers = append(triggers, chance.Spell)
	}
	return []map[string]any{{
		"kind": "spell_data_damage_proc", "trigger_aura": aura.Label, "trigger_spells": triggers,
		"landed_only": true, "require_damage": false, "proc_chance": 1.0, "chances": chances,
		"spell": spell, "average": 0.0, "variance": 0.0, "roll": []float64{200, 300}, "can_crit": true,
	}}
}

// common/classic/items_weapons.go Ebon Hilt of Marduk: a weapon proc on landed hits, at the weapon's
// proc manager, that casts Corruption (18656): a magic hit roll without damage whose landing starts
// a damage over time of 28 every 3 seconds for 3 ticks (Dot.Snapshot of a flat amount, ticking
// OutcomeTick on current stats, which a spell without a spell power share deals as it would any
// periodic amount). The item also holds a permanent aura that lowers the wearer's threat by 1%,
// which the player's threat multiplier already carries.
func ebonHiltEffects(simulation *core.Simulation, character *core.Character, unrepresented *[]string) []map[string]any {
	aura := character.GetAura("Ebon Hilt of Marduk Proc")
	if aura == nil {
		return nil
	}
	spell := -1
	for i, registered := range character.Spellbook {
		if registered.ActionID == (core.ActionID{SpellID: 18656}) {
			spell = i
		}
	}
	if aura.Dpm == nil || aura.Icd != nil || spell < 0 || character.Spellbook[spell].DefenseType != core.DefenseTypeMagic {
		*unrepresented = append(*unrepresented, "Ebon Hilt of Marduk's proc has no proc manager or spell")
		return nil
	}
	chances := dpmChances(character, aura.Dpm, simulation, func(spell *core.Spell) bool {
		return !spell.Flags.Matches(core.SpellFlagSuppressWeaponProcs)
	})
	triggers := []int{}
	for _, chance := range chances {
		triggers = append(triggers, chance.Spell)
	}
	return []map[string]any{{
		"kind": "spell_data_damage_proc", "trigger_aura": aura.Label, "trigger_spells": triggers,
		"landed_only": true, "require_damage": false, "proc_chance": 1.0, "chances": chances,
		"spell": spell, "average": 0.0, "variance": 0.0, "can_crit": false,
		"periodic": map[string]any{"tick_base": 28.0, "tick_outcome": "tick", "with_direct": false,
			"application": "magic_hit"},
	}}
}

// common/classic/items_weapons.go Thunderfury, Blessed Blade of the Windseeker: a weapon proc on
// landed hits, at the weapon's proc manager, whose handler a spell batch window later casts two
// spells on the unit hit. The first (tag 1) is a nature hit of 300 on the magic table with a crit,
// whose landing puts Cyclone on the target: a slow of 20% through core.AtkSpeedReductionEffect, an
// exclusive effect of the attack speed category Thunder Clap shares. The second (tag 2) deals no
// damage on the magic hit table to up to five targets from the unit hit, and each it lands on
// takes the Thunderfury aura: 25 less nature resistance while it lasts. Both auras last 12
// seconds. Every number is a Go literal of the item.
func thunderfuryEffects(simulation *core.Simulation, character *core.Character, unrepresented *[]string) []map[string]any {
	trigger := character.GetAura("Thunderfury Proc")
	if trigger == nil {
		return nil
	}
	strike, bounce := -1, -1
	for i, registered := range character.Spellbook {
		switch registered.ActionID {
		case core.ActionID{SpellID: 21992, Tag: 1}:
			strike = i
		case core.ActionID{SpellID: 21992, Tag: 2}:
			bounce = i
		}
	}
	target := character.Env.Encounter.ActiveTargetUnits[0]
	slow, resistance := target.GetAura("Cyclone"), target.GetAura("Thunderfury")
	if trigger.Dpm == nil || trigger.Icd != nil || strike < 0 || bounce < 0 || slow == nil || resistance == nil {
		*unrepresented = append(*unrepresented, "Thunderfury's proc has no proc manager, spells or auras")
		return nil
	}
	return []map[string]any{{
		"kind": "thunderfury", "trigger_aura": trigger.Label,
		"chances": dpmChances(character, trigger.Dpm, simulation, func(spell *core.Spell) bool {
			return !spell.Flags.Matches(core.SpellFlagSuppressWeaponProcs)
		}),
		"strike_spell": strike, "bounce_spell": bounce, "strike_damage": 300.0, "bounce_targets": int32(5),
		"slow_aura": slow.Label, "slow_multiplier": core.SlowedTimeMultiplier(-20),
		"resistance_aura": resistance.Label, "nature_resistance": -25.0,
	}}
}
