# Upstream tracking

## Reference baseline

| Source | Role | Baseline |
| --- | --- | --- |
| [MythicSim Go engine](https://github.com/sage3648/mythicsim-forever-engine) | Fixtures and live reference | `6823b49eb8aff741f197ef36d83766ef6a218285` |
| [Community Forever engine](https://github.com/ElliotWood/Forever) | Changes to review for applicability | No independent reconciliation baseline yet |

Go is a reference implementation, not proof of live-game correctness. Forever can
intentionally differ from inherited Classic behavior. Fixture client build:
`1.60.1.70170`.

The pin is encoded in Rust, comparison tools, the matched Go kernel and fixture
metadata. Update these deliberately together. Historical benchmark snapshots
retain their original source revision and source digest.

## Reconcile a fix

1. Identify the source repository and exact commit or issue.
2. Classify it as applicable, already covered, irrelevant to supported scope,
   deferred, or intentionally different for Forever.
3. Capture applicable bugs in regressions and port the behavior into Rust.
   Go preparation fixes and Rust runtime fixes may affect different layers.
4. Compare against the corrected reference and relevant game evidence.
5. Record the disposition in an issue or PR with the source link. Advance reviewed
   baselines only when the intervening range is reconciled.

Merging Go source does not update Rust mechanics. Compatible data can be imported;
code changes need translation and review. Shared formats reduce but do not eliminate
that work.

There is no automatic upstream monitor or synchronization job yet. The fixture pin
does not claim that every upstream change has been reviewed. Tracking reviewed
ranges and dispositions is part of the next milestone.

## Reproduce the reference comparison

Requires stable Rust, Go 1.25.6 or later, Python 3, Git and protoc:

```sh
python3 tools/compare.py
python3 tools/fair_compare.py
```

The tools clone pinned Go source into ignored scratch storage. Use
`--source /absolute/path` for a local repository containing the commit. Neither
command modifies production or the supplied source checkout.
