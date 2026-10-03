// Pet export: the pet Rust simulates, as Go core/pet.go builds it: one enabled at reset by a class
// whose pets are never summoned, dismissed or expired during a fight. Pets that are never enabled
// are inert_pet effects.
package main

import (
	"fmt"

	"github.com/wowsims/forever/sim/core"
	"github.com/wowsims/forever/sim/core/proto"
	"github.com/wowsims/forever/sim/core/simsignals"
)

// Classes whose pets are enabled only at reset, by EnabledOnStart, and never summoned, dismissed
// or expired during a fight. Each class file that registers such pets adds itself.
var resetOnlyPetClasses = map[proto.Class]bool{}

type PetMana struct {
	Max                      float64 `json:"max"`
	RegenPerSecondCasting    float64 `json:"regen_per_second_casting"`
	RegenPerSecondNotCasting float64 `json:"regen_per_second_not_casting"`
}

type Pet struct {
	Unit
	Name           string          `json:"name"`
	ReactionNs     int64           `json:"reaction_ns"`
	DistanceYards  float64         `json:"distance_yards"`
	CastSpeed      float64         `json:"cast_speed"`
	Mana           PetMana         `json:"mana"`
	AttackTable    AttackTable     `json:"attack_table"`
	Melee          Melee           `json:"melee"`
	Spells         []Spell         `json:"spells"`
	MetricsActions []MetricsAction `json:"metrics_actions"`
	// pet.go Enable's log lines after "Dynamic stat change": the pet's stats and what it
	// inherited; and Disable's stats line once the inheritance is gone.
	SummonLog   []string `json:"summon_log"`
	DismissLog  string   `json:"dismiss_log"`
	DynamicStat bool     `json:"dynamic_stats"`
}

// Whether Rust simulates the pet: enabled at reset by a class whose pets only change there.
func simulatedPet(character *core.Character, pet *core.Pet) bool {
	return resetOnlyPetClasses[character.Class] && pet.EnabledOnStart()
}

// The simulated pets of the player, in Go registration order. A feature Rust does not model is
// named in unrepresented.
func exportPets(request *proto.RaidSimRequest, character *core.Character, target *core.Unit, class classExport,
	timers *timerNames, unrepresented *[]string) []Pet {
	note := func(condition bool, message string) {
		if condition {
			*unrepresented = append(*unrepresented, message)
		}
	}
	pets := []Pet{}
	for index, agent := range character.PetAgents {
		pet := agent.GetPet()
		if !simulatedPet(character, pet) {
			continue
		}
		label := pet.Label
		note(pet.IsGuardian(), fmt.Sprintf("pet %s is a guardian", label))
		note(!pet.IsEnabled(), fmt.Sprintf("pet %s is not enabled after the reset", label))
		note(!pet.HasManaBar(), fmt.Sprintf("pet %s has no mana bar", label))
		note(privateField(pet, "hasDynamicMeleeSpeedInheritance").Bool(), fmt.Sprintf("pet %s inherits melee speed", label))
		note(privateField(pet, "hasDynamicCastSpeedInheritance").Bool(), fmt.Sprintf("pet %s inherits cast speed", label))
		note(privateField(pet, "hasResourceRegenInheritance").Bool(), fmt.Sprintf("pet %s inherits resource regeneration", label))
		note(privateField(pet, "startAttackDelay").Int() != 0, fmt.Sprintf("pet %s delays its first attack", label))
		note(pet.OnPetEnable != nil || pet.OnPetDisable != nil, fmt.Sprintf("pet %s has enable or disable callbacks", label))
		note(pet.HasFocusBar() || pet.HasEnergyBar(), fmt.Sprintf("pet %s has a focus or energy bar", label))
		note(len(pet.Pets) != 0, fmt.Sprintf("pet %s has pets", label))
		note(len(pet.OnCastSpeedChanged) != 0 || len(pet.OnTemporaryStatsChanges) != 0, fmt.Sprintf("pet %s has speed or stat listeners", label))
		note(pet.GetMajorCooldowns() != nil && len(pet.GetMajorCooldowns()) != 0, fmt.Sprintf("pet %s has major cooldowns", label))
		table := pet.AttackTables[target.UnitIndex]
		note(table.DamageDoneByCasterMultiplier != nil || len(table.DamageDoneByCasterExtraMultiplier) != 0,
			fmt.Sprintf("pet %s has caster damage callbacks", label))
		spells := []Spell{}
		for _, spell := range pet.Spellbook {
			spells = append(spells, exportSpell(spell, target, class, timers, unrepresented))
		}
		summon := []string{
			fmt.Sprintf("Pet stats: %s", pet.GetStats().FlatString()),
			fmt.Sprintf("Pet inherited stats: %s", pet.ApplyStatDependencies(pet.GetInheritedStats()).FlatString()),
		}
		dismiss := petDismissStats(request, index)
		pets = append(pets, Pet{
			Unit: Unit{Index: pet.UnitIndex, Label: pet.Label, Level: pet.Level, Stats: statValues(pet.GetStats()),
				PseudoStats: exportPseudo(pet.PseudoStats), Auras: exportAuras(&pet.Unit, timers)},
			Name: pet.Name, ReactionNs: nanos(pet.ReactionTime),
			DistanceYards: pet.DistanceFromTarget, CastSpeed: pet.CastSpeed,
			Mana: PetMana{Max: pet.MaxMana(), RegenPerSecondCasting: pet.ManaRegenPerSecondWhileCasting(),
				RegenPerSecondNotCasting: pet.ManaRegenPerSecondWhileNotCasting()},
			AttackTable: AttackTable{BaseSpellMissChance: table.BaseSpellMissChance, SpellCritSuppression: table.SpellCritSuppression,
				BonusSpellCritPercent: table.BonusSpellCritPercent, CritMultiplier: table.CritMultiplier,
				DamageDealtMultiplier: table.DamageDealtMultiplier, DamageTakenMultiplier: table.DamageTakenMultiplier},
			Melee:  exportMelee(&pet.Character, target, table, false, unrepresented),
			Spells: spells, MetricsActions: metricsActions(&pet.Unit),
			SummonLog: summon, DismissLog: dismiss,
			DynamicStat: privateField(pet, "isDynamic").Bool(),
		})
	}
	note(len(pets) > 1, "more than one pet is enabled at the start")
	return pets
}

// The stats line Disable logs: the pet's stats once its inheritance is removed, from a separate
// reset simulation. Pet.doneIteration expires the pet's auras first, undoing their stat buffs.
func petDismissStats(request *proto.RaidSimRequest, index int) string {
	simulation := core.NewSim(request, simsignals.CreateSignals())
	simulation.Reset()
	pet := simulation.Raid.Parties[0].Players[0].GetCharacter().PetAgents[index].GetPet()
	for _, aura := range pet.GetAuras() {
		if aura.IsActive() {
			aura.Deactivate(simulation)
		}
	}
	pet.Disable(simulation)
	return pet.GetStats().FlatString()
}
