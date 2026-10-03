# Forever Rust prototype, 2026-10-03

Completed the bounded first spike in [forever-engine](../README.md).
It independently executes a prepared Frostbolt-only Mage simulation. It is not a
full Mage implementation or a replacement for the worker's Forever binary.

The original timing ratios below compare different scope. The subsequent
[matched-kernel comparison](forever-rust-fair-comparison-2026-10-03.md) is the
appropriate result for assessing Rust against an equivalent Go implementation.

## Accuracy evidence

Reference: MythicSim's Go engine at
`6823b49eb8aff741f197ef36d83766ef6a218285`, client 1.60.1.70170.
The comparison helper calls the actual serial `core.RunRaidSim` API in an isolated
checkout of that commit. It does not reimplement the oracle's damage calculation.

Eleven scenarios at 3,000 iterations, three seeds (42, 173, 9001), three repeats:
**33 scenario/seed pairs and 99 paired comparisons passed**. Spell outcome totals
match exactly. DPS mean, population standard deviation and average mana delta match
within 1e-8 absolute. Both engines use equivalent labeled SplitMix64 random streams.

Coverage includes untalented Frostbolt; Improved Frostbolt, Elemental Precision,
Ice Shards, Piercing Ice and Frost Channeling resolved into static parameters;
Arcane Meditation's casting regeneration; 60, 120 and 300-second fights; mana
starvation; resistance; the hit cap; same-level targets; haste; two fixed weapons;
and projectiles crossing the encounter boundary with and without travel time.

Ten automated Rust tests pass, including all eleven frozen Go fixtures in one
regression test, an analytic expected-DPS check, RNG vectors, input rejection,
mana starvation and projectile boundaries. Rustfmt and Clippy with warnings denied
pass. CI runs those checks without downloading the Go source or running sims in
production. Raw Go requests and prepared snapshots are checked into
[fixtures](../fixtures/manifest.json).

Matching the old engine establishes compatibility for these mechanics. It does
not independently establish that every coefficient matches the live game.

## Local performance evidence

Apple M2 Max, macOS 26.5, arm64, 12 logical CPUs. Go 1.25.6; Rust 1.96.0,
release build with thin LTO. Each number is the median of the three per-seed timing
medians, with three repetitions per seed. These are kernel wall times for 3,000
iterations, excluding process launch and request/result file I/O.

| Prepared scenario | Go full-engine serial run | Rust constrained kernel | Ratio |
| --- | ---: | ---: | ---: |
| Untalented, 60 seconds | 58.202 ms | 3.628 ms | 16.04x |
| Static Frost talents, 60 seconds | 63.748 ms | 4.887 ms | 13.04x |
| Static Frost talents, 120 seconds | 151.516 ms | 6.998 ms | 21.65x |
| Meditation, 300 seconds | 323.224 ms | 15.850 ms | 20.39x |
| Haste, 60 seconds | 96.202 ms | 8.530 ms | 11.28x |

These ratios compare different implementation scope. Go constructs the full
environment, registers the class's spells and produces richer metrics. Rust begins
with resolved parameters, runs only Frostbolt, and skips failed mana-readiness
polls while retaining their timing grid. Therefore this is evidence for a faster
specialized kernel, not a measured language-only improvement or a predicted
production speedup. The production Go CLI also parallelizes iterations, while this
comparison uses serial iteration paths. The host was shared, with no CPU pinning
or memory/CPU-time profiling. Process wall times and all samples are retained in
the [benchmark snapshot](../benchmarks/2026-10-03.json).

## Implementation boundary and next decision

Go still prepares equipment, base stats, client spell data and static talent effects.
Rust owns casting, the GCD, binary hit/resistance math, damage and crit rolls, mana
ticks and the five-second rule, travel and fight boundaries, and aggregate statistics.
The Rust binary runs without Go using the checked-in prepared fixtures. Its input
contract rejects unknown fields and unsupported spell IDs or source revisions.

The next useful increment is Rust character construction and static Frost talent
resolution, followed by Ice Lance, Fingers of Frost and Winter's Chill. Only after
the production Frost preset and full gear inputs pass differential checks should
worker routing be considered. No API, worker, deployment, queue or public ranking
was changed by this spike.

Reproduce:

```sh
cargo test --locked --manifest-path Cargo.toml
python3 tools/compare.py
```

The harness can clone from an existing engine Git repository via `--source`.
See the [README](../README.md) for limits, trace options and attribution.
