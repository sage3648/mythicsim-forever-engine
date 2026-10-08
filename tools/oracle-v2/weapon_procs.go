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
		direct, periodic := damage.DamageEffect(), damage.PeriodicDamageEffect()
		if aura.Dpm == nil || aura.Icd != nil || spell < 0 {
			*unrepresented = append(*unrepresented, fmt.Sprintf("%s's proc has no proc manager or spell", proc.label))
			continue
		}
		defense := character.Spellbook[spell].DefenseType
		outcome := procHitOutcome(defense, damage.CannotCrit())
		switch {
		case direct == spelldata.NilEffect && periodic == spelldata.NilEffect:
			*unrepresented = append(*unrepresented, fmt.Sprintf("%s's proc deals no damage", proc.label))
			continue
		case damage.DebuffsTheTarget():
			*unrepresented = append(*unrepresented, fmt.Sprintf("%s's proc debuffs the target", proc.label))
			continue
		case periodic != spelldata.NilEffect && (direct.HitsAnArea() || direct.ChainTargets > 1):
			*unrepresented = append(*unrepresented, fmt.Sprintf("%s's proc spreads and leaves a damage over time", proc.label))
			continue
		case periodic != spelldata.NilEffect && direct == spelldata.NilEffect && damage.Speed != 0:
			*unrepresented = append(*unrepresented, fmt.Sprintf("%s's missile carries only a damage over time", proc.label))
			continue
		case periodic != spelldata.NilEffect && procTickOutcome(damage) == "tick_magic_hit":
			*unrepresented = append(*unrepresented, fmt.Sprintf("%s's ticks roll a magic hit without a crit", proc.label))
			continue
		case defense != core.DefenseTypeMagic && defense != core.DefenseTypeMelee && defense != core.DefenseTypeRanged:
			*unrepresented = append(*unrepresented, fmt.Sprintf("%s's proc rolls no known hit table", proc.label))
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
			"spell": spell, "average": 0.0, "variance": 0.0, "can_crit": !damage.CannotCrit(),
		}
		if direct != spelldata.NilEffect {
			effect["average"], effect["variance"] = direct.Average(character.Level), direct.Variance
			if outcome != "" {
				effect["outcome"] = outcome
			}
			switch {
			case direct.HitsAnArea():
				effect["area"] = map[string]any{"max_targets": int32(damage.MaxTargets), "splits": damage.SplitsDamage,
					"aoe_cap_multiplier": character.Env.Encounter.AOECapMultiplier()}
			case direct.ChainTargets > 1:
				effect["chain"] = map[string]any{"targets": int32(direct.ChainTargets), "amp": float64(direct.ChainAmp)}
			}
		}
		if periodic != spelldata.NilEffect {
			effect["periodic"] = map[string]any{"tick_base": periodic.Average(character.Level),
				"tick_outcome": procTickOutcome(damage), "with_direct": direct != spelldata.NilEffect}
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
// " Proc".
var weaponAuraProcAuras = []string{"Sword of Zeal", "Argent Avenger"}

// weaponAuraProcEffects describes those weapon procs: a weapon proc on landed hits at the weapon's
// proc manager whose handler, a spell batch window later, activates the aura. Only an aura
// without stacks is described. The parsed aura logs nothing, and its stat changes are read as stat
// auras.
func weaponAuraProcEffects(simulation *core.Simulation, character *core.Character, unrepresented *[]string) []map[string]any {
	effects := []map[string]any{}
	for _, name := range weaponAuraProcAuras {
		trigger := character.GetAura(name + " Proc")
		if trigger == nil {
			continue
		}
		aura := character.GetAura(name)
		if trigger.Dpm == nil || trigger.Icd != nil || aura == nil || aura.MaxStacks > 0 {
			*unrepresented = append(*unrepresented, fmt.Sprintf("%s's proc is not a chance on hit for an aura without stacks", name))
			continue
		}
		effects = append(effects, map[string]any{
			"kind": "stat_proc", "trigger_aura": trigger.Label, "rng_label": trigger.Label, "aura": aura.Label,
			"chances": dpmChances(character, trigger.Dpm, simulation, func(spell *core.Spell) bool {
				return !spell.Flags.Matches(core.SpellFlagSuppressWeaponProcs)
			}),
		})
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
