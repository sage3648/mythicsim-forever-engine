# First Frost build inventory

Piece 1 is complete: a real application reference request, an observed full Go run,
its mechanics checklist and its application consumers are frozen in
[inventory/first-frost](../inventory/first-frost/manifest.json).
This is planning and reference evidence. Rust still supports only its prepared
Frostbolt kernel and rejects this production request.

## Frozen starting point

| Identity | Value |
| --- | --- |
| Application commit | `1ca721161aba693a899466943b8f6c3a6085df67` |
| Go engine commit | `6823b49eb8aff741f197ef36d83766ef6a218285` |
| Community base recorded by application | `f4b776b4f41d5c7799b8141697a2c9e67c89d426` |
| Client build | `1.60.1.70170` |
| Reference | `frost-mage`, Missile Barrage 16/3/32 |
| Character | Human Mage, level 60, Engineering 300 |
| Talents | `05020500300001-03-0555000301001301251` |
| Gear | Curated dungeon/crafted reference dated 2026-09-23, empty trinkets |
| Talent credit | mixarxrt's tested Missile Barrage build |
| Fight | One level 63 target, 120 seconds with 15-second variation, 20 yards |
| Simulation | 3,000 iterations, seed 42, first-iteration debug |

The input is the application's synthetic reference, not a private player's export.
The application may advance independently. These pins describe the captured source,
not a claim about what is currently deployed. The recorded community base is not
an independently reviewed reconciliation baseline.

[snapshot.json](../inventory/first-frost/snapshot.json) was produced by calling
`ReferenceBuildBySlug` and `BuildRequest` from the pinned application.
It contains the original export, actual protojson request, selected rotation and
warnings. Do not feed it to Rust's current `sim` command.

[observation.json](../inventory/first-frost/observation.json) contains exercised
Go action metrics, aura metrics, resource flows, target auras and the first-fight
timeline parsed by the application's own report helpers. Registered but unused
spell ranks are omitted. It is a historical observation, not an accepted full-build
Rust golden, a benchmark or live-game proof.

The [Go parity validation](go-parity-validation-2026-10-03.md) records a fresh
reproduction of all saved output fields and separate live Rust kernel comparisons.

The observed run completed all 3,000 iterations with mean DPS about **590.947**
and mean fight length about **119.899 seconds**. Those numbers carry the inherited
mechanics caveats below. No Rust full-build result exists to compare yet.

## Rotation and required mechanics

The production Frost preset has six priority rules, in this order:

1. Use Mana Ruby, item `8008`, at or below 80% mana.
2. Autocast other cooldowns.
3. Evocation `12051` below 20% mana with more than 25 seconds remaining.
4. Ice Lance `1240047` while Fingers of Frost `400669` is active.
5. Arcane Missiles `25345` when Missile Barrage `44404` is known and active.
6. Frostbolt `25304`.

Required operators are `castSpell`, `autocastOtherCooldowns`, `cmp` with
`OpLe`/`OpLt`/`OpGt`, `and`, `const`, `currentManaPercent`, `remainingTime`,
`auraIsKnown` and `auraIsActive`. Percent and duration constants need their own
units. Decision timing and unavailable-action behavior need parity, too.

The inventory records all 17 selected talents and all 17 export gear slots,
including empty slots and enchant IDs. The request builder compresses empty
equipment entries: request array positions must not be mistaken for export slots.
Talent identity, cast identity and triggered aura identity can differ. Cold Snap
is `12472` in this client; inherited spell names are insufficient to identify it.

| System | Required behavior beyond isolated Frostbolt |
| --- | --- |
| Ice Lance | Instant cast, travel, binary resistance, frozen multiplier and proc consumption |
| Arcane Missiles | Channel `25345`, distinct damaging tick `25346`, hit/crit, partial resistance, travel and modified tick timing |
| Procs | Clearcasting, Missile Barrage, Fingers of Frost and Winter's Chill, including stacks, expiry, ICDs and callback order |
| Cooldowns | Cold Snap, Evocation, four registered mana gems, potion, Demonic Rune and equipped Robe of the Archmage |
| Mana | Mage Armor, Spirit/MP5, five-second rule, shared conjured cooldown, Judgement of Wisdom, mana cost and overcap reporting |
| Preparation | Base/racial stats, talents, item and enchant stats, Engineering, oils, elixirs, food, flask, raid buffs and debuffs |
| Encounter | Variable fight duration, target level, travel distance, seed handling and end-of-fight events |
| Reports | Headline, action and resource metrics, aura uptime, rotation/settings, first-fight and averaged timelines |

The full reference exercised Frostbolt, Ice Lance, Arcane Missiles and its tick,
Cold Snap, Mana Ruby, potion, rune and Robe mana restoration. Evocation and the
three lower gems were not exercised in this run but are still required: longer
or mana-starved fights can reach them. An observed action list alone is insufficient
to declare coverage.

The default assumptions include Mage Armor, Arcane Brilliance, Prayer of Spirit,
Gift of the Wild, Mana Spring, Moonkin Aura, Kings, Wisdom, Curse of the Elements
and Judgement of Wisdom. Consumables include Supreme Power, Frost Power,
Greater Arcane Elixir, Mageblood, food, oil and Cerebral Cortex Compound.
The exact IDs and enabled fields live in the frozen request.

No pet or engineering explosive is active in this request. Do not add Water
Elemental merely because another Mage simulator supports it. Defensive talents
Ice Block and Ice Barrier are selected but modeled as no-ops in the Go reference.
The initial scope has no Mage incoming-damage simulation.

## Open correctness questions

- The application warns that Arcane Missiles does not spend a Fingers of Frost
  charge in Go and DPS can be overstated. Retain that warning until a reviewed fix.
- Missile Barrage uses inherited hardcoded values because generated client rows
  are missing. These values are compatibility assumptions.
- Ice Lance's `0.143` coefficient is a Go estimate supported by preliminary beta
  log reasoning, despite a zero coefficient in the client row.
- Mind Carver's Carved Mind effect `1302342` appears in item data but its Go proc
  registration is commented out. Go parity does not prove full in-game support.
- This synthetic export lacks addon talent spell IDs. Its positional mapping is
  pinned and the application's warning is preserved.

Each question has file evidence in `manifest.json`. Piece 3 will turn these into
the upstream/mechanics ledger. Accepted game corrections must be distinguished
from unexplained Rust discrepancies.

## Audit and reproduce

The ordinary audit needs Python 3 only and runs in CI:

```sh
python3 tools/inventory.py check
python3 -m unittest discover -s tools -p '*_test.py'
```

It checks frozen hashes, request field coverage, selected talents, equipment and
enchants, observed/rotation action IDs, source references, completed iterations
and first-fight timeline presence. It verifies inventory consistency, not mechanic
implementation or completeness of future Rust coverage.

To verify source provenance, supply checkouts at the exact commits above:

```sh
python3 tools/inventory.py check \
  --app-source /absolute/path/to/mythicsim \
  --engine-source /absolute/path/to/go-engine
```

To recapture evidence, use Go 1.25 or newer and those same checkouts. The Go
checkout needs its generated proto and embedded database available, as for the
existing comparison oracle. The application source is needed only for this
maintainer capture; community contributors can use the frozen files offline.

```sh
python3 tools/inventory.py capture \
  --app-source /absolute/path/to/mythicsim \
  --engine-source /absolute/path/to/go-engine \
  --output output/frost-reference-recapture
```

The output directory must not exist. Capture builds helper programs in temporary
modules, reads supplied source and runs locally without queuing production jobs.
It refuses to replace accepted inventory files. It compares request contents,
action identities and every saved metric/timeline field. Integer counts must match
exactly; floating-point comparisons allow only `1e-8` absolute or `1e-12` relative
roundoff. A difference fails capture and is listed in `comparison.json` for review.
Go dependencies can be
downloaded on first use.

Source hashes cover 51 relevant application and Go files. They catch changes to
the audited surfaces; they are not a digest of every transitive build input.
The full commit pins provide the broader reference identity.

Next is piece 2: design prepared v2 and the release compatibility manifest around
these requirements. The existing Rust schema remains unchanged in this piece.
