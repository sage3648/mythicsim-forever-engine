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
| Prepared v2 identity checks and coverage gate | [src/engine/prepared.rs](../src/engine/prepared.rs), [src/classes/mage/prepared/coverage.rs](../src/classes/mage/prepared/coverage.rs) |
| Rotation (APL) subset | [src/rotation.rs](../src/rotation.rs) |
| Strict input limits and supported-build checks | [src/engine/validation.rs](../src/engine/validation.rs) |
| Iteration lifecycle and aggregate statistics | [src/engine.rs](../src/engine.rs) |
| Fight runtime: queue, units, casting, auras, damage, channels, rotation, metrics, logs | [src/core/fight.rs](../src/core/fight.rs), [src/core/fight/](../src/core/fight/) |
| Go pending-action ordering | [src/core/queue.rs](../src/core/queue.rs) |
| Mage runtime hooks | [src/classes/mage/agent.rs](../src/classes/mage/agent.rs) |
| Druid runtime hooks, forms and regressions | [src/classes/druid/agent.rs](../src/classes/druid/agent.rs), [src/classes/druid/forms.rs](../src/classes/druid/forms.rs), [tests/classes/druid.rs](../tests/classes/druid.rs) |
| Warlock runtime hooks and regressions | [src/classes/warlock/agent.rs](../src/classes/warlock/agent.rs), [tests/classes/warlock/](../tests/classes/warlock/) |
| Priest runtime hooks and regressions | [src/classes/priest/agent.rs](../src/classes/priest/agent.rs), [tests/classes/priest.rs](../tests/classes/priest.rs) |
| Rogue runtime hooks and regressions | [src/classes/rogue/agent.rs](../src/classes/rogue/agent.rs), [tests/classes/rogue.rs](../tests/classes/rogue.rs) |
| Fights against several targets: each target's state, area hits, multidot and the gate | `Side::Extra` and `TargetUnit` in [src/core/fight.rs](../src/core/fight.rs), `several_target_limits` in [src/engine/coverage.rs](../src/engine/coverage.rs), [tools/oracle-v2/targets.go](../tools/oracle-v2/targets.go) |
| The targets swinging at a tank, one swing for each copy of the boss | [src/core/fight/enemy.rs](../src/core/fight/enemy.rs), the weapon attack list in [src/core/fight/melee.rs](../src/core/fight/melee.rs), [tools/oracle-v2/enemy.go](../tools/oracle-v2/enemy.go) |
| Energy bar, energy ticks and combo points | [src/core/fight/energy.rs](../src/core/fight/energy.rs) |
| The player taking damage and Chance of Death | [src/core/fight/damage_taken.rs](../src/core/fight/damage_taken.rs) |
| Hunter runtime hooks and regressions | [src/classes/hunter/agent.rs](../src/classes/hunter/agent.rs), [tests/classes/hunter.rs](../tests/classes/hunter.rs) |
| Player melee and ranged auto attacks | [src/core/fight/melee.rs](../src/core/fight/melee.rs) |
| A unit that moves: a prepull move or a move of the rotation, a move for a duration, the Movement aura, the lazy position, the movement speed categories, the ranged auto swing's pause and a weaver's wakeups, and Charge | [src/core/fight/movement.rs](../src/core/fight/movement.rs), the categories in [src/core/fight/exclusive.rs](../src/core/fight/exclusive.rs), [src/classes/warrior/spells/charge.rs](../src/classes/warrior/spells/charge.rs), `player_movement_limits` in [src/engine/coverage.rs](../src/engine/coverage.rs), [tools/oracle-v2/movement.go](../tools/oracle-v2/movement.go) |
| Rotation groups and value variables: the instances Go builds, how references bind them, placeholders and a group's variables | [src/rotation/groups.rs](../src/rotation/groups.rs), `build_groups` in [src/prepare/rotation.rs](../src/prepare/rotation.rs), `flatten_groups` in [src/engine/coverage.rs](../src/engine/coverage.rs) |
| Pets: simulated summons and registered pets nothing summons | [src/core/fight/pet.rs](../src/core/fight/pet.rs) |
| Buffs other players cast on the player on cooldown (the external Power Infusion) and Power Infusion's multipliers | [src/core/fight/external_cooldown.rs](../src/core/fight/external_cooldown.rs), [src/core/fight/power_infusion.rs](../src/core/fight/power_infusion.rs), [src/prepare/power_infusion.rs](../src/prepare/power_infusion.rs), [tools/oracle-v2/power_infusion.go](../tools/oracle-v2/power_infusion.go) |
| Weapon procs of a client row (a chance on hit that casts its damage spell: direct, chain, area or damage over time), Flurry Axe's extra attack and Annihilator's armor debuff | [src/core/fight/proc_damage.rs](../src/core/fight/proc_damage.rs), `weapon_damage_proc_effects` in [src/prepare/export_items.rs](../src/prepare/export_items.rs), [tools/oracle-v2/weapon_procs.go](../tools/oracle-v2/weapon_procs.go) |
| Warlock demon AI and abilities | [src/classes/warlock/pets.rs](../src/classes/warlock/pets.rs) |
| Shared build gate and class gates | [src/engine/coverage.rs](../src/engine/coverage.rs), `src/classes/<class>/prepared/coverage.rs` |
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
      masks.rs                   Go Mage class masks by exported name
      prepared.rs                prepared v2 execution for Mage builds
      prepared/coverage.rs       prepared v2 Mage build gate
      spells.rs                  Mage spell entry: Frostbolt, Arcane Blast, cooldowns
      talents.rs                 Mage talent entry: procs and their auras
      specs.rs                   Mage spec entry
      specs/frost.rs             prepared v1 Frostbolt kernel

tests/
  cli.rs                         executable contract tests
  classes/main.rs                class integration test target
  classes/mage.rs                Mage test entry
  classes/mage/frost.rs           Frost test entry
  classes/mage/frost/kernel.rs    timing, mana and rejection regressions
  classes/mage/frost/oracle.rs    immutable Go golden comparison
  classes/mage/prepared_v2.rs    prepared v2 contract, coverage and Go golden tests
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

### Match Go's fused multiply-adds

The pinned reference runs as an arm64 build, and Go's arm64 compiler fuses a
floating point add or subtract with a product that feeds it into one `FMADD`,
`FMSUB`, `FNMSUB` or `FNMADD` instruction, which rounds once. The rewrite works
on SSA values, so it reaches across statements, struct fields and inlined calls:
`a := x * y` followed by `b := a + z` fuses, and so does `base + Roll(...)`
when `Roll` is inlined. Only an explicit `float64(...)` conversion or an
out-of-line call stops it. A Rust formula that rounds the product first agrees
almost always and then differs in the last bit of one value, which surfaces
rarely, as a 0.001 log difference or a flipped comparison.

Do not guess from the source. Run

```sh
python3 tools/fma_scan.py
```

after `tools/prepared_v2.py` has built `oracle-cache/forever-go-oracle-v2`. It
lists every fused instruction of the simulation packages and the exporter by
function and Go source line. Where a ported formula appears there, write it with
`mul_add` in the same shape: Go's `a + x*y` is `x.mul_add(y, a)`, `a - x*y` is
`(-x).mul_add(y, a)` and `x*y - a` is `x.mul_add(y, -a)`. When the line holds
several operations, read the instruction operands with
`go tool objdump -s '<function regex>' oracle-cache/forever-go-oracle-v2` to see
which product feeds which add. The shared helpers follow the binary:
`Fight::go_roll` is `Simulation.Roll`, `Fight::effect_roll` is spelldata
`Effect.Roll`, which needs the row's average and variance rather than its bounds,
and `threat_of`, `calc_damage`, `snapshot_dot`, the weapon rolls, the resist
tables and the metric aggregates already fuse where Go does. Two forms need no
`mul_add`: `x * 2` compiles to `x + x`, and its fused form equals the doubled
rounded product, and an exact product, such as an integer times an integer or
anything times 1, rounds the same either way. The
[fused multiply-add audit](../validation/2026-10-04-fma-audit.json) records every
site the binary fuses and what the runtime does with it.

Production runs the Forever engine on the same architecture: a linux/arm64 image on the
production Mac, the only worker that polls the Forever queue. `python3
tools/architecture_check.py --goarch arm64 --image IMAGE --output <scratch>` rebuilds the
pinned engine for a Linux architecture, runs every accepted case in that image and
compares it with the goldens. On linux/arm64 every result and log matches; on
linux/amd64, where Go fuses nothing, every result agrees within the comparison tolerance
but some first-fight logs differ in the last printed digit, so a result Go made on amd64
is not an exact reference
([arm64 record](../validation/2026-10-05-architecture-linux-arm64.json),
[amd64 record](../validation/2026-10-05-architecture-linux-amd64.json)).

### Several targets

Go creates every copy of the boss as its own unit, and Rust follows: each target holds a copy
of the first target's auras at the same positions, its own dots, armor, resistances and
damage taken modifiers, and its own metrics. Class code acts on the target it was given: a
spell's `target`, or a hit's `result.target`. `Fight::dot_on` finds a dot's copy on a target
and `Fight::aura_on` an aura's, so a debuff or dot lands where Go puts it. An area spell hits
`Fight::target_sides` in unit index order and a cleave follows `Fight::next_target`, keeping
Go's random draw order: calculate and deal each hit in turn, or calculate every hit before
dealing any, exactly as the Go helper it ports does.

A class opts in through its gate's `several_targets` hook, which lists the reachable spells
that reach another target in Go but not yet in Rust. Go reaches other targets only through
its area and cleave helpers, its loops over the encounter's targets and the rotation's target
choices, so `grep` the class's Go package for `ActiveTargetUnits`, `AllTargetUnits`,
`NextActiveTarget`, `Aoe` and `Cleave` before opening it. Then compare the class at 2 to 5
targets, for example with requests from `tools/reference-capture/builds -targets N`.

When the player tanks, Go sets every copy's target to the tank, so each copy swings at the
player on its own timer: `Fight::enemies` holds each target's swing at its position, with its
own metrics and melee speed, and the weapon attack list runs the swings in unit index order.
A hit the tank takes names the copy that swung in `SpellResult::attacker`, Go's `spell.Unit`.
Reactive code answers that unit and never `Side::Target`: Holy Shield's damage, Eye for an
Eye's reflection, Retaliation's strike, a struck item proc and Sulfuras' Immolation. A class
opts in through its gate's `tanks_several_targets`, once its listeners of hits taken do that
and a tank of the class at 2 to 5 targets matches Go; a debuff on a copy, such as Thunder
Clap's slow or Demoralizing Roar's attack power cut, is read from that copy's aura.

A rotation's `castSpell` may name a target past the first. A class opts in through its gate's
`other_target_casts` hook, which lists what its spells do not yet land on the target they are
cast at, as the Warlock's Demonic Brand; `None` refuses every such cast. Check a class with
`python3 tools/rotation_forms.py casts --output <scratch>`, which aims every cast of each
production rotation at another target, and compare the requests with `tools/prepared_v2.py
compare`: a debuff or aura that lands on the first target shows as a difference.

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
