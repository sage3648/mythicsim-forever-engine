// Write the application's request for each builder starter in each of its races, as Build a
// character exports it, without queuing a job.
//
// usage: starters STARTERS_JSON TREES_DIRECTORY OUTPUT_DIRECTORY
//
// STARTERS_JSON is the application's web/src/lib/forever-builder-starters.json and
// TREES_DIRECTORY its web/src/lib/forever-talent-trees. Each starter becomes the export
// foreverBuildExport writes for a fresh build (web/src/lib/forever-builder.ts): the starter's
// gear, professions and race, talents by client spell identity from the class's trees, the
// druid form while the build is Feral, the hunter style and the simulation spec. The export is
// read with the worker's own ParseExport and built by its BuildRequest with the UI's defaults,
// 3000 iterations, seed 42 and the first-fight timeline, and is written as
// OUTPUT_DIRECTORY/SLUG--RACE.request.json, with the race lower case and hyphenated.
//
// Like builds/main.go, it runs from a scratch module whose go.mod replaces
// github.com/mythicsim/mythicsim/worker with the application's worker directory.
package main

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"regexp"
	"strings"

	"github.com/mythicsim/mythicsim/worker/forever"
)

type starterItem struct {
	ID           int `json:"id"`
	Enchant      int `json:"enchant,omitempty"`
	RandomSuffix int `json:"randomSuffix,omitempty"`
}

type starter struct {
	Slug           string               `json:"slug"`
	Class          string               `json:"class"`
	Race           string               `json:"race"`
	Races          []string             `json:"races"`
	Talents        string               `json:"talents"`
	Professions    []forever.Profession `json:"professions"`
	Gear           []*starterItem       `json:"gear"`
	DruidForm      string               `json:"druidForm"`
	HunterStyle    string               `json:"hunterStyle"`
	SimulationSpec string               `json:"simulationSpec"`
}

type starterData struct {
	ClientBuild string    `json:"clientBuild"`
	Starters    []starter `json:"starters"`
}

type talentTrees struct {
	ClientBuild string `json:"clientBuild"`
	Trees       []struct {
		Nodes []struct {
			MaxRank     int `json:"maxRank"`
			ClientSpell int `json:"clientSpell"`
		} `json:"nodes"`
	} `json:"trees"`
}

func must(err error) {
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}

func read(path string, into any) {
	data, err := os.ReadFile(path)
	must(err)
	must(json.Unmarshal(data, into))
}

// forever-talent-rules.ts decodeRanks then talentSpells: per tree, [client spell, rank] for
// every node in order.
func talentSpells(trees *talentTrees, talents string) ([][][]int, error) {
	parts := strings.Split(strings.TrimSpace(talents), "-")
	if len(parts) > len(trees.Trees) {
		return nil, fmt.Errorf("talents %q have more trees than the class", talents)
	}
	out := make([][][]int, len(trees.Trees))
	for t, tree := range trees.Trees {
		part := ""
		if t < len(parts) {
			part = parts[t]
		}
		if len(part) > len(tree.Nodes) {
			return nil, fmt.Errorf("talents %q overflow tree %d", talents, t)
		}
		out[t] = make([][]int, len(tree.Nodes))
		for n, node := range tree.Nodes {
			rank := 0
			if n < len(part) {
				rank = int(part[n] - '0')
			}
			if rank < 0 || rank > node.MaxRank {
				return nil, fmt.Errorf("talents %q exceed a rank in tree %d", talents, t)
			}
			out[t][n] = []int{node.ClientSpell, rank}
		}
	}
	return out, nil
}

var nonWord = regexp.MustCompile(`[^a-z0-9]+`)

func main() {
	if len(os.Args) != 4 {
		must(fmt.Errorf("usage: starters STARTERS_JSON TREES_DIRECTORY OUTPUT_DIRECTORY"))
	}
	var data starterData
	read(os.Args[1], &data)
	output := os.Args[3]
	must(os.MkdirAll(output, 0o755))
	for _, s := range data.Starters {
		var trees talentTrees
		read(filepath.Join(os.Args[2], s.Class+".json"), &trees)
		spells, err := talentSpells(&trees, s.Talents)
		must(err)
		mythicsim := map[string]any{
			"builder": s.Slug, "talentSource": "traits", "clientBuild": trees.ClientBuild, "talentSpells": spells,
		}
		if s.SimulationSpec != "" {
			mythicsim["simulationSpec"] = s.SimulationSpec
		}
		// withForeverTalents keeps the druid form only while the talents are Feral, which only
		// the Feral starters name; a Hunter always says whether it fights in melee.
		if s.Class == "druid" && s.DruidForm != "" {
			mythicsim["druidForm"] = s.DruidForm
		}
		if s.Class == "hunter" {
			style := s.HunterStyle
			if style == "" {
				style = "ranged"
			}
			mythicsim["hunterStyle"] = style
		}
		for _, race := range s.Races {
			export := map[string]any{
				"version": "1.0", "name": s.Slug, "realm": "", "race": race, "class": s.Class, "level": 60,
				"talents": s.Talents, "professions": s.Professions, "gear": map[string]any{"items": s.Gear},
				"mythicsim": mythicsim,
			}
			paste, err := json.Marshal(export)
			must(err)
			parsed, err := forever.ParseExport(paste)
			must(err)
			built, err := forever.BuildRequest(parsed, forever.RequestOptions{Iterations: 3000, RandomSeed: 42, Timeline: true})
			name := s.Slug + "--" + strings.Trim(nonWord.ReplaceAllString(strings.ToLower(race), "-"), "-")
			if err != nil {
				fmt.Fprintf(os.Stderr, "%s: %v\n", name, err)
				continue
			}
			for _, warning := range built.Warnings {
				fmt.Fprintf(os.Stderr, "%s: %s\n", name, warning)
			}
			must(os.WriteFile(filepath.Join(output, name+".request.json"), append(built.JSON, '\n'), 0o644))
		}
	}
}
