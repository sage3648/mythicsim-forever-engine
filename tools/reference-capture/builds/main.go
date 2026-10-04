// Write the application's request for each named reference build without queuing a job.
//
// usage: builds OUTPUT_DIRECTORY SLUG...
//
// Each request is built by the application's own ReferenceBuildBySlug and BuildRequest
// with the UI's defaults, 3000 iterations, seed 42 and the first-fight timeline, and is
// written as OUTPUT_DIRECTORY/SLUG.request.json.
package main

import (
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
	if len(os.Args) < 3 {
		must(fmt.Errorf("usage: builds OUTPUT_DIRECTORY SLUG..."))
	}
	output := os.Args[1]
	must(os.MkdirAll(output, 0o755))
	for _, slug := range os.Args[2:] {
		build, err := forever.ReferenceBuildBySlug(slug)
		must(err)
		built, err := forever.BuildRequest(&build.Export, forever.RequestOptions{Iterations: 3000, RandomSeed: 42, Timeline: true})
		must(err)
		for _, warning := range built.Warnings {
			fmt.Fprintf(os.Stderr, "%s: %s\n", slug, warning)
		}
		must(os.WriteFile(filepath.Join(output, slug+".request.json"), append(built.JSON, '\n'), 0o644))
	}
}
