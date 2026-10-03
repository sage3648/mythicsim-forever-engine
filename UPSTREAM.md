# Upstream tracking

## Reference baseline

| Source | Role | Baseline |
| --- | --- | --- |
| [MythicSim Go engine](https://github.com/sage3648/mythicsim-forever-engine-go) | Fixtures and live reference | `6823b49eb8aff741f197ef36d83766ef6a218285` |
| [Community Forever engine](https://github.com/ElliotWood/Forever) | Changes to review for applicability | Adopted base `f4b776b4f41d5c7799b8141697a2c9e67c89d426`; reviewed through `017ff78fa6b1aeb9f839c9729628e01c701c5c08` (2026-10-03) |

Go is a reference implementation, not proof of live-game correctness. Forever can
intentionally differ from inherited Classic behavior. Fixture client build:
`1.60.1.70170`.

The pin is written once, in [upstream/sources.json](upstream/sources.json). The Rust
build script, the Python tools and the Go helpers they build (through `-ldflags -X`)
all read it from there. Fixtures, manifests, validation records and benchmark
snapshots keep the revision they were made with as provenance, and the checks
reject accepted fixtures whose revision or client build differs from the pin. The
matched Go kernel behind the historical benchmarks keeps its own revision, because
its source digest is part of that benchmark's record.

To move the pin:

1. Change `pinned_revision`, `client_build` and, if the fork rebased,
   `community_base` in `upstream/sources.json`.
2. Run `python3 tools/prepared_v2.py refresh` and `python3 tools/compare.py`. Every
   Go golden that changes is a reference behavior change to review, not to accept.
3. Re-run the [compatibility sweep](validation/2026-10-03-frost-sweep.json) and
   review the ledger range against the new community base.

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
reference). The Arcane preset reads Missile Barrage without a guard, so an Arcane build
without it casts Arcane Missiles whenever the rule is reached (449.7 DPS and no Arcane
Blasts, against 396.6 DPS for the rotation as written). Rust compiles each condition
both ways. Where they act differently, it rejects the rotation with both behaviors
named until the reference adopts the fix; where they act the same, as when an
`auraIsKnown` guard prunes the action either way or a missing stack count leaves a
constant comparison, it runs. The regressions are the `frost-no-fingers`,
`arcane-no-missile-barrage` and `reference-no-missile-barrage` prepared fixtures.

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
