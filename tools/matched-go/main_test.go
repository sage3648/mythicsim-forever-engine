package main

import (
	"encoding/json"
	"math"
	"os"
	"path/filepath"
	"testing"
)

func TestPublishedSplitMixVector(t *testing.T) {
	rng := splitmix(0)
	for _, expected := range []uint64{0xe220a8397b1dcdaf, 0x6e789e6aa1b965f4, 0x06c45d188009454f} {
		if actual := rng.next(); actual != expected {
			t.Fatalf("got %x, want %x", actual, expected)
		}
	}
}

func TestAllPinnedEngineGoldens(t *testing.T) {
	directory := filepath.Join("..", "..", "fixtures")
	var manifest struct {
		Cases []struct {
			ID       string `json:"id"`
			Rust     string `json:"rust"`
			Expected Report `json:"expected"`
		} `json:"cases"`
	}
	data, err := os.ReadFile(filepath.Join(directory, "manifest.json"))
	if err != nil {
		t.Fatal(err)
	}
	if err = json.Unmarshal(data, &manifest); err != nil {
		t.Fatal(err)
	}
	if len(manifest.Cases) != 11 {
		t.Fatal("missing golden cases")
	}
	for _, c := range manifest.Cases {
		t.Run(c.ID, func(t *testing.T) {
			data, err := os.ReadFile(filepath.Join(directory, c.Rust))
			if err != nil {
				t.Fatal(err)
			}
			var request Request
			if err = json.Unmarshal(data, &request); err != nil {
				t.Fatal(err)
			}
			result, err := simulate(&request)
			if err != nil {
				t.Fatal(err)
			}
			if result.Counts != c.Expected.Counts || result.Seed != c.Expected.Seed || result.Iterations != c.Expected.Iterations {
				t.Fatal("counts or options differ from the real Go engine")
			}
			if math.Abs(result.Mean-c.Expected.Mean) > 1e-8 || math.Abs(result.Stdev-c.Expected.Stdev) > 1e-8 || math.Abs(result.ManaDelta-c.Expected.ManaDelta) > 1e-8 {
				t.Fatal("metrics differ from the real Go engine")
			}
			if result.Work.Completions != result.Counts.Casts || result.Work.HitRolls != result.Counts.Casts {
				t.Fatal("inconsistent work counters")
			}
		})
	}
}

func TestHeapOrdersEqualTimeEventsByPriorityAndInsertion(t *testing.T) {
	q := queue{events: make([]event, 0, 8)}
	q.push(event{at: 10, priority: -1, kind: impact})
	q.push(event{at: 10, priority: 0, kind: ready})
	q.push(event{at: 10, priority: 1, kind: manaTick})
	q.push(event{at: 10, priority: 0, kind: complete})
	for _, kind := range []uint8{manaTick, ready, complete, impact} {
		if q.pop().kind != kind {
			t.Fatal("incorrect event ordering")
		}
	}
}
