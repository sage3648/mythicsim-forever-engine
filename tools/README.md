# Reference and validation tools

These tools are development helpers. The Rust engine does not call Go during a
fight. Ordinary Rust tests use frozen data and need no Go checkout.

| Tool | Purpose | Requirements |
| --- | --- | --- |
| [compare.py](compare.py) | Compare the Rust kernel with the actual pinned Go engine | Rust, Go, Python, Git, protoc |
| [fair_compare.py](fair_compare.py) | Time equivalent Go/Rust kernels with work checks and a full-engine oracle | Same tools as the comparison |
| [inventory.py](inventory.py) | Audit the first-build inventory offline; optionally recapture and compare Go output | Python for audit; Go and pinned source checkouts for capture |
| [oracle/main.go](oracle/main.go) | Prepare restricted reference cases and run the actual Go engine | Built by the comparison tool in isolated scratch |
| [prepared_v2.py](prepared_v2.py) | Audit prepared v2 fixtures offline; re-export them from the pinned engine into scratch | Python for audit; Go, Git and protoc for capture |
| [census.py](census.py) | Count which prepared inputs or requests Rust would run and rank the reasons blocking the rest | Python and Rust for prepared inputs; Go, Git and protoc to export requests |
| [sweep.py](sweep.py) | Generate seeded randomized variants of one request for a compatibility sweep | Python |
| [upstream.py](upstream.py) | Audit the [upstream ledger](../UPSTREAM.md#ledger), optionally against a community clone | Python; Git for the range check |
| [oracle-v2/main.go](oracle-v2/main.go) | Export a reset Go simulation as [prepared v2](../docs/prepared-v2.md) and run the full Go engine | Built by prepared_v2.py in isolated scratch |
| [matched-go/](matched-go/) | Go implementation of the same narrow Rust kernel for fair timing | Go |
| [reference-capture/](reference-capture/) | Standalone request and observation helper programs | Built in temporary modules by inventory capture |
| [app-timeline/main.go](app-timeline/main.go) | Parse Go and Rust logs with the application's timeline helpers and compare | Go and an application checkout, in a scratch module |

Run the lightweight checks from the repository root:

```sh
python3 -m unittest discover -s tools -p '*_test.py'
python3 tools/inventory.py check
python3 tools/prepared_v2.py check
python3 tools/upstream.py check
```

Survey a set of inputs, such as exported application requests, before routing them:

```sh
python3 tools/census.py --output output/census/report.json REQUESTS_OR_DIRECTORIES...
```

The [kernel guide](../docs/kernel.md), [inventory guide](../docs/first-frost-inventory.md)
and [Go parity report](../docs/go-parity-validation-2026-10-03.md) describe scope and
reproduction commands. Comparisons write scratch results under ignored `output/`
and build the oracle under ignored `oracle-cache/`. They do not update accepted
fixtures, engine pins or production jobs.

New benchmark source digests include every nested Rust source module, Cargo
metadata and the matched Go implementation. Historical digests remain unchanged.
