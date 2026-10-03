# Forever Rust prototype

An independently executing Frostbolt simulation kernel for MythicSim's proposed
Rust Forever engine. This is an experimental spike, not a production backend.

The kernel simulates cast completion, the global cooldown, projectile arrival,
binary resistance, hit and crit rolls, mana spending at cast completion, two-second
mana ticks, the five-second rule, and reaction-time alignment after mana starvation.
It computes DPS mean, population standard deviation, standard error, spell outcome
counts, and final mana. An optional first-iteration trace shows event times.

## Scope and input boundary

- Level 60 caster, Frostbolt spell 25304, one level 60 to 63 target.
- Fixed encounter duration, a Frostbolt-only rotation, and static character/spell parameters.
- Checked-in fixtures use a Human Mage with racials disabled, Robe of the Archmage
  (14152), Briarwood Reed (12930), and either Azuresong Mageblade (17103) or
  Staff of Dominance (18842). The remaining slots are empty.
- Static talent effects are resolved by the pinned Go engine when preparing inputs.
  The sparse talent strings isolate mechanics. These are test characters, not
  complete raid builds or suggested talent allocations.
- No dynamic procs, Fingers of Frost, Winter's Chill, Ice Lance, cooldowns, pets,
  arbitrary APL interpretation, gear database, addon importer, or full raid model.

**Rust consumes a prepared snapshot, not a `RaidSimRequest`.** Preparation currently
uses Go to resolve gear, base stats, spell data, and static talents. Rust does not
call Go during a simulation. Fields, schema versions, spell IDs and revisions outside
the supported input contract fail with a nonzero exit. This binary must not replace
`FOREVER_BINARY_PATH`: its input and output schemas are deliberately different.

The pinned reference is `sage3648/mythicsim-forever-engine` revision
`6823b49eb8aff741f197ef36d83766ef6a218285`, the revision recorded by the production
checkout at implementation time. This is compatibility evidence for that revision,
not proof of current game behavior or a claim about the currently running deployment.

## Run without Go or network access

From the repository root:

```sh
cargo test --locked --manifest-path Cargo.toml
cargo run --locked --release --manifest-path Cargo.toml -- \
  sim --infile fixtures/static-frost-60.rust.json --trace
```

The crate only depends on Serde and serde_json. After Cargo dependencies are cached,
`--offline` can be added. `version` identifies the prototype and its source revision.

Inputs have integer nanosecond timestamps and bounded iteration counts. The seed is
nonzero and fits positive Go int64. Reports are JSON. Cast and outcome counts are
totals across all iterations; DPS and final mana are means.

Frostbolt is a binary spell. Resistances reduce hit chance, and landed hits take no
partial resistance damage. Hit chance stays capped at 99%. Outcomes are counted
at cast completion, as in the source engine, but damage only contributes when the
missile arrives within the encounter. A completed cast can therefore contribute
an outcome without contributing damage.

## Re-run the actual Go comparison

Requires Python 3, Git, Go 1.25 or later, protoc, and Rust. The script builds its own
protoc-gen-go using the pinned engine dependency. It clones the pinned source into
an ignored cache, generates protos there, and compiles a thin helper that calls the
actual engine. It never edits the supplied source repository.

```sh
python3 tools/compare.py
```

A local engine repository containing the pinned commit avoids cloning over the network:

```sh
python3 tools/compare.py --source /absolute/path/to/engine-repository
```

By default the harness compares 11 scenarios at 3,000 iterations, seeds 42, 173,
and 9001, with three repeats and alternating process order. It requires identical
casts/hits/crits/misses, DPS mean and spread within 1e-8 absolute, and average mana
delta within 1e-8. Failure produces a nonzero exit. Raw requests, reports, timing
samples, and `summary.json` are saved under ignored `output/`.

Use `--case resistance --seeds 42 --repeats 1 --trace` to inspect a specific case.
Trace-enabled runs are excluded from timing medians. `--iterations`, `--repeats`,
`--cache`, and `--output` allow controlled experiments.

The normal Rust test suite replays frozen outputs from the real Go engine without
requiring Go. Those goldens complement live differential comparisons and analytic
tests for the binary hit table, mana exhaustion and encounter boundaries.

## Performance interpretation

The original full-engine comparison is an accuracy check and a comparison of
different designs. For the fairer comparison added afterward, run:

```sh
python3 tools/fair_compare.py
```

This builds a separate, dependency-free Go implementation of the same prepared
kernel. It is a benchmark control, not an optimization to the upstream engine.
Both kernels use the same snapshots, binary event-heap design, mana-check skipping,
labeled SplitMix64 streams, Welford aggregation, and result fields. Event and random
draw counters must match exactly. Go uses a typed heap to avoid per-event interface
boxing or allocation. Both reuse queue capacity across iterations.

Each case is first checked against the actual pinned engine at 3,000 iterations.
The matched kernels then run 30,000 iterations per sample, three in-process warmups,
seven samples per batch and three batches, across all eleven cases and three seeds.
Execution order is shuffled deterministically for each batch. Every timed sample's
metrics and work counters are checked. Go is limited to one logical processor,
with the standard GC setting (`GOGC=100`), and Rust uses one simulation thread.
Parsing, process startup, serialization and character preparation are outside both
timers. Output includes all timing samples and source/compiler identities.

Results are saved under `output/fair/`. Use `--case static-frost-60 --seeds 42`
for a smaller run. `--iterations`, `--warmups`, `--samples` and `--batches` control
measurement duration. The Go control has its own tests against the frozen real
engine fixtures:

```sh
cd tools/matched-go
go test ./...
go vet ./...
```

This provides a fairer language/implementation comparison for the current kernel.
It still excludes full character construction, dynamic procs, other spells, the
production APL, richer reports and parallel workloads. Shared-host timing variance
and compiler/data-layout differences remain. See the
[follow-up results](forever-rust-fair-comparison-2026-10-03.md).

### Original full-engine comparison

The harness uses serial iterations in both engines. Kernel timing excludes process
startup and file I/O. The Go timer includes environment construction and result
aggregation; Rust uses prepared parameters and a smaller result contract. Process
wall time is recorded separately. No CPU affinity, memory benchmark, or isolation
from other host workloads is imposed.

Rust skips unsuccessful mana checks until a mana tick can change readiness, while
preserving the source engine's reaction-time grid. This is valid because this
kernel has no dynamic readiness conditions. Adding procs or cooldowns requires
revisiting that optimization.

The benchmark can establish the value of this constrained design. It cannot
attribute the entire difference to Rust or predict full-class or production latency.
The Go CLI normally parallelizes iterations; the oracle deliberately calls the
serial engine API to compare against this single-threaded Rust implementation.

## Next implementation steps

The current [roadmap](../ROADMAP.md) starts with one complete Frost build and a
Go preparation adapter, then evaluates hybrid integration. Moving character
construction and data ownership into Rust remains a later cutover decision.

No worker routing, production queues, catalogs, public rankings or deployments are
changed by this prototype.

## Attribution and license

The reference engine, RNG behavior, and ported combat semantics are derived from the
MIT-licensed [wowsims Forever project](https://github.com/ElliotWood/Forever) and
[MythicSim's fork](https://github.com/sage3648/mythicsim-forever-engine).
The upstream copyright and MIT notice are preserved in [LICENSE](../LICENSE).
Any eventual user-facing integration should retain the existing visible upstream link.
