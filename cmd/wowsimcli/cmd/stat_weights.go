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
	statWeightsInfile  string
	statWeightsOutfile string
	statWeightsVerbose bool
	statWeightsStrict  bool
)

var statWeightsCmd = &cobra.Command{
	Use:   "statweights",
	Short: "compute stat weights and EP values",
	Run:   statWeightsMain,
}

func init() {
	statWeightsCmd.Flags().StringVar(&statWeightsInfile, "infile", "input.json", "location of input file (StatWeightsRequest in protojson format)")
	statWeightsCmd.Flags().StringVar(&statWeightsOutfile, "outfile", "", "location of output file, defaults to stdout")
	statWeightsCmd.Flags().BoolVar(&statWeightsVerbose, "verbose", false, "print information during runtime")
	statWeightsCmd.Flags().BoolVar(&statWeightsStrict, "strict", false, "reject input with unknown field names or enum value names instead of dropping them")
	statWeightsCmd.MarkFlagRequired("infile")
}

// loadStatWeightsRequest parses a protojson StatWeightsRequest, with the same strict
// behaviour as loadRaidSimRequest.
func loadStatWeightsRequest(data []byte, strict bool) (*proto.StatWeightsRequest, error) {
	input := &proto.StatWeightsRequest{}
	if err := (protojson.UnmarshalOptions{DiscardUnknown: !strict}).Unmarshal(data, input); err != nil {
		return nil, err
	}
	return input, nil
}

func statWeightsMain(cmd *cobra.Command, args []string) {
	data, err := os.ReadFile(statWeightsInfile)
	if err != nil {
		log.Fatalf("failed to load input json file %q: %v", statWeightsInfile, err)
	}
	input, err := loadStatWeightsRequest(data, statWeightsStrict)
	if err != nil {
		log.Fatalf("failed to load input json file: %s", err)
	}

	if input.SimOptions == nil {
		log.Fatalf("expected property 'simOptions' to be present in the input json file")
	}
	if len(input.StatsToWeigh) == 0 && len(input.PseudoStatsToWeigh) == 0 {
		log.Fatalf("expected property 'statsToWeigh' or 'pseudoStatsToWeigh' to list at least one stat")
	}

	reporter := make(chan *proto.ProgressMetrics, 100)
	core.StatWeightsAsync(input, reporter, "cmd-stat-weights")

	var finalResult *proto.StatWeightsResult
	for v := range reporter {
		if v.FinalWeightResult != nil {
			finalResult = v.FinalWeightResult
			break
		}
		if statWeightsVerbose {
			fmt.Printf("Stat weights progress: sim %d / %d, %d / %d iterations\n", v.CompletedSims, v.TotalSims, v.CompletedIterations, v.TotalIterations)
		}
	}

	output, err := protojson.MarshalOptions{EmitUnpopulated: true}.Marshal(finalResult)
	if err != nil {
		log.Fatalf("failed to marshal final results: %s", err)
	}

	if statWeightsOutfile == "" {
		fmt.Print(string(output))
	} else {
		if err := os.WriteFile(statWeightsOutfile, output, 0666); err != nil {
			log.Fatalf("failed to write output file: %s", err)
		}
		if statWeightsVerbose {
			fmt.Printf("Wrote output file: `%s` successfully.\n", statWeightsOutfile)
		}
	}
}
