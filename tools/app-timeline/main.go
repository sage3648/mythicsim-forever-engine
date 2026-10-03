// Parse a Go result and a Rust prepared report with the application's own timeline
// helpers and compare the timelines. Build it in a scratch module that requires
// github.com/mythicsim/mythicsim/worker and replaces it with an application checkout:
//
//	go run . first|average GO_RESULT.json RUST_REPORT.json PLAYER_NAME
package main

import (
	"encoding/json"
	"fmt"
	"os"
	"reflect"

	"github.com/mythicsim/mythicsim/worker/forever"
)

func load(path string) (string, float64) {
	data, err := os.ReadFile(path)
	if err != nil {
		panic(err)
	}
	var wrapper map[string]json.RawMessage
	if err := json.Unmarshal(data, &wrapper); err != nil {
		panic(err)
	}
	if result, ok := wrapper["result"]; ok { // Rust prepared report
		data = result
	}
	_, logs, duration, err := forever.TakeLogs(data)
	if err != nil {
		panic(err)
	}
	return logs, duration
}

func main() {
	mode, goPath, rustPath, player := os.Args[1], os.Args[2], os.Args[3], os.Args[4]
	goLog, goDuration := load(goPath)
	rustLog, rustDuration := load(rustPath)
	var a, b any
	if mode == "first" {
		a, b = forever.ParseTimeline(goLog, player, goDuration), forever.ParseTimeline(rustLog, player, rustDuration)
	} else {
		a, b = forever.ParseAverageTimeline(goLog, player), forever.ParseAverageTimeline(rustLog, player)
	}
	ja, _ := json.Marshal(a)
	jb, _ := json.Marshal(b)
	fmt.Printf("%s timeline: %d bytes, identical=%v\n", mode, len(ja), reflect.DeepEqual(ja, jb))
}
