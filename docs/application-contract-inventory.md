# Application contract inventory

This inventories consumers at application commit
`1ca721161aba693a899466943b8f6c3a6085df67`. It identifies requirements for the
future adapter, without introducing a new wire schema or production routing.
Machine-readable fields, dispositions and source hashes are under `consumers`
in [manifest.json](../inventory/first-frost/manifest.json).

## Inputs and job boundary

The application accepts an addon character export: race, class, level, talents,
optional identity-based talent spell lists, professions, equipped IDs, enchants,
random suffixes and optional bag candidates. The worker constructs the Go
`RaidSimRequest`, including effective rotation and raid assumptions.

The current subprocess invocation is `sim --strict --infile ... --outfile ...`.
Unknown proto fields and enum names must fail. Seeds are decimal strings in
protojson, not arbitrary JSON numbers. The initial frozen request is one player,
one target, 3,000 iterations and first-iteration debug.

Relevant product variation exists even though it is outside the first frozen
reference: iterations up to 25,000; fight duration 30 to 600 seconds; variation
0 to 120 seconds and shorter than the fight; 1 to 5 targets; distance 0 to 40
yards; disabled assumptions; selected potion; alternate preset or custom APL;
timeline and stat-weight options. Custom APL is bounded to 64 KiB and 200 actions
including groups and prepull actions. A declared Frost capability must not silently
accept all these variations.

For release 1, unsupported variations need an explicit capability decision before
simulation. The future worker can route the complete original request to Go.
Gear comparisons and stat weights must choose one engine for the entire batch.
Piece 1 does not implement that decision or enable any jobs.

## Output consumers

| Consumer | Fields or behavior that must survive |
| --- | --- |
| Worker result parser | Embedded `error.message`, positive `iterationsDone`, player DPS/HPS, average fight duration |
| API headline | PascalCase `summary` fields, iterations, confidence from standard error, character/spec, warnings |
| Ability rows | Action ID/tag, total damage/casts across iterations, enemy unit index, direct and tick outcomes |
| Breakdown | DPS histogram, aura proc counts and mean uptime, resource gain/actual gain/events, overcap, OOM, threat |
| Settings and talents | Actual request assumptions/options/talents, effective rotation identity and character metadata |
| Rotation card | Prefer `effectiveRotation`, fall back to input; `rotationStats` determines whether validation is shown |
| Timeline and log | First-fight casts, spans and resource points; separate combat log artifact; averaged timeline |
| Deferred modes | `stat_weights`, errors, gear comparison results, ranking/lab scenario and eligibility metadata |

The artifact envelope currently uses `engine: "wowsims-forever"`, an
`engine_version`, character/spec metadata, rotation metadata, `summary`, `request`
and raw `result`. The existing API rejects other engine names. Rust result identity
requires an intentional adapter/parser decision in piece 2 and later integration;
renaming the repository alone does not change this contract. Preserve historical
report identities.

Headline fields are `DPSMean`, `DPSStdev`, `DPSMin`, `DPSMax`, `HPSMean`,
`IterationsDone` and `AvgIterationDuration`. Abilities use damage divided by
`iterations * average_duration`, not the configured duration. Action counters and
resource flows are totals; aura `uptimeSecondsAvg` and `procsAvg` are already means.
Enemy action entries must be separated from self effects. Channel cast and damage
tick identities are distinct. Preserve proto omitted-zero behavior and enum names.

The current Go observation omits `effectiveRotation` and `rotationStats`. API
fallback behavior remains part of the contract. Do not report an unvalidated
rotation as validated by filling an empty success object.

The worker normally builds an averaged timeline using a second 100-fight run,
seed `"7"` and full debug, in addition to the first-fight log. Both timelines are
requirements for normal result compatibility. This piece captures the first-fight
timeline only; averaged timeline execution and report parity belong to piece 10.

The API converts raw Go output into `forever_breakdown`, `forever_advanced`,
`forever_talents`, gear comparisons and standard ability rows. The Next.js result
view consumes those normalized API objects. Its boundary is not the raw Rust
kernel JSON.

## Scope and evidence limits

The full Go reference completed 3,000 iterations, its embedded error was checked,
and the application's `ParseResult`, `NewReport`, `TakeLogs` and `ParseTimeline`
helpers consumed it. The frozen output is trimmed reference evidence. No Rust
report adapter, API/frontend end-to-end compatibility test or averaged timeline
capture has been implemented here.

Quick Sim for the exact first build is the first intended migrated job. Best Gear,
trinket boards, stat weights, public rankings, lab workflows, custom rotations,
other specs and broader gear/talent variants remain Go requirements. The manifest
records these consumers so later migration phases cannot accidentally omit them.
