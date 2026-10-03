// Capture the application's public synthetic Frost reference without queuing a job.
package main

import (
	"encoding/json"
	"os"

	"github.com/mythicsim/mythicsim/worker/forever"
)

// The engine revision the report names, set by tools/inventory.py with -ldflags -X from
// the inventory manifest.
var engineRevision string

func must(err error) {
	if err != nil {
		panic(err)
	}
}

func main() {
	if engineRevision == "" {
		panic("build with the engine revision from the inventory manifest (tools/inventory.py does this)")
	}
	build, err := forever.ReferenceBuildBySlug("frost-mage")
	must(err)
	built, err := forever.BuildRequest(&build.Export, forever.RequestOptions{Iterations: 3000, RandomSeed: 42, Timeline: true})
	must(err)
	var value any = struct {
		Export   forever.CharacterExport `json:"export"`
		Request  json.RawMessage         `json:"request"`
		Rotation forever.BuiltRotation   `json:"rotation_info"`
		Warnings []string                `json:"warnings"`
	}{build.Export, built.JSON, built.Rotation, built.Warnings}
	if len(os.Args) == 3 && os.Args[1] == "--result" {
		data, err := os.ReadFile(os.Args[2])
		must(err)
		summary, err := forever.ParseResult(data)
		must(err)
		stripped, log, duration, err := forever.TakeLogs(data)
		must(err)
		report := forever.NewReport(&build.Export, built, summary, engineRevision, stripped)
		report.Timeline = forever.ParseTimeline(log, build.Export.Name, duration)
		report.CombatLog = log != ""
		value = report
	} else if len(os.Args) != 1 {
		panic("usage: request [--result PATH]")
	}
	encoder := json.NewEncoder(os.Stdout)
	encoder.SetIndent("", "  ")
	must(encoder.Encode(value))
}
