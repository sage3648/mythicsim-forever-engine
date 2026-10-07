// Matched benchmark kernel. The full pinned Go engine remains the accuracy oracle.
// Combat semantics and SplitMix64 derive from the MIT wowsims engine; see ../../LICENSE.
package main

import (
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"io"
	"math"
	"os"
	"runtime"
	"strconv"
	"time"
)

const revision = "2d93e423e0303e93dbb16e190d435503248b68f9"
const second uint64 = 1000000000

type Caster struct {
	Level        uint32  `json:"level"`
	MaxMana      float64 `json:"max_mana"`
	SpellPower   float64 `json:"spell_power"`
	HitPercent   float64 `json:"hit_percent"`
	CritPercent  float64 `json:"crit_percent"`
	Penetration  float64 `json:"spell_penetration"`
	RegenCasting float64 `json:"regen_casting_per_second"`
	RegenIdle    float64 `json:"regen_idle_per_second"`
}
type Target struct {
	Level      uint32  `json:"level"`
	Resistance float64 `json:"frost_resistance"`
}
type Spell struct {
	ID               uint32  `json:"id"`
	Min              float64 `json:"min_damage"`
	Max              float64 `json:"max_damage"`
	Coefficient      float64 `json:"coefficient"`
	DamageMultiplier float64 `json:"damage_multiplier"`
	CritMultiplier   float64 `json:"crit_multiplier"`
	ManaCost         float64 `json:"mana_cost"`
	Cast             uint64  `json:"cast_ns"`
	GCD              uint64  `json:"gcd_ns"`
	Travel           uint64  `json:"travel_ns"`
}
type Request struct {
	Schema     uint32 `json:"schema_version"`
	Revision   string `json:"source_revision"`
	ID         string `json:"scenario_id"`
	Iterations uint32 `json:"iterations"`
	Seed       uint64 `json:"seed"`
	Duration   uint64 `json:"duration_ns"`
	Reaction   uint64 `json:"reaction_ns"`
	Caster     Caster `json:"caster"`
	Target     Target `json:"target"`
	Spell      Spell  `json:"spell"`
}

func (r *Request) validate() error {
	if r.Schema != 1 || r.Revision != revision || r.Spell.ID != 25304 || r.Caster.Level != 60 || r.Target.Level < 60 || r.Target.Level > 63 {
		return errors.New("unsupported prepared input")
	}
	if len(r.ID) == 0 || len(r.ID) > 200 || r.Iterations == 0 || r.Iterations > 1000000 || r.Seed == 0 || r.Seed > math.MaxInt64-uint64(r.Iterations) || r.Duration < second || r.Duration > 600*second || r.Reaction < 10000000 || r.Reaction > second || r.Spell.Cast < second || r.Spell.Cast > 10*second || r.Spell.GCD < second || r.Spell.GCD > 10*second || r.Spell.Travel > 10*second {
		return errors.New("parameters outside prototype limits")
	}
	values := []float64{r.Caster.MaxMana, r.Caster.SpellPower, r.Caster.HitPercent, r.Caster.CritPercent, r.Caster.Penetration, r.Caster.RegenCasting, r.Caster.RegenIdle, r.Target.Resistance, r.Spell.Min, r.Spell.Max, r.Spell.Coefficient, r.Spell.DamageMultiplier, r.Spell.CritMultiplier, r.Spell.ManaCost}
	for _, v := range values {
		if math.IsNaN(v) || math.IsInf(v, 0) || v < 0 || v > 1000000 {
			return errors.New("invalid numeric parameter")
		}
	}
	if r.Caster.MaxMana == 0 || r.Spell.ManaCost > r.Caster.MaxMana || r.Caster.HitPercent > 100 || r.Caster.CritPercent > 100 || r.Spell.Min > r.Spell.Max || r.Spell.CritMultiplier < 1 || r.Spell.DamageMultiplier == 0 {
		return errors.New("inconsistent prepared input")
	}
	return nil
}

type Counts struct {
	Casts  uint64 `json:"casts"`
	Hits   uint64 `json:"hits"`
	Crits  uint64 `json:"crits"`
	Misses uint64 `json:"misses"`
}
type Work struct {
	ManaTicks   uint64 `json:"mana_ticks"`
	ReadyChecks uint64 `json:"ready_checks"`
	Completions uint64 `json:"cast_completions"`
	Impacts     uint64 `json:"impacts"`
	DamageRolls uint64 `json:"damage_rolls"`
	HitRolls    uint64 `json:"hit_rolls"`
	CritRolls   uint64 `json:"crit_rolls"`
}
type Report struct {
	Engine     string     `json:"engine"`
	Revision   string     `json:"source_revision"`
	ID         string     `json:"scenario_id"`
	Iterations uint32     `json:"iterations"`
	Seed       uint64     `json:"seed"`
	Mean       float64    `json:"dps_mean"`
	Stdev      float64    `json:"dps_stdev"`
	Error      float64    `json:"dps_standard_error"`
	Counts     Counts     `json:"counts"`
	Work       Work       `json:"work"`
	ManaEnd    float64    `json:"mana_end_mean"`
	ManaDelta  float64    `json:"mana_delta_mean"`
	Elapsed    int64      `json:"elapsed_ns"`
	Trace      []struct{} `json:"trace"`
}

type splitmix uint64

func (s *splitmix) next() uint64 {
	*s += 0x9e3779b97f4a7c15
	z := uint64(*s)
	z = (z ^ (z >> 30)) * 0xbf58476d1ce4e5b9
	z = (z ^ (z >> 27)) * 0x94d049bb133111eb
	return z ^ (z >> 31)
}
func (s *splitmix) float() float64 { return float64(s.next()>>11) * (1.0 / float64(uint64(1)<<53)) }
func labeled(seed uint64, label string) splitmix {
	text := label + strconv.FormatUint(seed, 16)
	h := uint32(2166136261)
	for i := 0; i < len(text); i++ {
		h = (h ^ uint32(text[i])) * 16777619
	}
	return splitmix(h)
}

const (
	manaTick uint8 = iota
	ready
	complete
	impact
)
const (
	hit uint8 = iota
	crit
	miss
)

type event struct {
	at       uint64
	sequence uint64
	priority int8
	kind     uint8
	outcome  uint8
	damage   float64
}

// A typed binary heap avoids interface boxing and per-event allocations.
// It has the same ordering and reused capacity as Rust's BinaryHeap.
type queue struct {
	events   []event
	sequence uint64
}

func before(a, b event) bool {
	if a.at != b.at {
		return a.at < b.at
	}
	if a.priority != b.priority {
		return a.priority > b.priority
	}
	return a.sequence < b.sequence
}
func (q *queue) push(e event) {
	e.sequence = q.sequence
	q.sequence++
	q.events = append(q.events, e)
	i := len(q.events) - 1
	for i > 0 {
		p := (i - 1) / 2
		if !before(q.events[i], q.events[p]) {
			break
		}
		q.events[i], q.events[p] = q.events[p], q.events[i]
		i = p
	}
}
func (q *queue) pop() event {
	result := q.events[0]
	last := len(q.events) - 1
	q.events[0] = q.events[last]
	q.events = q.events[:last]
	for i := 0; ; {
		child := 2*i + 1
		if child >= last {
			break
		}
		if child+1 < last && before(q.events[child+1], q.events[child]) {
			child++
		}
		if !before(q.events[child], q.events[i]) {
			break
		}
		q.events[i], q.events[child] = q.events[child], q.events[i]
		i = child
	}
	return result
}

func simulate(r *Request) (Report, error) {
	if err := r.validate(); err != nil {
		return Report{}, err
	}
	start := time.Now()
	mean, m2, manaMean := 0.0, 0.0, 0.0
	var counts Counts
	var work Work
	q := queue{events: make([]event, 0, 8)}
	baseMiss := 0.17
	switch r.Target.Level {
	case 60:
		baseMiss = 0.04
	case 61:
		baseMiss = 0.05
	case 62:
		baseMiss = 0.06
	}
	resistance := max(0, r.Target.Resistance-r.Caster.Penetration)
	hitChance := min(0.99, (1-baseMiss)*(1-0.75*min(1, resistance/(5*float64(r.Caster.Level))))+r.Caster.HitPercent/100)
	for iteration := uint32(0); iteration < r.Iterations; iteration++ {
		seed := r.Seed + uint64(iteration)
		damageRand, hitRand, critRand := labeled(seed, "Damage Roll"), labeled(seed, "Magical Hit Roll"), labeled(seed, "Magical Crit Roll")
		mana, damageTotal := r.Caster.MaxMana, 0.0
		fiveSecondRule := uint64(0)
		q.events = q.events[:0]
		q.sequence = 0
		q.push(event{at: 0, kind: ready})
		q.push(event{at: 2 * second, priority: 1, kind: manaTick})
		for len(q.events) > 0 {
			e := q.pop()
			if e.at > r.Duration {
				break
			}
			switch e.kind {
			case manaTick:
				work.ManaTicks++
				regen := r.Caster.RegenIdle
				if e.at < fiveSecondRule {
					regen = r.Caster.RegenCasting
				}
				mana = min(r.Caster.MaxMana, mana+2*regen)
				q.push(event{at: e.at + 2*second, priority: 1, kind: manaTick})
			case ready:
				work.ReadyChecks++
				if mana >= r.Spell.ManaCost {
					q.push(event{at: e.at + r.Spell.Cast, kind: complete})
					if r.Spell.GCD > r.Spell.Cast {
						q.push(event{at: e.at + r.Spell.GCD, kind: ready})
					}
				} else {
					nextTick := (e.at/(2*second) + 1) * 2 * second
					delta := nextTick - e.at
					polls := delta / r.Reaction
					if delta%r.Reaction != 0 {
						polls++
					}
					q.push(event{at: e.at + polls*r.Reaction, kind: ready})
				}
			case complete:
				work.Completions++
				mana -= r.Spell.ManaCost
				if r.Spell.ManaCost > 0 {
					fiveSecondRule = e.at + 5*second
				}
				counts.Casts++
				base := r.Spell.Min
				if r.Spell.Max > r.Spell.Min {
					work.DamageRolls++
					base += (r.Spell.Max - r.Spell.Min) * damageRand.float()
				}
				damage := (base + r.Spell.Coefficient*r.Caster.SpellPower) * r.Spell.DamageMultiplier
				outcome := hit
				work.HitRolls++
				if hitRand.float() >= hitChance {
					damage = 0
					outcome = miss
					counts.Misses++
				} else {
					work.CritRolls++
					if critRand.float() < r.Caster.CritPercent/100 {
						damage *= r.Spell.CritMultiplier
						outcome = crit
						counts.Crits++
					} else {
						counts.Hits++
					}
				}
				q.push(event{at: e.at + r.Spell.Travel, priority: -1, kind: impact, damage: damage, outcome: outcome})
				if r.Spell.Cast >= r.Spell.GCD {
					q.push(event{at: e.at, kind: ready})
				}
			case impact:
				work.Impacts++
				damageTotal += e.damage
			}
		}
		dps := damageTotal / (float64(r.Duration) / float64(second))
		n := float64(iteration + 1)
		delta := dps - mean
		mean += delta / n
		m2 += delta * (dps - mean)
		manaMean += (mana - manaMean) / n
	}
	stdev := math.Sqrt(m2 / float64(r.Iterations))
	elapsed := time.Since(start).Nanoseconds()
	return Report{Engine: "forever-matched-go-kernel", Revision: revision, ID: r.ID, Iterations: r.Iterations, Seed: r.Seed, Mean: mean, Stdev: stdev, Error: stdev / math.Sqrt(float64(r.Iterations)), Counts: counts, Work: work, ManaEnd: manaMean, ManaDelta: manaMean - r.Caster.MaxMana, Elapsed: elapsed, Trace: []struct{}{}}, nil
}

func run() error {
	if len(os.Args) < 2 || (os.Args[1] != "sim" && os.Args[1] != "bench") {
		return errors.New("expected sim or bench")
	}
	flags := flag.NewFlagSet(os.Args[1], flag.ContinueOnError)
	infile := flags.String("infile", "", "prepared JSON request")
	outfile := flags.String("outfile", "", "JSON report")
	warmups := flags.Int("warmups", 3, "untimed warmups")
	samples := flags.Int("samples", 7, "timed samples")
	if err := flags.Parse(os.Args[2:]); err != nil {
		return err
	}
	if len(flags.Args()) != 0 || *infile == "" || *warmups < 0 || *warmups > 20 || *samples < 1 || *samples > 100 {
		return errors.New("invalid arguments")
	}
	file, err := os.Open(*infile)
	if err != nil {
		return err
	}
	defer file.Close()
	decoder := json.NewDecoder(file)
	decoder.DisallowUnknownFields()
	var request Request
	if err = decoder.Decode(&request); err != nil {
		return err
	}
	if err = decoder.Decode(&struct{}{}); err != io.EOF {
		return errors.New("unexpected trailing JSON")
	}
	var output any
	if os.Args[1] == "bench" {
		var sink Report
		for i := 0; i < *warmups; i++ {
			sink, err = simulate(&request)
			if err != nil {
				return err
			}
		}
		reports := make([]Report, 0, *samples)
		timings := make([]int64, 0, *samples)
		for i := 0; i < *samples; i++ {
			report, err := simulate(&request)
			if err != nil {
				return err
			}
			reports = append(reports, report)
			timings = append(timings, report.Elapsed)
		}
		runtime.KeepAlive(sink)
		output = struct {
			Warmups int      `json:"warmups"`
			Samples int      `json:"samples"`
			Timings []int64  `json:"elapsed_ns_samples"`
			Reports []Report `json:"reports"`
		}{*warmups, *samples, timings, reports}
	} else {
		output, err = simulate(&request)
		if err != nil {
			return err
		}
	}
	data, err := json.MarshalIndent(output, "", "  ")
	if err != nil {
		return err
	}
	data = append(data, '\n')
	if *outfile != "" {
		return os.WriteFile(*outfile, data, 0644)
	}
	_, err = os.Stdout.Write(data)
	return err
}
func main() {
	if err := run(); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}
