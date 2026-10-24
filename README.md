# MythicSim Forever Engine

A Rust simulation engine for WoW Forever. [MythicSim](https://mythicsim.com/wow-forever/sim)
runs Forever Quick Sims on it, and the Go engine for the rest; [Coverage](#coverage) lists
which is which.

The engine began as an exact port of [MythicSim's Go fork](https://github.com/sage3648/mythicsim-forever-engine-go)
of [wowsims Forever](https://github.com/ElliotWood/Forever): event order, random numbers and
float math match, so a sim the Go engine covers gives the same result and the same combat log
in either engine. Rust is 1.3x to 2x faster. Since October 2026 new Forever work happens here
only: the Go fork no longer takes patches and its frozen goldens guard what it covered, and
mechanics Go never had, starting with the healer sims, are built and checked in Rust alone
(see [Rust-only mechanics](#rust-only-mechanics)).

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

1. Rust prepares the character from the application's request: gear, stats, auras,
   spells and rotation ([prepared v2](docs/prepared-v2.md),
   [Rust preparation](docs/rust-preparation.md)).
2. A coverage gate checks the prepared input and refuses anything Rust cannot simulate
   exactly, with a reason for each refusal.
3. Rust runs the fight.

[`tools/route.py`](docs/routing.md) is what the MythicSim worker calls. It runs a request in
Rust, or tells the worker to run it in Go and says why.

## Coverage

MythicSim's Forever Quick Sims run in Rust, for every class and spec it offers: the Druid,
Hunter, Mage, Paladin, Priest, Rogue, Shaman, Warlock and Warrior damage and tank specs, with
their talents, gear, buffs, consumables, pets, tanking and 2 to 5 targets. Holy Paladin heal
sims match Go too, under the reference's rules: heals cast on the player and counted in full, as
Go counts them. Production prepares them with Forever's rules, which leave out overhealing.

These still run in Go:

- Quick Sims that ask for stat weights.
- Best Gear, trinket boards, race boards and the Build Lab.
- The one fight a Quick Sim's timeline and combat log show. It is replayed in Go, which gives
  the same fight.
- Quick Sims the coverage gate refuses, about 5% of them: some item swaps, some custom
  rotation syntax and a few spells and set bonuses. Each refusal gives a reason code.
- Quick Sims on MythicSim's second worker, which does not carry the Rust engine yet.

A character Rust preparation does not cover yet is prepared by the Go exporter, and Rust
still runs the fight.

## The Go reference

The reference pin stays at Go revision `a667dce8492bd9fb94269a9f864289cb74006775`, the fork's
patches 190 to 192 on the last Go release. Every accepted fixture keeps its Go golden, so a change to a mechanic Go covers
still has to match it or explain why it no longer should. A match means the same behavior as
that Go engine, not proof of the live game. [UPSTREAM.md](UPSTREAM.md) records how the pin
moved while Go led, and [docs/validation-summary.md](docs/validation-summary.md) lists the
comparison records.

## Rust-only mechanics

Mechanics the Go engine never had have no Go golden. They are checked against the game's own
numbers instead: the client spell rows, Wowhead Forever tooltips and logs, with focused tests
that pin the expected heal, cost or timing from those numbers. A request that reaches one runs
in Rust or is refused; it never falls back to Go, which would skip the spells it does not know.
The healer sims are the first: every Holy Paladin, Priest, Restoration Shaman and Restoration
Druid heal lands on a player who needs it, less the share real healers overheal (below), so
healing per second is what the healer's mana heals in a dungeon.

Where Rust departs from a mechanic the Go reference has, preparation follows Forever by default
and the reference with `Rules::GoReference` (`prepare --go-rules`), which every comparison with
the Go goldens and exporter uses. The departures:

- Pet hit, from Forever's developers on their tracker (22 October 2026): a Hunter's pet takes
  17/8 of its master's ranged hit as spell hit (Lightning Breath and the other magic pet hits),
  and a Warlock's demon takes 8/17 of its master's spell hit as melee hit, where the reference
  gives a Hunter pet no spell hit and a demon the whole spell hit.
- Improved Mind Flay, from its Forever tooltip ("Each instance of damage of Mind Flay deals 20%
  more damage than its previous instance of damage"): each tick deals the step more than the tick
  before it, compounding (1, 1.2 and 1.44 times the tick at two points), where the reference adds
  the step per tick (1, 1.2 and 1.4). The talent is a dummy aura run by a server-side script, so
  the client data has no stacking rule of its own.
- Level partial resists, from Forever's beta logs on foreverlogs.gg (23 October 2026): a target
  above the caster's level partially resists nothing, dot ticks and direct hits alike, so only
  resistance past the caster's Spell Piercing does. About 1,000 dot ticks and 100 direct
  non-binary hits on Razorfen Kraul and Scarlet Monastery bosses, which miss level 30 casters'
  Shadow Bolts a third of the time, all landed whole. The reference's level 63 boss partially
  resists 18% of them, 6% of the damage on average. The prepared target carries
  `no_level_partial_resist`.
- Casting regeneration auras, from the client's mana regeneration interrupt aura (type 134),
  which acts as it is applied: Resourcefulness, Rapid Recuperation and Improved Stormstrike let
  more spirit regeneration continue while casting from the moment they are up. The reference
  raises the rate without refreshing its cached mana tick, so the buff acts only once a change of
  MP5, Spirit or Intellect refreshes it, which on a Survival Hunter is almost never. Each one's
  mana lands in a resource row of its own, named after the aura, as Innervate's does. The
  prepared player carries `casting_regen_at_once`.
- Overhealing, from Forever's level 30 beta logs on foreverlogs.gg (1 to 24 October 2026, 74
  public reports): a healer sim counts each heal less the share real Holy and Discipline
  Priests, Holy Paladins and Restoration Druids and Shamans overhealed of it, such as Flash Heal
  1%, Holy Light 10%, Renew 16%, Prayer of Healing 23% and Healing Stream Totem 63%. Heals the
  logs do not hold yet (Greater Heal, Chain Heal, Prayer of Mending and the other heals above
  level 30) take the pooled share of their kind: 5.4% on one player at once, 16.7% over time,
  24.6% on a group. Shields keep their whole absorb. The shares are dungeon play at level 30 and
  will move with level 60 raids. The reference, like Go, counts every heal in full. The prepared
  player carries `overheal`, and each heal's overhealing is reported beside its healing
  (`src/prepare/overheal.rs`, `validation/2026-10-24-healer-overheal.json`).
- Heal over time crits, from Forever's level 30 beta logs on foreverlogs.gg (1 to 24 October
  2026) and the client rows: Renew, Rejuvenation, Regrowth and Healing Stream Totem ticks crit
  for 1.5 times the tick, between normal ticks on the same player (a Holy Priest's Renew ticks
  97 and crits 145 and 146, a Restoration Druid's Rejuvenation ticks 95 and crits 141 and 142).
  Pooled over 104 reports, 7.0% of 8,245 Renew ticks and 5.7% of 7,171 Rejuvenation ticks crit,
  near the casters' direct heal crit (9.4% and 5.9%); a tick that wholly overheals loses its crit
  flag, so heavy overhealers read low. Every heal over time row the healer sims use states
  Periodic Can Crit, the client bit damage ticks already follow. A tick rolls the healing crit on
  spell crit and the spell's crit bonus, as a direct heal does (Improved Regrowth's reaches
  Regrowth's ticks, as the talent names the whole spell), less what a damage tick does not take
  (Inner Focus). The reference, like Go, never crits a heal tick. Each prepared heal over time
  whose row says so carries `tick_can_crit`
  (`validation/2026-10-25-heal-tick-crits.json`).

## Repository layout

| Path | Contents |
| --- | --- |
| `src/core`, `src/mechanics` | Shared fight runtime and combat rules |
| `src/classes/<class>` | Class spells and talents, with specs under `specs/` |
| `src/prepare` | Preparing a character from a request |
| `src/engine` | Coverage gate and orchestration |
| `src/contracts` | Input and output types |
| `tests/classes` | Tests, one folder per class |
| `fixtures` | Prepared inputs and Go results |
| `tools` | Exporter, routing, comparison and validation tools |

The [contributor code map](docs/contributor-guide.md) shows where each mechanic lives.

## Contribute

Read [CONTRIBUTING.md](CONTRIBUTING.md) for the checks to run. A mechanics change needs a
fixture or test that shows it matches Go. Open work is on the
[contributor board](https://github.com/sage3648/mythicsim-forever-engine/issues/25).

## License and acknowledgments

MIT licensed, with new project work credited to MythicSim contributors.
Upstream wowsims notices are retained for adapted portions in [LICENSE](LICENSE).
RNG behavior, combat semantics and reference tooling derive from
[wowsims Forever](https://github.com/ElliotWood/Forever) and
[MythicSim's Go fork](https://github.com/sage3648/mythicsim-forever-engine-go).
See [NOTICE.md](NOTICE.md) for provenance. This project is not affiliated with
Blizzard Entertainment or the WoW Forever server team.
