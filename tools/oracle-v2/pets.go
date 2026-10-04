// Pet export: the pet Rust simulates, as Go core/pet.go builds it: one enabled at reset by a class
// whose pets are never summoned, dismissed or expired during a fight. Pets that are never enabled
// are inert_pet effects.
package main

import (
	"fmt"
	"math"
	"reflect"
	"unsafe"

	"github.com/wowsims/forever/sim/core"
	"github.com/wowsims/forever/sim/core/proto"
	"github.com/wowsims/forever/sim/core/simsignals"
	"github.com/wowsims/forever/sim/core/stats"
)

// Classes whose pets are enabled only at reset, by EnabledOnStart, and never summoned, dismissed
// or expired during a fight. Each class file that registers such pets adds itself.
var resetOnlyPetClasses = map[proto.Class]bool{}

type PetMana struct {
	Max                      float64 `json:"max"`
	RegenPerSecondCasting    float64 `json:"regen_per_second_casting"`
	RegenPerSecondNotCasting float64 `json:"regen_per_second_not_casting"`
}

// focus.go focusBar of a pet: Enable fills it and starts its regeneration task.
type PetFocus struct {
	Max            float64 `json:"max"`
	RegenPerTick   float64 `json:"regen_per_tick"`
	TickDurationNs int64   `json:"tick_duration_ns"`
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
	// Present for a pet with a focus bar and no mana bar.
	Focus *PetFocus `json:"focus,omitempty"`
	// Go GetMovementSpeed, for a pet that starts at its owner's distance and moves in.
	MovementSpeed float64 `json:"movement_speed,omitempty"`
	// A guardian an effect summons during a fight with a timeout: each reset dismisses it.
	Summoned bool `json:"summoned,omitempty"`
	// A dynamic pet's inheritance of its owner's stat changes, one owner stat for each pet stat,
	// and what Go recomputes its stats from: the stats before dependencies and the enabled
	// dependencies in their sorted order (unit.go AddStatsDynamic).
	Inheritance      []StatInheritance  `json:"inheritance,omitempty"`
	StatsWithoutDeps map[string]float64 `json:"stats_without_deps,omitempty"`
	Dependencies     []StatDependency   `json:"stat_dependencies,omitempty"`
	// A dynamic pet enabled at reset: what Disable takes away (pet.go inheritedStats), and
	// its stats line as name and value pairs in Go's stat order, the stats the inheritance
	// can change included even at zero, so the line follows the fight's float residue.
	InheritedStats map[string]float64 `json:"inherited_stats,omitempty"`
	DismissStats   []DismissStat      `json:"dismiss_stats,omitempty"`
	// What each permanent aura's expiry adds to the stats before dependencies.
	AuraStats []PetAuraStats `json:"aura_stats,omitempty"`
}

// The change a pet aura's expiry makes to the pet's stats before dependencies.
type PetAuraStats struct {
	Aura  string             `json:"aura"`
	Stats map[string]float64 `json:"stats"`
}

// One stat of a pet's dismissal line, as stats.go FlatString names it.
type DismissStat struct {
	Stat  string  `json:"stat"`
	Value float64 `json:"value"`
}

// An enabled stats.StatDependency.
type StatDependency struct {
	Src    string  `json:"src"`
	Dst    string  `json:"dst"`
	Amount float64 `json:"amount"`
	Step   float64 `json:"step,omitempty"`
}

// stats.go flooredGameStats, unexported.
var flooredGameStats = map[stats.Stat]bool{stats.Strength: true, stats.Agility: true, stats.Stamina: true,
	stats.Intellect: true, stats.Spirit: true}

type depTerm struct {
	src, dst     stats.Stat
	amount, step float64
}

// The pet's enabled stat dependencies, read from its private StatDependencyManager.
func petDependencies(pet *core.Pet) []depTerm {
	deps := privateField(pet, "StatDependencyManager").FieldByName("deps")
	terms := []depTerm{}
	for i := 0; i < deps.Len(); i++ {
		dep := deps.Index(i).Elem()
		if !dep.FieldByName("enabled").Bool() {
			continue
		}
		terms = append(terms, depTerm{src: stats.Stat(dep.FieldByName("src").Uint()), dst: stats.Stat(dep.FieldByName("dst").Uint()),
			amount: dep.FieldByName("amount").Float(), step: dep.FieldByName("step").Float()})
	}
	return terms
}

// deps.go ApplyStatDependencies, then FloorGameStats, as AddStatsDynamic recomputes stats.
func applyDependencies(without stats.Stats, deps []depTerm) stats.Stats {
	s := without
	for _, dep := range deps {
		switch {
		case dep.src == dep.dst:
			s[dep.dst] *= dep.amount
		case dep.step != 0:
			s[dep.dst] += math.Floor(s[dep.src]/dep.step) * dep.step * dep.amount
		case flooredGameStats[dep.src]:
			s[dep.dst] += math.Floor(s[dep.src]) * dep.amount
		default:
			s[dep.dst] += s[dep.src] * dep.amount
		}
	}
	return s.FloorGameStats()
}

// One term of a pet's linear stat inheritance: the pet stat gains coefficient times the owner
// stat's change.
type StatInheritance struct {
	Owner       string  `json:"owner"`
	Pet         string  `json:"pet"`
	Coefficient float64 `json:"coefficient"`
}

// A dynamic pet's PetStatInheritance, read from its private field, as linear terms. Go applies
// it to each batch of the owner's stat changes at the pet's next heartbeat (unit.go
// processDynamicBonus, pet.go AddOwnerStats). The terms are checked against Go on several
// changes, bit for bit, and so is AddOwnerStats on a reset pet, which also applies the pet's
// stat dependencies; anything else is unrepresented.
func petInheritance(request *proto.RaidSimRequest, index int, pet *core.Pet, note func(bool, string)) []StatInheritance {
	inherit := petStatInheritance(pet)
	terms := []StatInheritance{}
	sources := map[stats.Stat]stats.Stat{}
	coefficients := map[stats.Stat]float64{}
	for owner := stats.Stat(0); owner < stats.SimStatsLen; owner++ {
		unit := stats.Stats{}
		unit[owner] = 1
		inherited := inherit(unit)
		for petStat := stats.Stat(0); petStat < stats.SimStatsLen; petStat++ {
			if inherited[petStat] == 0 {
				continue
			}
			if _, taken := sources[petStat]; taken {
				note(true, fmt.Sprintf("pet %s inherits %s from more than one stat", pet.Label, petStat.StatName()))
			}
			sources[petStat] = owner
			coefficients[petStat] = inherited[petStat]
			terms = append(terms, StatInheritance{Owner: owner.StatName(), Pet: petStat.StatName(), Coefficient: inherited[petStat]})
		}
	}
	// Linear, bit for bit, on changes of either sign.
	for _, amount := range []float64{3.7, -12.25, 123.456, 0.1} {
		for petStat, owner := range sources {
			change := stats.Stats{}
			change[owner] = amount
			if got, want := inherit(change)[petStat], amount*coefficients[petStat]; got != want && !(math.IsNaN(got) && math.IsNaN(want)) {
				note(true, fmt.Sprintf("pet %s inherits %s nonlinearly", pet.Label, petStat.StatName()))
			}
		}
	}
	// AddOwnerStats on a reset pet recomputes its stats from the stats before dependencies and
	// the dependencies, as exported.
	deps := petDependencies(pet)
	if got := applyDependencies(pet.GetStatsWithoutDeps(), deps); got != pet.GetStats() {
		note(true, fmt.Sprintf("pet %s's stats do not follow its dependencies", pet.Label))
	}
	for owner := range func() map[stats.Stat]bool {
		owners := map[stats.Stat]bool{}
		for _, o := range sources {
			owners[o] = true
		}
		return owners
	}() {
		simulation := core.NewSim(request, simsignals.CreateSignals())
		simulation.Reset()
		resetPet := simulation.Raid.Parties[0].Players[0].GetCharacter().PetAgents[index].GetPet()
		without := resetPet.GetStatsWithoutDeps()
		change := stats.Stats{}
		change[owner] = 37.5
		resetPet.AddOwnerStats(simulation, change)
		inherited := inherit(change)
		without.AddInplace(&inherited)
		if applyDependencies(without, deps) != resetPet.GetStats() {
			note(true, fmt.Sprintf("pet %s's inherited %s does not follow its dependencies", pet.Label, owner.StatName()))
		}
	}
	return terms
}

// The pet's PetStatInheritance, read from its private field.
func petStatInheritance(pet *core.Pet) core.PetStatInheritance {
	field := privateField(pet, "statInheritance")
	return reflect.NewAt(field.Type(), unsafe.Pointer(field.UnsafeAddr())).Elem().Interface().(core.PetStatInheritance)
}

// Guardians a common effect summons during a fight, by pet name: common/classic
// emerald_dragon_whelp.go.
var summonedPetNames = map[string]bool{"Emerald Dragon Whelp": true}

// Pets a class effect summons during a fight, each recognized by its class file. The class
// effect describes the pet's enable and disable callbacks and the stats it inherits at each
// summon.
var classSummonedPets []func(pet *core.Pet) bool

func classSummonedPet(pet *core.Pet) bool {
	for _, summoned := range classSummonedPets {
		if summoned(pet) {
			return true
		}
	}
	return false
}

// Whether the pet is a guardian an exported effect summons during a fight, or a pet a class
// effect summons.
func summonedPet(pet *core.Pet) bool {
	return ((summonedPetNames[pet.Name] && pet.IsGuardian()) || classSummonedPet(pet)) && !pet.EnabledOnStart()
}

// Whether Rust simulates the pet: enabled at reset by a class whose pets only change there, or
// a guardian an effect summons.
func simulatedPet(character *core.Character, pet *core.Pet) bool {
	return (resetOnlyPetClasses[character.Class] && pet.EnabledOnStart()) || summonedPet(pet)
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
		summoned := summonedPet(pet)
		note(pet.IsGuardian() && !summoned, fmt.Sprintf("pet %s is a guardian", label))
		note(!pet.IsEnabled() && !summoned, fmt.Sprintf("pet %s is not enabled after the reset", label))
		note(pet.IsEnabled() && summoned, fmt.Sprintf("summoned pet %s is enabled after the reset", label))
		// A class summoned pet may have neither: Rust ticks and reports only the resources a
		// pet has.
		note(!pet.HasManaBar() && !pet.HasFocusBar() && !classSummonedPet(pet), fmt.Sprintf("pet %s has no mana bar", label))
		note(pet.HasManaBar() && pet.HasFocusBar(), fmt.Sprintf("pet %s has both mana and focus", label))
		note(privateField(pet, "hasDynamicMeleeSpeedInheritance").Bool(), fmt.Sprintf("pet %s inherits melee speed", label))
		note(privateField(pet, "hasDynamicCastSpeedInheritance").Bool(), fmt.Sprintf("pet %s inherits cast speed", label))
		note(privateField(pet, "hasResourceRegenInheritance").Bool(), fmt.Sprintf("pet %s inherits resource regeneration", label))
		note(privateField(pet, "startAttackDelay").Int() != 0, fmt.Sprintf("pet %s delays its first attack", label))
		note((pet.OnPetEnable != nil || pet.OnPetDisable != nil) && !classSummonedPet(pet),
			fmt.Sprintf("pet %s has enable or disable callbacks", label))
		note(pet.HasEnergyBar(), fmt.Sprintf("pet %s has an energy bar", label))
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
		dismissStats := petDismissStats(request, index)
		dismiss := dismissStats.FlatString()
		var focus *PetFocus
		if pet.HasFocusBar() {
			bar := privateField(&pet.Unit, "focusBar")
			focus = &PetFocus{Max: bar.FieldByName("maxFocus").Float(), RegenPerTick: bar.FieldByName("focusRegenPerTick").Float(),
				TickDurationNs: bar.FieldByName("focusTickDuration").Int()}
		}
		movementSpeed := 0.0
		if pet.DistanceFromTarget > core.MaxMeleeRange {
			movementSpeed = pet.GetMovementSpeed()
		}
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
			Focus:       focus, MovementSpeed: movementSpeed, Summoned: summoned,
		})
		if privateField(pet, "isDynamic").Bool() {
			exported := &pets[len(pets)-1]
			exported.Inheritance = petInheritance(request, index, pet, note)
			exported.StatsWithoutDeps = map[string]float64{}
			without := pet.GetStatsWithoutDeps()
			for s := stats.Stat(0); s < stats.SimStatsLen; s++ {
				if without[s] != 0 {
					exported.StatsWithoutDeps[s.StatName()] = without[s]
				}
			}
			for _, dep := range petDependencies(pet) {
				exported.Dependencies = append(exported.Dependencies, StatDependency{Src: dep.src.StatName(),
					Dst: dep.dst.StatName(), Amount: dep.amount, Step: dep.step})
			}
			if !summoned {
				exported.InheritedStats = map[string]float64{}
				inherited := pet.GetInheritedStats()
				for s := stats.Stat(0); s < stats.SimStatsLen; s++ {
					if inherited[s] != 0 {
						exported.InheritedStats[s.StatName()] = inherited[s]
					}
				}
				exported.DismissStats = petDismissStatList(dismissStats, exported)
				exported.AuraStats = petAuraFadeStats(request, index)
			}
		}
	}
	return pets
}

// The stats of a dynamic pet's dismissal line, in Go's stat order: each stat FlatString prints,
// and each stat the runtime can change during a fight, the tracked powers, the inherited stats
// and their dependencies, which a float residue can bring into the line.
func petDismissStatList(dismiss stats.Stats, pet *Pet) []DismissStat {
	tracked := map[string]bool{"SpellDamage": true, "AttackPower": true, "RangedAttackPower": true,
		"SpellCritPercent": true, "PhysicalCritPercent": true}
	for _, term := range pet.Inheritance {
		tracked[term.Pet] = true
	}
	for _, dep := range pet.Dependencies {
		tracked[dep.Dst] = true
	}
	list := []DismissStat{}
	for s := stats.Stat(0); s < stats.SimStatsLen; s++ {
		if name := s.StatName(); name != "none" && (dismiss[s] != 0 || tracked[name]) {
			list = append(list, DismissStat{Stat: name, Value: dismiss[s]})
		}
	}
	return list
}

// The changes the fight's end makes to a dynamic pet's stats before dependencies as each
// permanent aura expires (aura_helpers.go AttachStatsBuff), from a separate reset simulation.
// Each is read on a zeroed statsWithoutDeps, so it is the exact amount Go adds.
func petAuraFadeStats(request *proto.RaidSimRequest, index int) []PetAuraStats {
	simulation := core.NewSim(request, simsignals.CreateSignals())
	simulation.Reset()
	pet := simulation.Raid.Parties[0].Players[0].GetCharacter().PetAgents[index].GetPet()
	without := readPrivate(privateField(&pet.Unit, "statsWithoutDeps"))
	changes := []PetAuraStats{}
	for _, aura := range pet.GetAuras() {
		if !aura.IsActive() || aura.Duration != core.NeverExpires {
			continue
		}
		without.Set(reflect.ValueOf(stats.Stats{}))
		aura.Deactivate(simulation)
		change := without.Interface().(stats.Stats)
		values := map[string]float64{}
		for s := stats.Stat(0); s < stats.SimStatsLen; s++ {
			if change[s] != 0 {
				values[s.StatName()] = change[s]
			}
		}
		if len(values) != 0 {
			changes = append(changes, PetAuraStats{Aura: aura.Label, Stats: values})
		}
	}
	return changes
}

// The stats Disable logs: the pet's stats once its inheritance is removed, from a separate
// reset simulation. Pet.doneIteration expires the pet's auras first, undoing their stat buffs.
func petDismissStats(request *proto.RaidSimRequest, index int) stats.Stats {
	simulation := core.NewSim(request, simsignals.CreateSignals())
	simulation.Reset()
	pet := simulation.Raid.Parties[0].Players[0].GetCharacter().PetAgents[index].GetPet()
	for _, aura := range pet.GetAuras() {
		if aura.IsActive() {
			aura.Deactivate(simulation)
		}
	}
	pet.Disable(simulation)
	return pet.GetStats()
}
