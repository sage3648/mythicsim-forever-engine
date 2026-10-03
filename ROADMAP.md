# Roadmap

The intended destination is a complete Rust engine for the Forever features used by
MythicSim. A staged hybrid migration keeps Go preparation and fallback available
while each capability is validated. Go remains a development reference after
production cutover so community fixes can still be reviewed and ported.

The detailed [engine implementation plan](docs/hybrid-migration-plan.md) defines
the target architecture, pieces, dependencies, acceptance gates, AI-assisted
reconciliation process and attribution checkpoints.
Milestones advance on evidence, not promised delivery dates.

## Foundation completed

- Prepared Frostbolt kernel, deterministic RNG and strict input validation.
- Eleven frozen Go reference scenarios and live comparison tools.
- Matched Go/Rust benchmark, work counters and historical results.
- Standalone public repository, contribution guide and CI.
- First Frost build inventory: real application request, full Go reference
  observation, mechanics and report consumers, source hashes and offline audit.

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
2. Design prepared v2 and a release compatibility manifest.
3. Establish the community change ledger and Go-to-Rust mechanics map.
4. Consolidate reference pins and improve differential failure diagnostics.

The [first usable release](docs/hybrid-migration-plan.md#first-usable-release)
is one complete Frost build using Go preparation and Rust combat execution, with
the required production result fields. It does not imply general Mage coverage.

The [first contribution pieces](docs/hybrid-migration-plan.md#first-contribution-sized-pieces)
provide a reviewable backlog. Production routing, general preparation and upstream
sync automation are not implemented yet.
