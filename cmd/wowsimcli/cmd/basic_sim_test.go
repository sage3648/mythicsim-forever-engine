package cmd

import (
	"strings"
	"testing"

	"github.com/wowsims/forever/sim/core/proto"
)

const knownRequest = `{
	"raid": {"parties": [{"players": [{"race": "RaceOrc", "class": "ClassWarrior"}]}]},
	"simOptions": {"iterations": 1}
}`

// A misspelt field name: "iteration" for "iterations".
const unknownFieldRequest = `{
	"raid": {"parties": [{"players": [{"race": "RaceOrc", "class": "ClassWarrior"}]}]},
	"simOptions": {"iteration": 1}
}`

// A race name this build does not have.
const unknownEnumRequest = `{
	"raid": {"parties": [{"players": [{"race": "RaceNotARace", "class": "ClassWarrior"}]}]},
	"simOptions": {"iterations": 1}
}`

func TestLoadRaidSimRequestAcceptsAKnownRequestEitherWay(t *testing.T) {
	for _, strict := range []bool{false, true} {
		input, err := loadRaidSimRequest([]byte(knownRequest), strict)
		if err != nil {
			t.Fatalf("strict=%v: %v", strict, err)
		}
		if got := input.Raid.Parties[0].Players[0].Race; got != proto.Race_RaceOrc {
			t.Errorf("strict=%v: race %v, want RaceOrc", strict, got)
		}
		if got := input.SimOptions.Iterations; got != 1 {
			t.Errorf("strict=%v: %d iterations, want 1", strict, got)
		}
	}
}

// Without --strict the CLI keeps upstream's behaviour: unknown names are dropped.
func TestLoadRaidSimRequestDropsUnknownNamesByDefault(t *testing.T) {
	input, err := loadRaidSimRequest([]byte(unknownFieldRequest), false)
	if err != nil {
		t.Fatalf("unknown field: %v", err)
	}
	if input.SimOptions.Iterations != 0 {
		t.Errorf("the misspelt field was read as iterations = %d", input.SimOptions.Iterations)
	}

	input, err = loadRaidSimRequest([]byte(unknownEnumRequest), false)
	if err != nil {
		t.Fatalf("unknown enum name: %v", err)
	}
	if got := input.Raid.Parties[0].Players[0].Race; got != proto.Race_RaceUnknown {
		t.Errorf("an unknown race name loaded as %v, want RaceUnknown", got)
	}
}

func TestLoadRaidSimRequestStrictRejectsUnknownNames(t *testing.T) {
	for name, tc := range map[string]struct {
		request string
		mention string
	}{
		"field": {unknownFieldRequest, "iteration"},
		"enum":  {unknownEnumRequest, "RaceNotARace"},
	} {
		_, err := loadRaidSimRequest([]byte(tc.request), true)
		if err == nil {
			t.Errorf("unknown %s: strict load succeeded", name)
			continue
		}
		if !strings.Contains(err.Error(), tc.mention) {
			t.Errorf("unknown %s: error %q does not name %q", name, err, tc.mention)
		}
	}
}
