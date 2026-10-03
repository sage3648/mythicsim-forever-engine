# Matched Go and Rust Forever kernels, 2026-10-03

The fairer comparison shows a substantially smaller Rust advantage than the
original scope-mismatched benchmark. Across the 27 normal-duration scenario/seed
pairs, the median Go/Rust time ratio is **1.42x**, with ratios from 1.26x to 1.79x.
The two three-second boundary scenarios are faster in Go on all three seeds.

This replaces the original 13x example as evidence for comparing equivalent
kernel implementations. It does not establish full-engine or production speedups.

## Controls

A new dependency-free Go kernel implements the same bounded simulation as the Rust
kernel. Both consume identical prepared JSON requests, run a Frostbolt-only rotation,
reuse a binary event heap, skip unsuccessful mana polls until the same readiness
grid point, use equivalent labeled SplitMix64 streams, and accumulate the same
metrics using Welford's algorithm. The Go heap stores values directly, avoiding
interface boxing and allocation per event.

The timed scope includes the same simulation state setup, per-iteration seeding,
events, damage calculations and statistical aggregation. Character preparation,
JSON parsing, process startup, report metadata construction and serialization are
outside both timers. Trace capture is disabled. Both kernels expose counters for
mana ticks, readiness checks, completed casts, missile impacts, damage rolls,
hit rolls and crit rolls. Every sample must match these counters exactly as well
as numerical results.

Go uses `GOMAXPROCS=1`, `GOGC=100` and no memory limit. Rust uses one simulation
thread. The Go binary uses default compiler optimizations and `-trimpath`; Rust
uses the release profile with thin LTO and one codegen unit. Go 1.25.6 and Rust
1.96.0 ran on an Apple M2 Max, macOS 26.5, arm64, 12 logical CPUs.

Each of 11 scenarios and three seeds (42, 173, 9001) first passes comparison against
the actual pinned full Go engine at 3,000 iterations. Each matched kernel then
executes 30,000 iterations per timed sample. Every process performs three warmups,
then seven timed samples. There are three batches per scenario/seed pair, with
deterministically shuffled language order.

## Results

**All 33 scenario/seed pairs passed.** All 693 timed samples per language agree on
metrics and work counters. Spell counts and work counters match exactly; numerical
metrics match within 1e-8 absolute. This establishes equivalent work for this model,
not identical machine instructions or an independent verification of live-game rules.

The table uses the median of the three per-seed medians, each derived from 21 samples.
All times are for 30,000 iterations. A Go/Rust ratio above one favors Rust.

| Scenario | Matched Go | Rust | Go/Rust ratio |
| --- | ---: | ---: | ---: |
| Untalented, 60 seconds | 66.642 ms | 39.809 ms | 1.67x |
| Static Frost talents, 60 seconds | 63.891 ms | 46.692 ms | 1.37x |
| Static Frost talents, 120 seconds | 85.312 ms | 62.260 ms | 1.37x |
| Meditation, 300 seconds | 257.250 ms | 153.895 ms | 1.67x |
| Haste, 60 seconds | 71.784 ms | 45.467 ms | 1.58x |
| Travel boundary, 3 seconds | 7.422 ms | 11.689 ms | 0.64x |
| Zero-travel boundary, 3 seconds | 6.903 ms | 11.869 ms | 0.58x |

The [benchmark snapshot](../benchmarks/2026-10-03-fair.json) retains
all samples, execution order, exact work counts, build/runtime settings, and source
hash. The three-second cases spend a larger proportion of their time setting up
iteration random streams. Attribution of their difference to a particular compiler
or allocation behavior requires profiling; that was not measured here.

## Interpretation and limits

The original large ratio bundled together language, reduced scope, prepared data,
different aggregation, and skipped mana checks. Once Go receives the same design,
most of that large apparent Rust advantage disappears. Rust remains faster on
every normal-duration case's median in this local run, but the language change is
not uniformly beneficial across workloads.

The practical decision is to profile and optimize the existing Go engine before
justifying a full port on speed alone. Rust can still be useful for ownership,
memory control and integration, and the matched kernel makes further experiments
possible without conflating language and simulator scope.

The Go control is newly implemented benchmark code, not a patch to the upstream
simulator. Both kernels still omit dynamic procs, additional spells, full APL
execution, character construction, richer production reports and parallel workers.
The host was shared, with no CPU pinning or CPU-time/memory measurement. These are
local kernel wall-time measurements, not cost savings or deployment performance.

## Reproduce and checks

```sh
python3 tools/fair_compare.py
```

Use `--source /absolute/path/to/engine-repository` to clone the pinned engine locally.
For a smaller experiment, add `--case static-frost-60 --seeds 42 --batches 1`.
Requests, reference reports and all per-batch reports go under ignored `output/fair/`.
The actual pinned Go engine remains the accuracy oracle. The original
[prototype report](forever-rust-prototype-2026-10-03.md) remains as historical evidence.

Validation includes the Rust regression suite, the matched Go kernel's tests against
all eleven pinned-engine goldens and its heap/RNG tests, Go vet, Rustfmt, Clippy with
warnings denied, and Python tests that reject equal-DPS results with different work.
CI runs these checks using frozen fixtures, without running the full benchmark.
No production worker, API, queue, ranking or deployment was changed.
