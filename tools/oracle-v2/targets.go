package main

// Fights against several identical copies of the boss (the application's Advanced "targets"
// setting). Go creates every copy as an enemy unit, registers the same auras, dots, debuffs
// and damage taken modifiers on each, and numbers the raid's units after them. Rust keeps one
// set of state per target and gives each copy the first target's auras at the same positions,
// which holds while every copy exports exactly as the first. Which spells and effects reach
// the other targets is the Rust coverage gate's decision.

import (
	"encoding/json"
	"fmt"
	"slices"
	"sort"
	"strings"

	"github.com/wowsims/forever/sim/core"
	"github.com/wowsims/forever/sim/core/proto"
	googleProto "google.golang.org/protobuf/proto"
)

// The most targets the application offers.
const maxTargets = 5

// The number of targets when the fight has more than one, zero otherwise.
func targetCount(request *proto.RaidSimRequest) int32 {
	if count := len(request.Encounter.GetTargets()); count > 1 {
		return int32(count)
	}
	return 0
}

// The refusals that need only the request: the supported count, identical copies, every
// target active from the start and no tank.
func targetNotes(request *proto.RaidSimRequest, note func(bool, string)) {
	targets := request.Encounter.GetTargets()
	note(len(targets) == 0, "a fight without targets is unsupported")
	note(len(targets) > maxTargets, fmt.Sprintf("%d targets: at most %d are supported", len(targets), maxTargets))
	for i, target := range targets {
		note(target.GetDisabledAtStart(), fmt.Sprintf("target %d starts disabled: every target must be active", i+1))
		if i > 0 {
			note(!googleProto.Equal(target, targets[0]), fmt.Sprintf("target %d differs from the first target: only identical copies are supported", i+1))
		}
	}
	note(len(targets) > 1 && len(request.Raid.GetTanks()) != 0,
		"a tank assignment with several targets: every copy would swing at the tank")
}

// What Rust reads from a target, apart from its index and label, part by part, for comparing
// the copies.
func targetShape(target *core.Unit, character *core.Character) map[string]string {
	timers := &timerNames{names: map[*core.Timer]string{}}
	table := character.AttackTables[target.UnitIndex]
	extra := 0
	for _, handler := range table.DamageDoneByCasterExtraMultiplier {
		if handler != nil {
			extra++
		}
	}
	encode := func(value any) string {
		encoded, err := json.Marshal(value)
		if err != nil {
			fail(err)
		}
		return string(encoded)
	}
	shape := map[string]string{
		"level and mob type":     encode([]any{target.Level, target.MobType.String()}),
		"stats":                  encode(statValues(target.GetStats())),
		"pseudo stats":           encode(exportPseudo(target.PseudoStats)),
		"auto attacks":           encode([]bool{target.AutoAttacks.AutoSwingMelee, target.AutoAttacks.AutoSwingRanged}),
		"actions":                encode(metricsActions(target)),
		"damage taken modifiers": encode(len(target.DynamicDamageTakenModifiers)),
		"the player's attack table": encode([]any{table.BaseMissChance, table.BaseSpellMissChance, table.BaseBlockChance,
			table.BaseDodgeChance, table.BaseParryChance, table.BaseGlanceChance, table.BaseCrushChance, table.GlanceMultiplier,
			table.GlanceSpread, table.MeleeCritSuppression, table.SpellCritSuppression, table.HitSuppression, table.CritMultiplier,
			table.DamageDealtMultiplier, table.DamageTakenMultiplier, table.HealingDealtMultiplier, table.IgnoreArmor,
			table.ArmorIgnoreFactor, table.BonusSpellCritPercent, table.RangedDamageTakenMultiplier,
			table.DamageDoneByCasterMultiplier != nil, extra, table.MobTypeBonusStats}),
	}
	// A label may name its target, as a dot's "Vampiric Embrace - Target 1" does.
	for i, aura := range exportAuras(target, timers) {
		aura.Label = strings.ReplaceAll(aura.Label, target.Label, "<target>")
		aura.Callbacks = withoutStackObserver(aura)
		shape[fmt.Sprintf("aura %d (%s)", i+1, aura.Label)] = encode(aura)
	}
	shape["aura count"] = encode(len(target.GetAuras()))
	return shape
}

// The refusals that need the reset simulation: every target past the first must export as the
// first does, and no target aura may have an internal cooldown, which Go keeps per copy.
func targetCopyNotes(simulation *core.Simulation, character *core.Character, note func(bool, string)) {
	targets := simulation.Encounter.AllTargetUnits
	if len(targets) < 2 {
		return
	}
	first := targetShape(targets[0], character)
	for i, target := range targets[1:] {
		shape := targetShape(target, character)
		differs := []string{}
		for part, value := range shape {
			if first[part] != value {
				differs = append(differs, part)
			}
		}
		for part := range first {
			if _, ok := shape[part]; !ok {
				differs = append(differs, part)
			}
		}
		sort.Strings(differs)
		note(len(differs) > 0, fmt.Sprintf("target %d is set up unlike the first target in %s: only identical copies are supported",
			i+2, strings.Join(differs, ", ")))
	}
	for _, aura := range targets[0].GetAuras() {
		note(aura.Icd != nil, fmt.Sprintf("target aura %s has an internal cooldown, which each of several targets keeps apart", aura.Label))
	}
}

// apl_values_aura.go newValueAuraNumStacks reads a stacking aura's stacks through a reset and a
// stack change callback it adds to the aura on the first target alone, which a rotation
// condition such as Lacerate's stacks leaves there. The observer only records the stacks for
// that condition, which Rust reads from the first target as well, so the pair does not make
// the other copies differ. An aura holding just one of the two keeps it.
func withoutStackObserver(aura Aura) []string {
	if aura.MaxStacks == 0 || !slices.Contains(aura.Callbacks, "on_reset") || !slices.Contains(aura.Callbacks, "on_stacks_change") {
		return aura.Callbacks
	}
	kept := []string{}
	for _, callback := range aura.Callbacks {
		if callback != "on_reset" && callback != "on_stacks_change" {
			kept = append(kept, callback)
		}
	}
	return kept
}
