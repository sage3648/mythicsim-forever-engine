// The oracle runs the actual pinned Go engine, never a duplicate damage formula.
package main

import (
	"encoding/json"
	"flag"
	"fmt"
	"math"
	"os"
	"path/filepath"
	"time"

	"github.com/wowsims/forever/sim"
	"github.com/wowsims/forever/sim/core"
	"github.com/wowsims/forever/sim/core/proto"
	"github.com/wowsims/forever/sim/core/simsignals"
	"github.com/wowsims/forever/sim/core/spelldata"
	"github.com/wowsims/forever/sim/core/stats"
	"google.golang.org/protobuf/encoding/protojson"
)

// The reference pin, set at build time from upstream/sources.json with -ldflags -X.
var engineRevision string

func fail(err error) {
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}

func writeJSON(path string, value any) {
	data, err := json.MarshalIndent(value, "", "  ")
	fail(err)
	fail(os.WriteFile(path, append(data, '\n'), 0644))
}

func readRequest(path string) *proto.RaidSimRequest {
	data, err := os.ReadFile(path)
	fail(err)
	request := &proto.RaidSimRequest{}
	fail((protojson.UnmarshalOptions{DiscardUnknown: false}).Unmarshal(data, request))
	if request.SimOptions == nil || request.SimOptions.Iterations <= 0 {
		fail(fmt.Errorf("missing iterations"))
	}
	return request
}

type scenario struct {
	ID          string
	Duration    float64
	Talents     string
	Distance    float64
	TargetLevel int
	Resistance  float64
	Weapon      int
	BonusHit    float64
	BonusHaste  float64
}

func prepare(directory string, iterations int, seed int64) {
	fail(os.MkdirAll(directory, 0755))
	cases := []scenario{
		{"untalented-60", 60, "--", 20, 63, 0, 17103, 0, 0},
		{"static-frost-60", 60, "--053500033", 20, 63, 0, 17103, 0, 0},
		{"static-frost-120-oom", 120, "--053500033", 20, 63, 0, 17103, 0, 0},
		{"meditation-300-oom", 300, "0000000000003--053500033", 20, 63, 0, 17103, 0, 0},
		{"resistance-60", 60, "--053500033", 20, 63, 120, 17103, 0, 0},
		{"hit-cap-60", 60, "--053500033", 20, 63, 0, 17103, 224, 0},
		{"same-level-60", 60, "--053500033", 20, 60, 0, 17103, 0, 0},
		{"travel-boundary-3", 3, "--", 30, 63, 0, 17103, 0, 0},
		{"zero-travel-boundary-3", 3, "--", 0, 63, 0, 17103, 0, 0},
		{"staff-60", 60, "--053500033", 20, 63, 0, 18842, 0, 0},
		{"haste-60", 60, "--053500033", 20, 63, 0, 17103, 0, 100},
	}
	manifest := []map[string]any{}
	for _, c := range cases {
		// Fixed gear with no sets, weapon procs or active items in the rotation.
		ids := []int{0, 0, 0, 0, 14152, 0, 0, 0, 0, 0, 0, 0, 12930, 0, c.Weapon, 0, 0}
		items := make([]map[string]any, len(ids))
		for i, id := range ids {
			items[i] = map[string]any{"id": id}
		}
		bonus := make([]float64, stats.ProtoStatsLen)
		bonus[stats.SpellHitRating] = c.BonusHit
		bonus[stats.SpellHasteRating] = c.BonusHaste
		targetStats := make([]float64, stats.ProtoStatsLen)
		targetStats[stats.FrostResistance] = c.Resistance
		player := map[string]any{
			"name": "Rust prototype", "class": "ClassMage", "race": "RaceHuman",
			"disableRacials": true, "talentsString": c.Talents,
			"equipment":          map[string]any{"items": items},
			"bonusStats":         map[string]any{"stats": bonus},
			"distanceFromTarget": c.Distance, "reactionTimeMs": 100,
			"mage": map[string]any{"options": map[string]any{"classOptions": map[string]any{}}},
			"rotation": map[string]any{"type": "TypeAPL", "priorityList": []any{
				map[string]any{"action": map[string]any{"castSpell": map[string]any{"spellId": map[string]any{"spellId": 25304}}}},
			}},
		}
		raw := map[string]any{
			"raid": map[string]any{"parties": []any{map[string]any{"players": []any{player}}}},
			"encounter": map[string]any{"duration": c.Duration, "targets": []any{map[string]any{
				"level": c.TargetLevel, "mobType": "MobTypeHumanoid", "stats": targetStats,
			}}},
			"simOptions": map[string]any{"iterations": iterations, "randomSeed": fmt.Sprint(seed), "useLabeledRands": true},
		}
		goPath := filepath.Join(directory, c.ID+".go.json")
		rustPath := filepath.Join(directory, c.ID+".rust.json")
		writeJSON(goPath, raw)
		request := readRequest(goPath)
		simulation := core.NewSim(request, simsignals.CreateSignals())
		simulation.Reset()
		character := simulation.Raid.Parties[0].Players[0].GetCharacter()
		target := simulation.Encounter.ActiveTargetUnits[0]
		spell := character.GetSpell(core.ActionID{SpellID: 25304})
		if spell == nil {
			fail(fmt.Errorf("pinned engine has no Frostbolt 25304"))
		}
		table := character.AttackTables[target.UnitIndex]
		effect := spelldata.Find(25304).DamageEffect()
		if effect == nil {
			fail(fmt.Errorf("no Frostbolt damage effect"))
		}
		writeJSON(rustPath, map[string]any{
			"schema_version": 1, "source_revision": engineRevision, "scenario_id": c.ID,
			"iterations": iterations, "seed": seed, "duration_ns": simulation.BaseDuration.Nanoseconds(),
			"reaction_ns": character.ReactionTime.Nanoseconds(),
			"caster": map[string]any{
				"level": character.Level, "max_mana": character.MaxMana(), "spell_power": spell.SpellDamage(target),
				"hit_percent": spell.SpellHitChance(target) * 100, "crit_percent": spell.SpellCritChance(target) * 100,
				"spell_penetration":        character.GetStat(stats.SpellPiercing),
				"regen_casting_per_second": character.ManaRegenPerSecondWhileCasting(),
				"regen_idle_per_second":    character.ManaRegenPerSecondWhileNotCasting(),
			},
			"target": map[string]any{"level": target.Level, "frost_resistance": target.GetStat(stats.FrostResistance)},
			"spell": map[string]any{
				"id": 25304, "min_damage": effect.Min(60), "max_damage": effect.Max(60),
				"coefficient":       spell.BonusCoefficient,
				"damage_multiplier": spell.AttackerDamageMultiplier(table, false) * spell.TargetDamageMultiplier(simulation, table, false),
				"crit_multiplier":   spell.CritDamageMultiplier(table), "mana_cost": spell.Cost.GetCurrentCost(),
				"cast_ns":   spell.CastTime().Round(time.Millisecond).Nanoseconds(),
				"gcd_ns":    max(time.Second, character.ApplyCastSpeed(spell.DefaultCast.GCD).Round(time.Millisecond)).Nanoseconds(),
				"travel_ns": spell.TravelTime().Nanoseconds(),
			},
		})
		manifest = append(manifest, map[string]any{"id": c.ID, "go": filepath.Base(goPath), "rust": filepath.Base(rustPath), "equipment_ids": ids, "talents_string": c.Talents})
	}
	writeJSON(filepath.Join(directory, "manifest.json"), map[string]any{"source_revision": engineRevision, "cases": manifest})
}

func run(request *proto.RaidSimRequest, output string) {
	started := time.Now()
	result := core.RunRaidSim(request)
	elapsed := time.Since(started).Nanoseconds()
	if result.Error != nil && result.Error.Message != "" {
		fail(fmt.Errorf("Go engine: %s", result.Error.Message))
	}
	player := result.RaidMetrics.Parties[0].Players[0]
	manaDelta := 0.0
	for _, resource := range player.Resources {
		if resource.Type == proto.ResourceType_ResourceTypeMana {
			manaDelta += resource.ActualGain
		}
	}
	counts := map[string]int64{"casts": 0, "hits": 0, "crits": 0, "misses": 0}
	for _, action := range player.Actions {
		if action.Id.GetSpellId() != 25304 {
			continue
		}
		for _, target := range action.Targets {
			counts["casts"] += int64(target.Casts)
			counts["hits"] += int64(target.Hits)
			counts["crits"] += int64(target.Crits)
			counts["misses"] += int64(target.Misses)
		}
	}
	writeJSON(output, map[string]any{
		"engine": "forever-go-oracle", "source_revision": engineRevision,
		"iterations": request.SimOptions.Iterations, "seed": request.SimOptions.RandomSeed,
		"dps_mean": player.Dps.Avg, "dps_stdev": player.Dps.Stdev,
		"dps_standard_error": player.Dps.Stdev / math.Sqrt(float64(request.SimOptions.Iterations)),
		"mana_delta_mean":    manaDelta / float64(request.SimOptions.Iterations),
		"counts":             counts, "elapsed_ns": elapsed, "logs": result.Logs,
	})
}

func main() {
	if engineRevision == "" {
		fail(fmt.Errorf("build with the pin from upstream/sources.json (tools/compare.py does this)"))
	}
	sim.RegisterAll()
	if len(os.Args) < 2 {
		fail(fmt.Errorf("expected prepare or sim"))
	}
	flags := flag.NewFlagSet(os.Args[1], flag.ExitOnError)
	infile := flags.String("infile", "", "Go request")
	outfile := flags.String("outfile", "", "result file or fixture directory")
	iterations := flags.Int("iterations", 3000, "fixture iterations")
	seed := flags.Int64("seed", 42, "fixture seed")
	trace := flags.Bool("trace", false, "log first iteration")
	fail(flags.Parse(os.Args[2:]))
	if *outfile == "" {
		fail(fmt.Errorf("--outfile is required"))
	}
	switch os.Args[1] {
	case "prepare":
		if *iterations <= 0 || *iterations > 1000000 || *seed <= 0 {
			fail(fmt.Errorf("invalid iterations or seed"))
		}
		prepare(*outfile, *iterations, *seed)
	case "sim":
		request := readRequest(*infile)
		request.SimOptions.DebugFirstIteration = *trace
		run(request, *outfile)
	default:
		fail(fmt.Errorf("unsupported command %s", os.Args[1]))
	}
}
