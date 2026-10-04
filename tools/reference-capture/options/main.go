// Write the application's request for each named reference build under each option the
// application lets a person change, one option at a time, without queuing a job.
//
// usage: options OUTPUT_DIRECTORY SLUG...
//
// Each build is the application's own ReferenceBuildBySlug, built by its BuildRequest with
// the UI's defaults, 3000 iterations, seed 42 and the first-fight timeline, as builds/main.go
// does, once as it is and once for each value of each choice: the Advanced settings (potion,
// warlock demon and Demonic Pact sacrifice, Enhancement imbue, fire and air totems, preset
// rotation, starting distance, fight length and its variation), the Protection Paladin seal,
// and the builder's simulation spec, Feral form and Hunter style. A choice the spec does not
// take is refused by BuildRequest and reported on standard error. Each request is written as
// OUTPUT_DIRECTORY/SLUG--OPTION-VALUE.request.json.
//
// Like builds/main.go, it runs from a scratch module whose go.mod replaces
// github.com/mythicsim/mythicsim/worker with the application's worker directory.
package main

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/mythicsim/mythicsim/worker/forever"
)

func must(err error) {
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}

type variant struct {
	name     string
	advanced *forever.Advanced
	seal     string
	export   func(*forever.CharacterExport)
}

func distance(yards float64) *float64 { return &yards }

func variants(build *forever.ReferenceBuild) []variant {
	out := []variant{{name: "base"}}
	add := func(name string, advanced forever.Advanced) {
		a := advanced
		out = append(out, variant{name: name, advanced: &a})
	}
	for _, potion := range []string{"MajorMana", "MightyRage", "GreaterStoneshield", "None"} {
		add("potion-"+potion, forever.Advanced{Potion: potion})
	}
	for _, demon := range []string{"Imp", "Succubus", "Voidwalker", "Felhunter"} {
		add("demon-"+demon, forever.Advanced{WarlockDemon: demon})
	}
	for _, pact := range []string{"Imp", "Succubus", "Voidwalker", "None"} {
		add("pact-"+pact, forever.Advanced{WarlockPactSacrifice: pact})
	}
	for _, imbue := range []string{"WindfuryWeapon", "FlametongueWeapon", "FrostbrandWeapon", "RockbiterWeapon", "NoImbue"} {
		add("imbue-"+imbue, forever.Advanced{ShamanImbue: imbue})
	}
	for _, totem := range []string{"SearingTotem", "MagmaTotem", "FlametongueTotem", "None"} {
		add("fire-"+totem, forever.Advanced{FireTotem: totem})
	}
	for _, totem := range []string{"WindfuryTotem", "GraceOfAirTotem", "None"} {
		add("air-"+totem, forever.Advanced{AirTotem: totem})
	}
	for _, yards := range []float64{0, 5, 10, 20, 30, 40} {
		add(fmt.Sprintf("distance-%g", yards), forever.Advanced{StartingDistance: distance(yards)})
	}
	for _, seconds := range []float64{30, 60, 180, 300, 600} {
		add(fmt.Sprintf("duration-%g", seconds), forever.Advanced{Duration: seconds})
	}
	for _, seconds := range []float64{0, 5, 30, 60} {
		add(fmt.Sprintf("variation-%g", seconds), forever.Advanced{Duration: 120, DurationVariation: seconds})
	}
	for _, seal := range []string{"Righteousness", "Fury"} {
		out = append(out, variant{name: "seal-" + seal, seal: seal})
	}
	export := build.Export
	if spec, err := forever.SelectExportSpec(&export); err == nil {
		if choices, err := forever.RotationChoices(spec); err == nil {
			for _, choice := range choices {
				add("rotation-"+strings.ReplaceAll(choice.ID, "/", "-"), forever.Advanced{Rotation: choice.ID})
			}
		}
	}
	for _, key := range forever.SimulationSpecChoices(build.Export.Class) {
		key := key
		out = append(out, variant{name: "simspec-" + key, export: func(ex *forever.CharacterExport) {
			ex.MythicSim.SimulationSpec = key
		}})
	}
	for _, form := range []string{"cat", "bear"} {
		form := form
		out = append(out, variant{name: "druidform-" + form, export: func(ex *forever.CharacterExport) {
			ex.MythicSim.DruidForm = form
		}})
	}
	for _, style := range []string{"melee", "ranged"} {
		style := style
		out = append(out, variant{name: "hunterstyle-" + style, export: func(ex *forever.CharacterExport) {
			ex.MythicSim.HunterStyle = style
		}})
	}
	return out
}

func main() {
	if len(os.Args) < 3 {
		must(fmt.Errorf("usage: options OUTPUT_DIRECTORY SLUG..."))
	}
	output := os.Args[1]
	must(os.MkdirAll(output, 0o755))
	for _, slug := range os.Args[2:] {
		build, err := forever.ReferenceBuildBySlug(slug)
		must(err)
		for _, v := range variants(build) {
			export := build.Export
			if v.export != nil {
				v.export(&export)
			}
			built, err := forever.BuildRequest(&export, forever.RequestOptions{Iterations: 3000, RandomSeed: 42,
				Timeline: true, Advanced: v.advanced, PrimarySeal: v.seal})
			name := slug + "--" + v.name
			if err != nil {
				fmt.Fprintf(os.Stderr, "%s: refused: %v\n", name, err)
				continue
			}
			must(os.WriteFile(filepath.Join(output, name+".request.json"), append(built.JSON, '\n'), 0o644))
		}
	}
}
