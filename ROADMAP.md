# Roadmap

Release gates require correctness for a defined workload, supported by regressions
and differential comparisons. These are intended milestones, not delivery dates.

## Foundation, completed

- Prepared Frostbolt kernel, deterministic RNG and strict input validation.
- Mana, timing, resistance, encounter boundaries and aggregate metrics.
- Eleven frozen Go reference scenarios and live comparison tools.
- Matched Go/Rust benchmark, work counters and historical results.
- Standalone public repository, contributor docs and CI.

## 1. One complete Frost build

- Document versioned prepared inputs and required result fields.
- Add full character preparation, initially through a Go adapter for the hybrid.
- Implement Ice Lance, Fingers of Frost, Winter's Chill and other mechanics required
  by the chosen build, supported by evidence and focused tests.
- Add mana cooldowns and the rotation rules needed for that build.
- Compare complete characters, gear changes and timing/resource edge cases.
- Keep an explicit capability list and reject unsupported requests.

Acceptance: the selected build runs its full rotation with reviewed agreement
against Go, except for documented intentional corrections.

## 2. Evaluate the hybrid

- Compare complete jobs in development before enabling production routing.
- Define shared results and capability checks, with Go covering unsupported builds.
- Measure preparation, process overhead, full reports, memory and equal concurrency.
- Profile equivalent Go optimizations alongside Rust.
- Enable Rust only for validated inputs, with a Go fallback and rollback path.

Acceptance: useful measured benefits justify integration and maintenance costs.
No production router is implemented today.

## 3. Broaden community coverage

- Publish mechanics contribution examples and reference fixtures.
- Add builds and classes in small, independently reviewable increments.
- Track reviewed upstream changes and port applicable fixes with regressions.
- Add concurrency after deterministic single-threaded behavior is established.

## 4. Consider full Rust cutover

- Own character preparation, curated data ingestion and supported rotation execution.
- Cover the builds, gear interactions and reports required by MythicSim.
- Validate end-to-end compatibility, performance and release/rollback procedures.
- Remove Go from production only after coverage and maintenance are proven.

Full cutover is a decision gate, not a commitment. The hybrid may remain useful
long term if it delivers the required behavior with lower effort.
