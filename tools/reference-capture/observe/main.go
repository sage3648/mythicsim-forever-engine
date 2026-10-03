// Run the pinned full Go engine on the captured application request.
package main

import (
	"encoding/json"
	"os"

	"github.com/wowsims/forever/sim"
	"github.com/wowsims/forever/sim/core"
	"github.com/wowsims/forever/sim/core/proto"
	"google.golang.org/protobuf/encoding/protojson"
)

func must(err error) {
	if err != nil {
		panic(err)
	}
}

func main() {
	if len(os.Args) != 3 {
		panic("usage: observe SNAPSHOT RESULT")
	}
	data, err := os.ReadFile(os.Args[1])
	must(err)
	var snapshot struct{ Request json.RawMessage }
	must(json.Unmarshal(data, &snapshot))
	request := &proto.RaidSimRequest{}
	must((protojson.UnmarshalOptions{DiscardUnknown: false}).Unmarshal(snapshot.Request, request))
	sim.RegisterAll()
	result := core.RunRaidSim(request)
	if result.Error != nil || result.IterationsDone != request.SimOptions.Iterations {
		panic("Go reference failed or completed fewer iterations than requested")
	}
	data, err = protojson.Marshal(result)
	must(err)
	must(os.WriteFile(os.Args[2], data, 0644))
}
