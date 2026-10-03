# Go parity validation, 2026-10-03

The saved first-Frost reference reproduces against fresh pinned Go execution.
The current Rust kernel also matches Go for every supported comparison scenario.
These are separate correctness checks; Rust cannot execute the full Frost build yet.

The [recorded evidence](../validation/2026-10-03-go-parity.json) identifies the
application and engine commits, compiler versions, per-case results and limits.
Accepted fixtures and historical benchmark files were not regenerated.

| Check | Result |
| --- | --- |
| Full Frost reference, 3,000 fights, seed 42 | All 902 saved fields matched fresh Go output, including 804 numeric fields |
| Frozen observation versus fresh observation | Byte-identical |
| Rust kernel versus actual Go engine | 33/33 comparisons passed: 11 scenarios across seeds 42, 173 and 9001 |
| Iterations per kernel comparison | 3,000 per engine, 99,000 fights per engine overall |
| Rust receiving the complete Go Frost request | Rejected with nonzero exit and no report written |

The full reference's mean DPS was `590.9470407845581` in both saved and fresh
output. Compared fields include headline statistics, exercised actions, aura
uptime/procs, resources and first-fight timeline values. Unused registered spells
and the averaged timeline are outside this saved observation.

The kernel comparison checks exact source/seed/iteration identities and outcome
counts, then DPS mean, DPS standard deviation and mana delta within `1e-8`.
The largest observed mean-DPS difference was approximately `6.82e-13`.
Coverage includes static talents, hit cap, resistance, mana starvation, Spirit
regen, target level, haste, weapon stats and projectile fight-end boundaries.
It excludes dynamic procs, Ice Lance, Arcane Missiles and full-build preparation.

The capture tool now compares saved metrics as well as action identities, failing
with JSON field paths when an output changes. Integer counts are exact; floating
fields allow `1e-8` absolute or `1e-12` relative roundoff. Regression tests verify
that changed counts, damage, uptime, resource flows, timeline values and missing
fields fail. The Python suite has 20 passing tests.

## Reproduce

Use source checkouts at the inventory's exact pins and a fresh output directory:

```sh
python3 tools/inventory.py capture \
  --app-source /absolute/path/to/pinned-application \
  --engine-source /absolute/path/to/pinned-go-engine \
  --output output/full-frost-go-check

python3 tools/compare.py \
  --source /absolute/path/to/go-engine \
  --iterations 3000 --seeds 42 173 9001 --repeats 1 \
  --output output/go-parity

python3 -m unittest discover -s tools -p '*_test.py'
```

The existing kernel harness also records timings. They were excluded from this
validation artifact because preparation and supported scope differ. No performance
conclusion is drawn. Go's inherited mechanics caveats remain unchanged.
