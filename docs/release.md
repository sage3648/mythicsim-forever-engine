# Release and rollback

A Rust engine release is a bundle that the MythicSim worker runs: the release Rust
engine, the Go exporter at the reference pin, the comparison and routing tools, and a
`manifest.json` naming every revision. This guide covers how a release is made, checked,
observed and rolled back to Go. The application side, its settings and its logs, is
documented in the MythicSim repository in `docs/forever-shadow-sims.md` and
`docs/forever-rust-routing.md`.

Go stays deployed for the whole hybrid migration. Rolling back never needs a Go build or a
Go deployment: it only stops sending sims to Rust.

## Stages

A release moves through three stages. Each stage has an observation window, and the
release advances only when the window closes clean.

| Stage | Worker setting | Users see | Window closes clean when |
| --- | --- | --- | --- |
| Shadow | `FOREVER_SHADOW_BUNDLE` | Go only | At least 50 shadow verdicts over at least 24 hours, no `mismatch`, no `error` that the bundle caused, and Rust within noise of production on every `match` |
| Routed sample | `FOREVER_ROUTE_BUNDLE` and a small `FOREVER_ROUTE_SAMPLE_RATE` | Rust for a share of supported Quick Sims | At least 24 hours at each rate, no `fault` decisions, and the shadow verdicts on the same bundle stay clean |
| Routed | `FOREVER_ROUTE_SAMPLE_RATE=1` | Rust for every supported Quick Sim, Go for the rest | At least 7 days with no fault and no correctness report traced to Rust |

The routed sample rises in steps, for example 0.01, 0.05, 0.25 and 1, with a full window
at each. A refused request is not a failure: it falls back to Go with its refusal codes,
which only measure coverage. A fault is a failure at any stage.

## Make a release

1. **Start from a green `main`.** CI passes, including the production architecture check.
   Locally, the checks in [CONTRIBUTING.md](../CONTRIBUTING.md#development-checks) pass and
   `python3 tools/validation_summary.py check` reports the summary is current.
2. **Match the production engine.** The pin in [upstream/sources.json](../upstream/sources.json)
   must equal the application's `FOREVER_SHA`. The worker refuses to route while they
   differ. If production moved, move the pin first, following [UPSTREAM.md](../UPSTREAM.md).
3. **Update the release manifest.** [release/manifest.json](../release/manifest.json) names
   the Go reference revision, the community base, the client build, the Go database digest,
   the contracts and the capabilities with their evidence. Each must agree with the pin and
   the validation records of this commit; `python3 tools/prepared_v2.py check` rejects
   fixtures whose revision or client build differs.
4. **Tag the commit** `bundle-<short-sha>` with an annotated tag, so a release can always
   be rebuilt from its source.
5. **Build the bundle** on the worker machine, from a clean checkout of the tagged commit:

   ```sh
   python3 tools/shadow.py build --output ~/forever-shadow/bundle-<short-sha>
   ```

   The bundle's `manifest.json` records `rust_dirty`; a release bundle must have it false.
6. **Smoke test the bundle** before any worker uses it. Run a supported fixture request and
   a refused one through the bundle's own tools, and check the decisions:

   ```sh
   python3 ~/forever-shadow/bundle-<short-sha>/tools/shadow.py run --request REQUEST.json --output /tmp/smoke-shadow --seed 7
   python3 ~/forever-shadow/bundle-<short-sha>/tools/route.py --request REQUEST.json --output /tmp/smoke-route
   ```

   The shadow verdict must be `match`, the supported route decision `rust`, the refused one
   `fallback` with its codes. The decision's `bundle` must name the tagged commit in
   `rust_revision`, and the result's `identity` the pin and the client build.

## Observe a release

- **Shadow verdicts:** the admin-only Discord channel, and the application's
  `GET /api/admin/forever-shadow` for the 24 hour and 7 day summaries and the runs by
  status. Every verdict carries the bundle manifest, so a window only counts verdicts of
  the bundle under observation.
- **Routing decisions:** the worker logs each decision as `forever route` with the sim id,
  the status, the refusal codes, the failing stage and the step timings.
- **Stored reports:** a routed sim's engine version is
  `forever-rust-<rust revision>-ref-<pin>`, so every sim a release served can be found
  later, and a stored report keeps the identity of the engine that made it.

A mismatch or fault closes the window unclean. Keep or roll back the stage as below, accept
the request as a fixture with `python3 tools/prepared_v2.py accept`, and fix it in a new
release; never widen the gate to admit it.

## Roll back

Rolling back sends every sim to Go again. In order of reach:

1. **Stop routing.** Set `FOREVER_ROUTE_SAMPLE_RATE=0`, or unset `FOREVER_ROUTE_BUNDLE`,
   and restart the worker. Every Quick Sim runs in Go from the next one on. Sims already
   served by Rust keep their results and their Rust engine version.
2. **Return to the previous bundle.** Point `FOREVER_ROUTE_BUNDLE`, or
   `FOREVER_SHADOW_BUNDLE`, at the previous release's bundle and restart the worker. Keep
   at least the two most recent release bundles on the worker machine for this.
3. **Stop shadow runs.** Unset `FOREVER_SHADOW_BUNDLE` and restart the worker; queued
   shadow runs expire. Set `FOREVER_SHADOW_SAMPLE_RATE=0` on the API to stop starting them.

Back up the worker's environment file before each change, change only the line named,
and confirm the worker's start log before calling the rollback done.

## Rehearse the rollback

Rehearse a rollback once before the routed sample first rises above its smallest rate, and
again whenever the worker's deployment changes:

1. With routing on at the smallest rate, record the time and stop routing as in step 1
   above.
2. Confirm the worker's start log shows no route bundle, and that the next routed-eligible
   Quick Sim's report carries the Go engine version.
3. Restore the setting and confirm the next routed sim carries the Rust engine version.
4. Record the rehearsal in `validation/<date>-rollback-rehearsal.json`: the bundle, the
   steps, the time from the change to the first Go-served sim, and who ran it.

## Full cutover

Removing Go from production follows the cutover gates of the
[implementation plan](hybrid-migration-plan.md#phase-7-complete-production-cutover): every
required job runs in Rust, rollback has been rehearsed, no unexplained correctness failure
remains, and the previous compatible Go deployment stays available for the observation
window after the switch.
