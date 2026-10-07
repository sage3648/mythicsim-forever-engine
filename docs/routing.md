# Routing requests to Rust

The application worker runs engines as subprocesses. `tools/route.py` is the one
command it calls to run a request in Rust: it takes a `RaidSimRequest`, has one Rust process
prepare it, gate the prepared state with the coverage gate and run it when it is supported, and
otherwise tells the worker to use Go. A request Rust preparation does not cover is prepared by
the pinned Go exporter instead, and the decision's `preparation` names the provider. It runs from a bundle that
[`tools/shadow.py build`](shadow-sims.md#build-a-bundle) makes, so the worker needs
neither Go nor Cargo, only Python 3.

```sh
python3 BUNDLE/tools/route.py --request REQUEST.json --output NEW_FOLDER
```

| Option | Meaning |
| --- | --- |
| `--request` | The `RaidSimRequest` JSON the worker would give Go |
| `--batch` | Instead of `--request`, the requests of one batch job; see [batch jobs](#batch-jobs) |
| `--output` | A folder that does not exist yet; the command writes everything there |
| `--bundle` | The bundle folder; by default the one holding the tool |
| `--seed` | The seed for a request without one; by default a fresh one, as Go draws |
| `--timeout` | Seconds each step may take, 300 by default |

The command prints one JSON decision and writes it to `NEW_FOLDER/decision.json`.

| `status` | Meaning | Exit status | What the worker does |
| --- | --- | --- | --- |
| `rust` | Rust ran the request. `result` names `NEW_FOLDER/result.json`, its `RaidSimResult` in the JSON Go prints; `identity` is the engine that made it | 0 | Serve the Rust result |
| `fallback` | The gate refused the input. `refusals` lists each reason with its stable code, and `codes` the distinct codes. No result is written | 0 | Run the request in Go and count the fallback by its codes |
| `fault` | A step failed. `stage` is `request`, `prepare`, `check` or `rust`, and `error` says what failed; the gate runs in the same process as the simulation, and a `check` fault is an invalid prepared input or an unreadable refusal | 2 | Record a fault; the user can still be served by Go, but a fault is never counted as a fallback |

A fault is anything that is not a decision: an unreadable request, a step that exits with
an error or outlives its timeout, gate output that cannot be read, a refusal without a
reason, or a Rust result without its metrics. A fault never writes `result.json`, so a
partial simulation is never returned.

Every decision also carries `timings_ms`, the wall time of each step that ran (`prepare`, then
`rust`, which includes the gate, or `check` alone for a refusal), `seed` and
`seed_source` (`request`, `argument` or `drawn`), and the bundle's `manifest.json`
when the bundle has one. The refusal codes are listed in the
[prepared v2 contract](prepared-v2.md#unknown-and-unsupported-input).

A request keeps its own seed. One without a seed gets a fresh random one, as Go would
draw, so results stay as varied as Go's; the decision records it, so the run can be
repeated exactly.

## Cost of a routed job

Rust now prepares the request itself ([Rust preparation](rust-preparation.md)), so a routed
job is Python and one Rust process that prepares, gates and simulates; the Go exporter runs
only after a preparation refusal. The
[whole-job benchmark](../benchmarks/2026-10-07-whole-jobs-rust-preparation.json) measures the
42 production requests at 3,000 iterations: Rust prepared all 40 it routed, every one gave
Go's DPS, the median Go over Rust wall time is 1.15 and a routed job peaks at about 23 MB
instead of the exporter's 93 MB. Four jobs at a time, routed Rust ran 284 jobs a minute and
Go 213. The machine was heavily shared during that run, so its absolute times are loose.

Rust's prepare takes 15 to 50 ms for most builds. The Warrior, Protection Paladin, Bear and
Enhancement Shaman builds still take 0.1 to 0.35 s, because preparation builds a reset
simulation for every stat aura combination, as Go's exporter does
([the earlier record](../benchmarks/2026-10-06-whole-jobs-after-cheaper-prepare.json) explains
why one simulation cannot serve several combinations).

## Batch jobs

For Best Gear, stat weights and rankings, run every request of a batch in one engine, so
small differences between candidates never come from the engines. `--batch` decides the
requests of one job together:

```sh
python3 BUNDLE/tools/route.py --batch CANDIDATE.json ... --output NEW_FOLDER
```

Every request is prepared and checked before any runs, and Rust runs only when the gate
supports all of them. Each request writes its files into `NEW_FOLDER/000`, `001` and so
on, and the decision lists them in `requests`, each with its own status.

| `status` | Meaning | Exit status | What the worker does |
| --- | --- | --- | --- |
| `rust` | Every request ran in Rust; each entry of `requests` names its `result` | 0 | Serve every Rust result |
| `fallback` | The gate refused at least one request, whose entry carries its `refusals`; `codes` gathers the codes of all of them. Rust ran none | 0 | Run the whole batch in Go |
| `fault` | A step failed for at least one request, whose entry names the `stage` and `error`. Results of requests that had already run are deleted and marked `discarded` | 2 | Run the whole batch in Go and record a fault |

Requests without their own seed all get the same seed, `--seed` or one drawn for the batch
and recorded as `seed`, so the candidates share their random numbers.

## Determinism and interrupted runs

The same input and seed give the same result under every deployment condition. The engine
runs a request on one thread, with no worker count, environment setting or flag that
changes the outcome: iteration `i` reseeds the random stream from the seed plus `i`, and
the only fields that differ between two runs are the wall time ones, `elapsed_ns` and
`result.elapsedNs`. Where a worker runs requests side by side, each in its own process,
the processes share nothing. `tests/determinism.rs` runs the same fixtures under several
worker counts and orders in one process and in many concurrent processes, whose
environments carry differing pool settings, and requires byte-identical output apart from
the timing fields.

The engine has no timeout or cancellation of its own. A worker bounds a run by ending its
process, which `route.py` does when a step outlives `--timeout`. The contract is about what
the ended process leaves behind:

- `forever-engine sim --outfile PATH` writes the report only after the whole simulation
  succeeded, to a hidden sibling file that it flushes and renames over `PATH`. A run that is
  timed out, cancelled or killed leaves `PATH` absent or as it was, never truncated. A
  killed run can leave only a `.PATH.<pid>.tmp` sibling, which is not a result.
- `route.py` treats a step that times out, is ended by a signal (the fault names it), exits
  with an error or prints an incomplete report as a `fault` at that stage. After a fault of the
  `rust` stage it removes the report and any temporary file, and `result.json` is never
  written. It writes `result.json` and `decision.json` by rename as well. In a batch, a
  fault in any request also removes the results of the requests that had already run.

`tests/interruption.rs` and `tools/route_test.py` check this. A worker that finds a
`result.json` beside a `status` of `rust` can trust that it is complete.
