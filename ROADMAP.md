# Roadmap

The intended destination is a complete Rust engine for the Forever features used by
MythicSim. A staged hybrid migration keeps Go preparation and fallback available
while each capability is validated. Go remains a development reference after
production cutover so community fixes can still be reviewed and ported.

The detailed [engine implementation plan](docs/hybrid-migration-plan.md) defines
the target architecture, pieces, dependencies, acceptance gates, AI-assisted
reconciliation process and attribution checkpoints.
Milestones advance on evidence, not promised delivery dates. The
[progress log](docs/progress-log.md) records the actual time each piece took.

## Foundation completed

- Prepared Frostbolt kernel, deterministic RNG and strict input validation.
- Eleven frozen Go reference scenarios and live comparison tools.
- Matched Go/Rust benchmark, work counters and historical results.
- Standalone public repository, contribution guide and CI.
- First Frost build inventory: real application request, full Go reference
  observation, mechanics and report consumers, source hashes and offline audit.
- Shared engine/core/mechanics and class/spec module boundaries, mirrored class
  tests and a contributor code map. Current simulation scope remains unchanged.
- [Prepared v2 contract](docs/prepared-v2.md): a Go exporter for real characters,
  strict Rust types, a coverage gate with named fallback reasons, an accepted fixture
  family and a [release compatibility manifest](release/manifest.json).
- [Upstream ledger](UPSTREAM.md#ledger): reviewed community range, dispositions, a
  Frost mechanics map and one applicable fix traced to a Rust guard and regression.
- A class-independent fight runtime mirroring Go, with Mage plugged in. The complete
  Frost, Arcane and Fire reference builds reproduce the pinned Go engine on the
  application's requests, on their variants and on randomized sweeps, across the
  whole result and the logs the application parses into timelines.
- Every race that can be a Mage, with its racials, matches Go on all three builds and
  on a randomized race sweep.
- All 29 production builds, across every class, run in Rust and match Go on about 7,900
  production variants and 3,700 random ones, with no mismatches. Every class also runs
  against 2 to 5 targets; tank builds there still fall back to Go.
- Production routing tools: stable refusal codes, the engine identity in every result,
  [`tools/route.py`](docs/routing.md) for one request or a whole batch job in one engine,
  determinism across worker counts and processes, interrupted runs that leave no partial
  result, and the [release and rollback process](docs/release.md).
- [Rust preparation](docs/rust-preparation.md): the engine builds every class's character
  from the application's request itself, as the pinned Go engine constructs it, and writes
  the same prepared state as the exporter. All 358 accepted fixtures prepare in Rust and
  equal the exporter's state exactly; a request it does not cover is refused with a code and
  the exporter prepares it, so routed jobs no longer start the Go exporter.
- [Shadow sims](docs/shadow-sims.md) on real MythicSim traffic: no mismatch with the
  pinned Go engine since they started.

## Migration sequence

| Phase | Outcome |
| --- | --- |
| 0 | Inventory product scope and define versioned input, preparation and result contracts |
| 1 | Establish upstream change tracking, mechanics mapping and reference manifests |
| 2 | Prepare real characters in Go and generalize the Rust combat primitives |
| 3 | Run one complete Frost build, rotation and required reports in Rust |
| 4 | Enable validated hybrid jobs with comparison evidence and Go fallback |
| 5 | Expand class mechanics, custom inputs and required product job modes |
| 6 | Replace Go preparation with Rust and reproducible data ingestion |
| 7 | Complete production cutover, retaining Go only as a development reference |

## Start here

1. Completed: [inventory the chosen Frost build and actual application consumers](docs/first-frost-inventory.md).
2. Completed: [prepared v2 and a release compatibility manifest](docs/prepared-v2.md).
3. Completed: [the community change ledger and Go-to-Rust mechanics map](UPSTREAM.md#ledger).
4. Completed: one reference pin and full-result differential diagnostics.
5. Completed: every production build matches the pinned Go engine.
6. Completed: routing with Go fallback, batch jobs in one engine and shadow comparison runs.
7. Next: make preparing a routed job cheaper than the time Rust saves
   ([#59](https://github.com/sage3648/mythicsim-forever-engine/issues/59); see the
   [whole-job benchmark](benchmarks/2026-10-06-whole-jobs.json)), then route a small share
   of Quick Sims to Rust following the [release stages](docs/release.md#stages), and close
   the coverage gaps the shadow sims refuse
   ([the board](https://github.com/sage3648/mythicsim-forever-engine/issues/25)).

The [first usable release](docs/hybrid-migration-plan.md#first-usable-release)
is one complete Frost build using Go preparation and Rust combat execution, with
the required production result fields. It does not imply general Mage coverage.

The [first contribution pieces](docs/hybrid-migration-plan.md#first-contribution-sized-pieces)
provide a reviewable backlog. Routing in production and
upstream sync automation are not implemented yet.
