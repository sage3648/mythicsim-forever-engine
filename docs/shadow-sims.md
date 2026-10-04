# Shadow sims

Shadow sims compare Rust with the pinned Go engine on real MythicSim
traffic, without users seeing them. The application samples finished
Forever Quick Sims, and a shadow worker reruns each request with both
engines. The application side is documented in the MythicSim repository at
`docs/forever-shadow-sims.md`.

This repository provides `tools/shadow.py`.

## Build a bundle

```sh
python3 tools/shadow.py build --output ~/forever-shadow/bundle-<short-sha>
```

A bundle is a folder with the release Rust engine, the Go exporter built at
the reference pin, the comparison tools and `manifest.json`, which records the
Rust revision, the pin, the client build and the exporter digest. The shadow
worker runs it with Python 3 only.

## Compare one request

```sh
python3 tools/shadow.py run --request REQUEST.json --output /tmp/shadow-run --seed 7
```

The run prints one JSON verdict and writes it, with the prepared input and
both results, into the output folder. An unseeded request gets `--seed`, so
both engines draw the same numbers.

| Status | Meaning |
| --- | --- |
| `match` | Every result field and the first-fight log equal the pinned Go engine |
| `mismatch` | `differences` and `first_log_difference` show what differs |
| `refused` | The coverage gate declined the input; `reasons` says why |
| `error` | A step failed; `stage` and `error` say where |

`timings_ms` holds the wall time of the prepare, Go and Rust steps, process
start included. `speedup` is the Go time over the Rust time.

## From a mismatch to a fix

Take the request from the shadow run's output folder and accept it as a
fixture with `python3 tools/prepared_v2.py accept`. Then fix Rust until the
fixture matches, as for any other mechanics change.
