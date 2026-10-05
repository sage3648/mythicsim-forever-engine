# Upstream tracking

## Reference baseline

| Source | Role | Baseline |
| --- | --- | --- |
| [MythicSim Go engine](https://github.com/sage3648/mythicsim-forever-engine-go) | Fixtures and live reference | `20b551c6bff9aa780fefe17ead89029c13cabcd4`, on community base `f764984d8b05f0d5ce73aab82185fb6efa40a9a4` |
| [Community Forever engine](https://github.com/ElliotWood/Forever) | Changes to review for applicability | Adopted base `f4b776b4f41d5c7799b8141697a2c9e67c89d426`; reviewed through `f764984d8b05f0d5ce73aab82185fb6efa40a9a4` (2026-10-05) |

Go is a reference implementation, not proof of live-game correctness. Forever can
intentionally differ from inherited Classic behavior. Fixture client build:
`1.60.1.70205`.

The pin is written once, in [upstream/sources.json](upstream/sources.json). The Rust
build script, the Python tools and the Go helpers they build (through `-ldflags -X`)
all read it from there. Fixtures, manifests, validation records and benchmark
snapshots keep the revision they were made with as provenance, and the checks
reject accepted fixtures whose revision or client build differs from the pin. The
matched Go kernel in `tools/matched-go` writes its accepted revision as a constant,
since it builds without the pin; the historical benchmarks keep the source digest
they were measured with.

To move the pin:

1. Change `pinned_revision`, `client_build` and, if the fork rebased,
   `community_base` in `upstream/sources.json`.
2. Run `python3 tools/prepared_v2.py repin --output <scratch>`. It re-exports every
   prepared input and Go golden at the new pin and lists each changed golden in
   `<scratch>/repin.json`. Every change is a reference behavior change to review, not
   to accept: port it to Rust until `python3 tools/prepared_v2.py compare` matches every
   case, and run `cargo test`. Re-export the Frost kernel fixtures with
   `tools/compare.py`'s oracle, check they keep their expected values, move the
   matched Go kernel's `revision` and run its `go test ./...`.
3. Re-run every recorded compatibility sweep, update `release/manifest.json`, and
   review the ledger range against the new community base. Mark the applicable changes
   the reference now includes as adopted.

The pin moved from `6823b49eb` to `20b551c6b` on 2026-10-05, the fork's merge of
community #613 to #641 plus its patch 84. 274 Go goldens changed: every aura with an
exclusive effect now reports that effect's uptime, and 28 builds changed DPS through
Mutilate (#632), Whirlwind and the warrior shout costs (#613, #614) and Piercing Ice
(#615). Rust matches all of them, and all 2439 recorded sweep variants
([record](validation/2026-10-05-reference-pin-20b551c6b-sweeps.json)).

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

The first review covered 53 community commits after the adopted base: 48 irrelevant
to Frost scope, 4 deferred client data updates and 1 applicable fix. The pin move to
`20b551c6b` extended it to 55 commits, all now in the reference, and marked the changes
Rust covers by then as applicable and adopted. The first fix,
[#622](https://github.com/ElliotWood/Forever/pull/622) (`252f57aa8`), changes how a
rotation reads an aura the character cannot have. The pinned reference drops such a
condition, so a Frost build without Fingers of Frost casts Ice Lance on every global
cooldown (about 81 casts and 211 DPS per fight, against 593 DPS for the talented
reference). The Arcane preset reads Missile Barrage without a guard, so an Arcane build
without it casts Arcane Missiles whenever the rule is reached (449.7 DPS and no Arcane
Blasts, against 396.6 DPS for the rotation as written). Until the reference adopted the
fix, Rust compiled each condition both ways and rejected the rotations where they acted
differently. Since `20b551c6b` both read a missing aura as inactive, with no stacks and
no time left. The regressions are the `frost-no-fingers`, `arcane-no-missile-barrage`
and `reference-no-missile-barrage` prepared fixtures.

## Reference defects

Bugs in the reference that block a comparison. Rust refuses the affected inputs until the
reference is fixed, since there is no Go result to match.

### Ignite on the Goblin Sapper Charge's hit on the player

The pinned engine panics with a nil pointer dereference at `sim/mage/talents_fire.go:135`
when a Mage with Ignite throws a Goblin Sapper Charge and the charge's hit on the Mage
crits. Community commit `8fb1a2d75a`, in the adopted base, deals that hit through its own
spell with `ProcMaskSpellDamage` and School Fire. The Mage's `OnSpellHitDealt` hears it,
and Ignite's trigger (`ProcMaskSpellDamage`, `OutcomeCrit`, Fire) calls
`mage.Ignite.Dot(result.Target)` with the Mage as the target. Dots exist only on enemies,
so `dot.IsActive()` dereferences nil. The handler is unchanged on community `master` at
`b49be9e13` (2026-10-04).

- Reproduction: run the pinned engine on
  `fixtures/mage/prepared-v2/fire-mage-goblin-sapper.request.json`, the production Fire
  request with `goblinSapper` set. The first charge whose hit on the Mage crits panics,
  inside `Character.newBasicExplosiveSpellConfig` (`sim/core/consumes.go:650`).
- Affected records: the Arcane and Fire Mage `goblinSapper` variants in
  `validation/2026-10-04-production-gear-swaps.json`, recorded as `go_error`.
- Report: not filed yet. A report to the community engine should carry the reproduction
  above and the trace.
- Rust now: the Mage gate refuses an Ignite build whose rotation reaches the charge. This
  also refuses the Frostfire variant, which matched only because its rotation never
  reaches the autocast while the global cooldown is busy, the only time Go casts an
  explosive. Before the guard, Rust rolled the Mage's own crit into an Ignite on the
  target.
- Once Go is fixed and the pin moves: compare the fixed Ignite trigger with Rust. If, as
  expected, Ignite ignores hits on a unit that is not an enemy, make Rust's Ignite trigger
  ignore hits on the player, drop the guard, promote `fire-mage-goblin-sapper` with
  `python3 tools/prepared_v2.py promote` and rerun the affected gear swap variants.

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
