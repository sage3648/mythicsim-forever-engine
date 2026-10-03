# Contributor code map

Start with [README](../README.md) for supported scope and [CONTRIBUTING](../CONTRIBUTING.md)
for checks and evidence requirements. This map explains where behavior belongs.

The project runs prepared v2 fights through a class-independent runtime in
`core/fight` that mirrors Go's sim/core, with Mage behavior plugged in through
`classes/mage/agent.rs`. Supported builds currently cast Frostbolt only. The
[Frost inventory](first-frost-inventory.md) and the reference build's coverage report
identify the mechanics still needed for the first complete build. The prepared v1
Frostbolt kernel remains unchanged beside it.

## Find the code

| Change | Start here |
| --- | --- |
| Library entry points and compatibility | [src/lib.rs](../src/lib.rs) |
| CLI arguments and JSON files | [src/main.rs](../src/main.rs) |
| Prepared input fields | [src/contracts.rs](../src/contracts.rs), [src/contracts/prepared_v2.rs](../src/contracts/prepared_v2.rs) |
| Prepared v2 identity checks and coverage gate | [src/engine/prepared.rs](../src/engine/prepared.rs), [src/classes/mage/specs/frost/coverage.rs](../src/classes/mage/specs/frost/coverage.rs) |
| Rotation (APL) subset | [src/rotation.rs](../src/rotation.rs) |
| Strict input limits and supported-build checks | [src/engine/validation.rs](../src/engine/validation.rs) |
| Iteration lifecycle and aggregate statistics | [src/engine.rs](../src/engine.rs) |
| Fight runtime: queue, units, casting, auras, damage, channels, rotation, metrics, logs | [src/core/fight.rs](../src/core/fight.rs), [src/core/fight/](../src/core/fight/) |
| Go pending-action ordering | [src/core/queue.rs](../src/core/queue.rs) |
| Mage runtime hooks | [src/classes/mage/agent.rs](../src/classes/mage/agent.rs) |
| Event ordering (prepared v1 kernel) | [src/core/events.rs](../src/core/events.rs) |
| Seeded random streams | [src/core/rng.rs](../src/core/rng.rs) |
| Simulation time units | [src/core/time.rs](../src/core/time.rs) |
| Shared binary hit-table math | [src/mechanics/damage.rs](../src/mechanics/damage.rs) |
| Mana ticks, spending and five-second rule | [src/mechanics/mana.rs](../src/mechanics/mana.rs) |
| Report fields, counters and trace records | [src/report.rs](../src/report.rs) |
| Mage spell mechanics | [src/classes/mage/spells/](../src/classes/mage/spells/) |
| Current Frost build scope and event decisions | [src/classes/mage/specs/frost.rs](../src/classes/mage/specs/frost.rs) |
| Frost regressions and Go goldens | [tests/classes/mage/frost/](../tests/classes/mage/frost/) |
| Go oracle, comparisons and capture commands | [tools/README.md](../tools/README.md) |

```text
src/
  lib.rs                         public API and reference identity
  main.rs                        executable entry point
  contracts.rs                   strict prepared v1 input types
  contracts/prepared_v2.rs       strict prepared v2 input types
  engine.rs                      validation and iteration orchestration
  engine/validation.rs           prepared v1 input validation
  engine/prepared.rs             prepared v2 identity checks and coverage entry
  rotation.rs                    strict APL subset parser
  core.rs                        shared scheduler/RNG/runtime module entry
  core/{events,queue,rng,time}.rs scheduling, random streams, time
  core/fight.rs                  class-independent fight runtime and Agent hooks
  core/fight/                    auras, casting, damage, dots, rotation, metrics, logs
  mechanics.rs                   reusable combat module entry
  mechanics/{damage,mana}.rs      shared combat primitives
  report.rs                      prototype report types
  classes.rs                     class domain entry
  classes/
    mage.rs                      Mage domain entry
    mage/
      agent.rs                   Mage hooks for the fight runtime
      spells.rs                  shared Mage spell entry
      spells/frostbolt.rs        Frostbolt calculation and outcome recording
      specs.rs                   Mage spec entry
      specs/frost.rs             current prepared Frost execution
      specs/frost/coverage.rs    prepared v2 Frost build gate

tests/
  cli.rs                         executable contract tests
  classes/main.rs                class integration test target
  classes/mage.rs                Mage test entry
  classes/mage/frost.rs           Frost test entry
  classes/mage/frost/kernel.rs    timing, mana and rejection regressions
  classes/mage/frost/oracle.rs    immutable Go golden comparison
  classes/mage/frost/prepared_v2.rs  prepared v2 contract and coverage tests
  release.rs                     release manifest consistency
```

Each module entry uses a descriptive filename, such as `mage.rs`, with child
files inside `mage/`. Use the same convention for new source domains. Cargo
integration test targets with multiple files use a directory containing `main.rs`.
This follows the [Cargo package layout](https://doc.rust-lang.org/cargo/guide/project-layout.html)
and the [Rust module guide](https://doc.rust-lang.org/book/ch07-05-separating-modules-into-different-files.html).

## Ownership and dependency rules

One crate keeps builds and contributions straightforward. Modules provide the
domain boundaries. Add a separate crate only when an actual dependency, reusable
library or build requirement justifies it.

`core` owns scheduling, time, random streams and the fight runtime. The runtime is
generic over an `Agent`: classes supply spell effects and aura callbacks, and the
runtime never names a class. `mechanics` owns reusable combat formulas. Neither
layer imports `classes` or `engine`. Mirror Go's event order, random draw order and
floating-point operation order: with a shared random stream every later draw depends
on them.

Classes consume the shared primitives. A class owns its spell and talent behavior;
a spec owns build scope, composition and spec decisions. The engine selects the
supported spec, validates the request and aggregates iterations. Today that
selection is explicitly the single Frostbolt slice; broader dispatch belongs to
the prepared-v2 work.

The prepared contracts and report types are data boundaries. Validation combines
wire limits with the spec's coverage gate in `engine/validation.rs`. Keep internal
implementation modules private, using `pub(crate)` only where another module
needs access. `lib.rs` re-exports the supported API so moving a file does not force
CLI or external callers to change their imports.

## Class and spec organization

Use `src/classes/<class>/spells/<spell>.rs` for a spell reusable across that class.
Frostbolt belongs to Mage spells even when a future Fire or hybrid Mage build casts
it. Do not duplicate its implementation under every spec or make Fire import
Frost's execution module to cast it.

Use `src/classes/<class>/specs/<spec>.rs` for the spec's supported build and
composition. When that file grows, put its internal modules in
`src/classes/<class>/specs/<spec>/`. Class-wide talent trees can similarly grow
under `src/classes/<class>/talents/`. A talent tree and a simulation spec are
different concepts, particularly for hybrid builds.

For example, a future Warrior contribution would introduce `classes/warrior.rs`,
its shared `warrior/spells/`, and a spec such as `warrior/specs/protection.rs`.
Its integration tests would live in `tests/classes/warrior/protection/`, reached
through the class test entry files. These are placement examples, not implemented
or accepted capabilities.

Introduce modules when implementing real behavior. Do not create empty class
registries, placeholder spell handlers or successful no-op specs. A directory or
module name does not establish simulation coverage.

## Add a mechanic

1. Identify the exact spell, talent or effect IDs, source revision and evidence.
   Read the relevant [inventory](first-frost-inventory.md), [upstream policy](../UPSTREAM.md)
   and the mechanic's entry in [upstream/mechanics-map.json](../upstream/mechanics-map.json).
2. Put reusable combat math in `mechanics`, class behavior in that class, and spec
   decisions in its `specs` module. Preserve deterministic trigger/event ordering.
3. Add a focused regression near the owning domain. Keep reusable primitive tests
   beside the primitive and complete spec comparisons under `tests/classes/`.
4. Extend explicit capability validation only when the complete requested behavior
   is implemented. Unknown input must still fail before simulation.
5. Run the contribution checks and the relevant pinned Go comparison. Explain
   intentional corrections rather than changing old goldens to hide a discrepancy.

To inspect every internal module in generated Rust documentation:

```sh
cargo doc --locked --no-deps --document-private-items --open
```

## Reference files and future areas

The existing flat `fixtures/` files and their manifest are historical, immutable
kernel evidence. See [fixtures/README.md](../fixtures/README.md). A new versioned
fixture family can use `fixtures/<class>/<spec>/<schema-or-reference>/`; moving or
renaming the accepted v1 files is unnecessary for this source refactor.

`inventory/` describes requirements and frozen inputs. `validation/` records
correctness runs. `benchmarks/` retains performance evidence. They are distinct
from runtime support and should not be used as a registry of implemented classes.

Dynamic auras/cooldowns, data ingestion, Rust preparation and upstream change
tracking are planned. Add `data`, `prepare` and `upstream` modules or directories
when those pieces are implemented. `rotation.rs` currently parses the APL subset
used by the Frost preset. The current
Frost loop remains deliberately limited to static prepared casting.

The [layout validation](../validation/2026-10-03-domain-layout.json) records 33
passing pinned-Go comparisons and exact old/new Rust report matches apart from
elapsed runtime. Accepted fixture JSON, prior validation and benchmark snapshots
were preserved.
