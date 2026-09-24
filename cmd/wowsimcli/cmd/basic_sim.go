package cmd

import (
	"fmt"
	"log"
	"os"

	"github.com/spf13/cobra"
	"github.com/wowsims/forever/sim/core"
	"github.com/wowsims/forever/sim/core/proto"
	"google.golang.org/protobuf/encoding/protojson"
)

var (
	infile  string
	outfile string
	verbose bool
	strict  bool
)

var simCmd = &cobra.Command{
	Use:   "sim",
	Short: "simulate items & settings",
	Run:   simMain,
}

func init() {
	simCmd.Flags().StringVar(&infile, "infile", "input.json", "location of input file (RaidSimRequest in protojson format)")
	simCmd.Flags().StringVar(&outfile, "outfile", "", "location of output file, defaults to stdout")
	simCmd.Flags().BoolVar(&verbose, "verbose", false, "print information during runtime")
	simCmd.Flags().BoolVar(&strict, "strict", false, "reject input with unknown field names or enum value names instead of dropping them")
	simCmd.MarkFlagRequired("infile")
}

// loadRaidSimRequest parses a protojson RaidSimRequest. By default unknown field names and
// unknown enum value names are dropped, so a request written for a newer or older proto
// still loads. With strict set they are an error instead: a caller that builds requests
// programmatically wants a misspelt field or a race this build does not know to fail
// loudly rather than sim a different character than it asked for.
func loadRaidSimRequest(data []byte, strict bool) (*proto.RaidSimRequest, error) {
	input := &proto.RaidSimRequest{}
	if err := (protojson.UnmarshalOptions{DiscardUnknown: !strict}).Unmarshal(data, input); err != nil {
		return nil, err
	}
	return input, nil
}

func simMain(cmd *cobra.Command, args []string) {
	data, err := os.ReadFile(infile)
	if err != nil {
		log.Fatalf("failed to load input json file %q: %v", infile, err)
	}
	input, err := loadRaidSimRequest(data, strict)
	if err != nil {
		log.Fatalf("failed to load input json file: %s", err)
	}

	if input.SimOptions == nil {
		log.Fatalf("expected property 'simOptions' to be present in the input json file")
	}

	var output []byte
	reporter := make(chan *proto.ProgressMetrics, 10)
	core.RunRaidSimConcurrentAsync(input, reporter, "cmd-raid-sim")

	var finalResult *proto.RaidSimResult
	for v := range reporter {
		if v.FinalRaidResult != nil {
			finalResult = v.FinalRaidResult
			break
		}
		if verbose {
			fmt.Printf("Sim Progress: %d / %d\n", v.CompletedIterations, v.TotalIterations)
		}
	}

	output, err = protojson.MarshalOptions{EmitUnpopulated: true}.Marshal(finalResult)
	if err != nil {
		log.Fatalf("failed to marshal final results: %s", err)
	}

	if outfile == "" {
		fmt.Print(string(output))
	} else {
		err = os.WriteFile(outfile, output, 0666)
		if err != nil {
			log.Fatalf("failed to write output file:: %s", err)
		}
		if verbose {
			fmt.Printf("Wrote output file: `%s` successfully.\n", outfile)
		}
	}
}
