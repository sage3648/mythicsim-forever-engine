# Upstream tracking

## Reference baseline

| Source | Role | Baseline |
| --- | --- | --- |
| [MythicSim Go engine](https://github.com/sage3648/mythicsim-forever-engine-go) | Fixtures and live reference | `6823b49eb8aff741f197ef36d83766ef6a218285` |
| [Community Forever engine](https://github.com/ElliotWood/Forever) | Changes to review for applicability | Adopted base `f4b776b4f41d5c7799b8141697a2c9e67c89d426`; reviewed through `017ff78fa6b1aeb9f839c9729628e01c701c5c08` (2026-10-03) |

Go is a reference implementation, not proof of live-game correctness. Forever can
intentionally differ from inherited Classic behavior. Fixture client build:
`1.60.1.70170`.

The pin is encoded in Rust, comparison tools, the matched Go kernel and fixture
metadata. Update these deliberately together. Historical benchmark snapshots
retain their original source revision and source digest.

## Ledger

The [upstream/](upstream/) directory records what has been reviewed and how:

| File | Content |
| --- | --- |
| [sources.json](upstream/sources.json) | Reference pin, adopted community base, last seen and last reviewed community revisions, and the fork's patch register |
| [changes.json](upstream/changes.json) | Every community commit in the reviewed range with a disposition and reason; applicable changes carry adoption state and regressions |
| [mechanics-map.json](upstream/mechanics-map.json) | Frost-scope mechanics: Go files, Rust modules, tests, port status, linked community changes and fork patch numbers |

Last seen, last reviewed and adopted are separate. Reviewing a change does not adopt
it: a deferred or pending change to a covered mechanic stays visible and Rust must
either implement it or reject the affected inputs. Audit the ledger offline, and
against a local clone of the community repository to prove the range is complete:

```sh
python3 tools/upstream.py check
python3 tools/upstream.py check --community /absolute/path/to/Forever
```

The first review covers 53 community commits after the adopted base: 48 irrelevant
to Frost scope, 4 deferred client data updates and 1 applicable fix. That fix,
[#622](https://github.com/ElliotWood/Forever/pull/622) (`252f57aa8`), changes how a
rotation reads an aura the character cannot have. The pinned reference drops such a
condition, so a Frost build without Fingers of Frost casts Ice Lance on every global
cooldown (about 81 casts and 211 DPS per fight, against 593 DPS for the talented
reference). Rust compiles each condition both ways. Where the results differ, it
rejects the rotation with both behaviors named until the reference adopts the fix;
where they agree, as when an `auraIsKnown` guard prunes the action either way, it runs.
The regressions are the `frost-no-fingers` and `reference-no-missile-barrage` prepared
fixtures.

## Reconcile a fix

The [migration plan](docs/hybrid-migration-plan.md#community-fix-workflow-with-ai-assistance)
defines the AI-assisted port workflow. Record each review in the ledger above.

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

There is no automatic upstream monitor or synchronization job yet. The ledger's
reviewed range, not the fixture pin, states which community changes were reviewed.

## Reproduce the reference comparison

Requires stable Rust, Go 1.25.6 or later, Python 3, Git and protoc:

```sh
python3 tools/compare.py
python3 tools/fair_compare.py
```

The tools clone pinned Go source into ignored scratch storage. Use
`--source /absolute/path` for a local repository containing the commit. Neither
command modifies production or the supplied source checkout.
