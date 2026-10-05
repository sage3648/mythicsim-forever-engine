// Write the application's request for each named reference build without queuing a job.
//
// usage: builds [-targets N] OUTPUT_DIRECTORY SLUG...
//
// Each request is built by the application's own ReferenceBuildBySlug and BuildRequest
// with the UI's defaults, 3000 iterations, seed 42 and the first-fight timeline, and is
// written as OUTPUT_DIRECTORY/SLUG.request.json. With -targets the fight has that many
// copies of the boss, as the application's Advanced setting gives, and the rotation the
// application's AoE lines; the request is written as SLUG-N-targets.request.json.
package main

import (
	"flag"
	"fmt"
	"os"
	"path/filepath"

	"github.com/mythicsim/mythicsim/worker/forever"
)

func must(err error) {
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}

func main() {
	targets := flag.Int("targets", 0, "copies of the boss, from 1 to 5; one when unset")
	flag.Parse()
	if flag.NArg() < 2 {
		must(fmt.Errorf("usage: builds [-targets N] OUTPUT_DIRECTORY SLUG..."))
	}
	output := flag.Arg(0)
	must(os.MkdirAll(output, 0o755))
	for _, slug := range flag.Args()[1:] {
		build, err := forever.ReferenceBuildBySlug(slug)
		must(err)
		built, err := forever.BuildRequest(&build.Export, forever.RequestOptions{
			Iterations: 3000, RandomSeed: 42, Timeline: true, TargetCount: *targets})
		must(err)
		for _, warning := range built.Warnings {
			fmt.Fprintf(os.Stderr, "%s: %s\n", slug, warning)
		}
		name := slug
		if *targets > 1 {
			name = fmt.Sprintf("%s-%d-targets", slug, *targets)
		}
		must(os.WriteFile(filepath.Join(output, name+".request.json"), append(built.JSON, '\n'), 0o644))
	}
}
