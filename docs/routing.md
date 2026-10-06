# Routing requests to Rust

The application worker runs engines as subprocesses. `tools/route.py` is the one
command it calls to run a request in Rust: it takes a `RaidSimRequest`, prepares it
with the pinned Go exporter, asks the Rust coverage gate whether it supports the
input, and either runs Rust or tells the worker to use Go. It runs from a bundle that
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
| `fault` | A step failed. `stage` is `request`, `prepare`, `check` or `rust`, and `error` says what failed | 2 | Record a fault; the user can still be served by Go, but a fault is never counted as a fallback |

A fault is anything that is not a decision: an unreadable request, a step that exits with
an error or outlives its timeout, gate output that cannot be read, a refusal without a
reason, or a Rust result without its metrics. A fault never writes `result.json`, so a
partial simulation is never returned.

Every decision also carries `timings_ms` for the steps that ran, `seed` and
`seed_source` (`request`, `argument` or `drawn`), and the bundle's `manifest.json`
when the bundle has one. The refusal codes are listed in the
[prepared v2 contract](prepared-v2.md#unknown-and-unsupported-input).

A request keeps its own seed. One without a seed gets a fresh random one, as Go would
draw, so results stay as varied as Go's; the decision records it, so the run can be
repeated exactly.

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
