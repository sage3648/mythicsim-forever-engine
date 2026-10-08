# MythicSim Forever Engine

A community Rust simulation engine for WoW Forever. 

**Status: experimental.** All 29 production builds run in Rust and give the same
result as the pinned comparison go engine. Production still uses Go. Routing real jobs to Rust,
with Go as the fallback, is the next step.

## Quick start

Install stable Rust, then run:

```sh
git clone https://github.com/sage3648/mythicsim-forever-engine.git
cd mythicsim-forever-engine
cargo test --locked
cargo run --locked --release -- check --infile fixtures/mage/prepared-v2/frost-reference.prepared.json
cargo run --locked --release -- sim --infile fixtures/mage/prepared-v2/frost-reference.prepared.json
```

`check` lists anything in an input that Rust does not support. `sim` runs the fight
and prints the same JSON result that Go prints. You do not need Go to build or test
the engine.

## How it works

1. Rust prepares the character from the application's request: the gear, stats, auras,
   spells and rotation, as the [prepared v2](docs/prepared-v2.md) state the pinned Go
   exporter in `tools/oracle-v2` writes ([Rust preparation](docs/rust-preparation.md)).
   A request it does not cover yet is refused with a code, and the Go exporter prepares it.
2. The Rust coverage gate reads the file. It refuses any input that it cannot
   simulate exactly, and gives the reasons.
3. Rust runs the fight. Event order, random numbers and float math follow Go, so the
   result and the combat log are the same.

[`tools/route.py`](docs/routing.md) does all three for the application worker: it runs a
request in Rust, or reports a fallback to Go with each refusal's stable code, or a fault.

## What works

| Class | Production builds |
| --- | --- |
| Mage | Arcane, Fire, Frost, Frostfire |
| Druid | Balance, Feral (cat), Feral (bear) |
| Hunter | Beast Mastery, Marksmanship, Survival |
| Paladin | Retribution, Shockadin, Protection, Retribution/Protection, Holy/Protection |
| Priest | Shadow, Smite |
| Rogue | Assassination, Combat, Subtlety |
| Shaman | Elemental, Enhancement |
| Warlock | Affliction, Demonology, Destruction |
| Warrior | Arms, Fury, Fury/Protection, Protection |

Many variants of these builds work too, for example other races, gear, talents,
buffs, presets, trinkets, pets and tanking.

Builds of every class also work against 2 to 5 copies of the boss, the application's Advanced targets setting, with the AoE lines the application adds:
each target keeps its own auras, dots, debuffs and metrics, and area hits, cleaves and
multidots reach every target as in Go. Paladin, Druid and Warrior tank builds work there too, with every
copy of the boss swinging at the tank on its own timer; tanks of other classes still fall back to Go.

## Evidence

- 439 accepted prepared v2 fixtures, 437 of them with Go results, checked by `cargo test`.
  Rust prepares every one of their requests itself, and each prepared state equals the Go
  exporter's exactly.
- 7,922 production variants (race boards, builder starters, gear, buffs, talents,
  class options and presets) and 3,724 random variants compared with Go. None
  differ. Rust refuses 174 of them. The 2 that crashed the Go engine before
  `74127c6c8` now match
  ([record](validation/2026-10-06-reference-pin-74127c6c8-sweeps.json)).
- At `cd7d44aec` every recorded sweep ran again, 9,076 variants in 107 records: 9,052 match
  Go, Rust refuses 24 and none differ
  ([record](validation/2026-10-07-reference-pin-cd7d44aec-sweeps.json)).
- At `2d93e423e` the recorded sweeps ran again, 8,120 of the 9,076 variants: all match Go,
  the 24 tank builds refused before among them; 956 Warlock multi-target variants were not
  compared ([record](validation/2026-10-07-reference-pin-2d93e423e-sweeps.json)).
- Against 2 to 5 targets, 267 reference builds of every class, 1,955 Mage and Warlock
  census variants, 272 tank fixture requests and 2,492 random variants match Go, the area
  hits, cleaves, multidots and every copy of the boss swinging at a tank included.
- The latest full rerun covered 24,173 inputs in 121 records, with no mismatches.
- [Shadow sims](docs/shadow-sims.md) compare Rust with Go on real MythicSim traffic,
  without users seeing them.
  [docs/validation-summary.md](docs/validation-summary.md) lists every record.
- Rust is faster on all 27 benchmarked production builds: 1.30x to 1.98x, with a
  median of 1.47x at 10,000 iterations
  ([snapshot](benchmarks/2026-10-04-production-builds.json)).
- Earlier timings came from the MythicSim prototype. See the
  [fair comparison](docs/forever-rust-fair-comparison-2026-10-03.md) and the
  [prototype report](docs/forever-rust-prototype-2026-10-03.md).

The reference is Go revision `2d93e423e0303e93dbb16e190d435503248b68f9`. A match
proves the same behavior as that Go engine. It does not prove the live game.
[UPSTREAM.md](UPSTREAM.md) explains how community fixes are reviewed and ported.

## Repository layout

| Path | Contents |
| --- | --- |
| `src/core`, `src/mechanics` | Shared fight runtime and combat rules |
| `src/classes/<class>` | Class spells and talents, with specs under `specs/` |
| `src/engine` | Coverage gate and orchestration |
| `src/contracts` | Input and output types |
| `tests/classes` | Tests, one folder per class |
| `fixtures` | Prepared inputs and Go results |
| `tools` | Exporter, comparison and validation tools |
| `validation`, `benchmarks` | Comparison and timing records |

The [contributor code map](docs/contributor-guide.md) shows where each mechanic lives.

## Contribute

Start with the pinned [contributor board](https://github.com/sage3648/mythicsim-forever-engine/issues/25).
It lists open work in order, grouped by milestone. Comment on an issue to claim it
before you start. Items marked `good first issue` are a good start.

Read [CONTRIBUTING.md](CONTRIBUTING.md) for the checks to run. Every mechanics
change needs a fixture or test that shows it matches Go. The [roadmap](ROADMAP.md)
shows the larger plan.

## License and acknowledgments

MIT licensed, with new project work credited to MythicSim contributors.
Upstream wowsims notices are retained for adapted portions in [LICENSE](LICENSE).
RNG behavior, combat semantics and reference tooling derive from
[wowsims Forever](https://github.com/ElliotWood/Forever) and
[MythicSim's Go fork](https://github.com/sage3648/mythicsim-forever-engine-go).
See [NOTICE.md](NOTICE.md) for provenance. This project is not affiliated with
Blizzard Entertainment or the WoW Forever server team.
