# MythicSim Forever Engine

A community Rust simulation engine for WoW Forever. It reproduces the Go engine
that MythicSim uses in production, exactly, and runs faster.

**Status: experimental.** All 29 production builds run in Rust and give the same
result as the pinned Go engine. Production still uses Go. Routing real jobs to Rust,
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

1. Go prepares the character. The exporter in `tools/oracle-v2` writes the gear,
   stats, auras, spells and rotation as a [prepared v2](docs/prepared-v2.md) file.
2. The Rust coverage gate reads the file. It refuses any input that it cannot
   simulate exactly, and gives the reasons.
3. Rust runs the fight. Event order, random numbers and float math follow Go, so the
   result and the combat log are the same.

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

## Evidence

- 255 accepted fixtures with Go results, checked by `cargo test`.
- About 7,900 production variants and 3,500 random variants compared with Go.
  [docs/validation-summary.md](docs/validation-summary.md) lists every record.
- Rust is faster on every production build: a median of 1.47x at 10,000 iterations
  ([snapshot](benchmarks/2026-10-04-production-builds.json)).
- Earlier timings came from the MythicSim prototype. See the
  [fair comparison](docs/forever-rust-fair-comparison-2026-10-03.md) and the
  [prototype report](docs/forever-rust-prototype-2026-10-03.md).

The reference is Go revision `6823b49eb8aff741f197ef36d83766ef6a218285`. A match
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

Pick an item from the [issue board](https://github.com/sage3648/mythicsim-forever-engine/issues).
Items are grouped by milestone and ordered by priority. Comment on an issue to claim
it before you start.

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
