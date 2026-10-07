# Upstream tracking

## Reference baseline

| Source | Role | Baseline |
| --- | --- | --- |
| [MythicSim Go engine](https://github.com/sage3648/mythicsim-forever-engine-go) | Fixtures and live reference | `2d93e423e0303e93dbb16e190d435503248b68f9`, on community base `5c115f1725ede7419e4f8582a7d94cdb0247c829` |
| [Community Forever engine](https://github.com/ElliotWood/Forever) | Changes to review for applicability | Adopted base `f4b776b4f41d5c7799b8141697a2c9e67c89d426`; reviewed through `5c115f1725ede7419e4f8582a7d94cdb0247c829` (2026-10-07) |

Go is a reference implementation, not proof of live-game correctness. Forever can
intentionally differ from inherited Classic behavior. Fixture client build:
`1.60.1.70235`.

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

The pin moved from `20b551c6b` to `6383c15a7` on 2026-10-06, one commit: the fork's
patch 87, where a Mongoose Bite that lands with the last Lacerating Strikes bleed still up
replaces it with its own 40% instead of a bleed that ticks for nothing, and the bleed
reports under its own id 1310536 instead of Mongoose Bite's with tag 1. The six Survival
Hunter goldens with Lacerating Strikes changed and Rust matches them; no other golden
moved, and all 2439 recorded sweep variants match
([record](validation/2026-10-06-reference-pin-6383c15a7-sweeps.json)).

The pin moved from `6383c15a7` to `74127c6c8` on 2026-10-06, two commits: the fork's
patches 88 and 89, which fix the two [reference defects](#reference-defects) below. Ignite
ignores hits on a unit that is not an enemy, so a Mage with Ignite and a Goblin Sapper Charge
no longer panics, and a pushback leaves a hardcast that finished during the spell batch
window alone. The one golden with a late pushback changed and Rust matches it; the Goblin
Sapper Fire Mage is accepted with Go goldens; no other golden moved
([record](validation/2026-10-06-reference-pin-74127c6c8-sweeps.json)).

The pin moved from `74127c6c8` to `cd7d44aec` on 2026-10-07, the fork's merge of community
#642 to #676 (69 commits, 35 of them changelog, leaderboard or data bookkeeping) and its
patches 85 to 96. Patches 93 and 94 are the Ignite and pushback fixes Rust already
followed, which were 88 and 89 at the old pin. 342 of the 343 Go goldens changed. Every
golden with damage gained the hit, crit, tick and crit tick ranges the fork reports since
its patch 92, and 133 changed in nothing else; the other 209 changed in behavior:

- Mage: Missile Barrage rolls when a bolt lands, Blizzard's and Flamestrike's ticks crit,
  Blizzard's cast rolls a hit on every enemy and Arcane Concentration skips the missiles and
  the ticks, Arcane Missiles spends Arcane Blast's stacks as it starts, Pyroblast's hit takes
  the bonus as direct damage, Master of Elements reads the base cost, Presence of Mind and
  Combustion share a cooldown, and Ignite reads Can Proc From Procs from the row (#643, #646,
  #647, #656, #660, #661).
- Shaman: Windfury Weapon strikes twice with the special hits 439440 and 439441 and no extra
  attack, Lightning Overload rolls its own rows, Elemental Devastation and Flurry read Can Proc
  From Procs, and Rockbiter Weapon is an attack power effect (#644, #646, #659, #673).
- Warrior: a refreshed Deep Wounds keeps its tick timer (patch 89) and its ticks ignore the
  warrior's modifiers, Sweeping Strikes needs a second target, Unbridled Wrath gives the same rage
  for every weapon, and Charge adds no threat (#645, #653, #667).
- Hunter: trap burns, Scorpid Poison and Volley ticks crit, Volley rolls the ranged tables, a
  hawk's dive bomb ticks roll the melee special table, and the traps keep their own cooldown
  categories (#650, #657).
- Warlock: Rain of Fire's cast rolls a hit on every enemy and its ticks crit, Demonic Brand
  rolls the pet's crit, and Life Tap adds no threat (#648, #652, #672).
- Rogue: Mutilate strikes main hand first, Hemorrhage raises each Rupture tick, Preparation
  finishes every other cooldown, Hack and Slash reads the right effects and Thistle Tea restores a
  flat 100 (#654, #665). Priest: Dark Sacrifice is Undead only and adds no threat (patch 85).
- Items: Dragon's Call's whelp waits out a 45 second cooldown (patch 86).

Rust matches all of them, and so does every recorded sweep: 9,052 of the 9,076 variants
match Go, the other 24 are the multi-target tank builds Rust refuses, and none differ
([record](validation/2026-10-07-reference-pin-cd7d44aec-sweeps.json)).

The pin moved from `cd7d44aec` to `2d93e423e` on 2026-10-07, the fork's merge of community
#677 to #719 (76 commits, 35 of them changelog or leaderboard bookkeeping) and its patches 97
and 98. The fork dropped its patches 3, 76, 91, 93 and 94, which the community now carries:
the Ignite and pushback fixes Rust already followed are #702 and #701 there. 270 of the 356
Go goldens changed. 63 changed only in the uptime a Windfury Totem, the party's or cast, now
reports in the Flametongue Totem category, 18 bear goldens only in the energy regeneration
metrics and 7 Smite goldens only in the Penance ranks now registered; the other 182 changed in
behavior:

- Hunter: a hawk swings on arrival and then every 2.5 seconds hasted by real ranged haste, for
  0.35 of the dive bomb base and without a crit, so dots hasted by real haste are supported;
  pet focus refills 2.5 every 250 ms; Scorpid Poison stacks to five; Intimidation is no longer
  a major cooldown (#694, #703, #704, #713).
- Mage: Arcane Missiles' missiles keep the Arcane Blast stacks the channel spends, 15% each
  (#715).
- Druid: Hurricane's ticks crit, Moonfire applies its dot as one hit event and Thorns can be
  cast in Moonkin form (#688). Energy refills 1 every 100 ms for cats and rogues alike (#709).
- Rogue and Warrior: Cold Blood is not spent by Mutilate's hand strikes, and Unbridled Wrath
  procs from white hits only (#690, #692).
- Priest: Shadowform is a form Holy Nova's and Chastise's cast requirements refuse, and a
  helpful Holy cast such as Power Infusion no longer ends it; every Penance rank is registered;
  Holy Precision and Holy Specialization leave Chastise out (#678, #679, #686, #719).
- Shaman: Searing Totem attacks every 2.43 seconds, Flametongue Totem's hit takes no talent,
  and a Windfury Totem switches Flametongue Totem off (#677, #682, #697, patch 98).
- Warlock and Paladin: the Imp waits 0.4 seconds between Firebolts, Malediction leaves
  Hellfire alone, Demonic Embrace keeps its -1% Spirit, and Sanctified Judgement returns 10/9
  of its stated share (#680, #687, #698, #705).

Rust matches all of them, and the two fixtures accepted at the pin: a Shadow Priest whose
Shadowform refuses Holy Nova and survives Power Infusion, and the fork's patch 97, Totem of
Thunder's 1% crit on Lightning Bolt. The recorded sweeps ran again on the production Mac,
capped to two engine processes at background priority: 8,120 of the 9,076 variants, all of
which match Go, the 24 multi-target tank builds Rust refused at `cd7d44aec` among them. The
other 956 are the Warlock multi-target census past its first 480 variants and the Warlock
multi-target sweep past the first four variants of each base, generated and not compared
([record](validation/2026-10-07-reference-pin-2d93e423e-sweeps.json)).

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
Rust covers by then as applicable and adopted. The move to `cd7d44aec` extended it to 124:
39 of the new 69 are bookkeeping, evidence notes or UI, 5 the fork already carried as its
own patches, 1 differs by design and 24 are applicable, each with the golden that regresses it.
The move to `2d93e423e` extended it to 200: 46 of the new 76 are bookkeeping, rotation presets,
evidence notes, comments or gear sets, 2 are fixes Rust already followed as the fork's patches,
2 restart the swing after a hard cast where the fork keeps its patch 17, and 26 are applicable
and adopted, 6 of them client data. The first fix,
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

Bugs in the reference. When one blocks a comparison, Rust refuses the affected inputs until
the reference is fixed, since there is no Go result to match. When Go still gives a result,
Rust reproduces it, so it matches the engine production runs. Both defects below were fixed in
the fork at `74127c6c8`, as its patches 93 and 94, and in the community engine by #702 and #701,
which the fork takes in their place since `2d93e423e`.

### Ignite on the Goblin Sapper Charge's hit on the player

The engine panicked with a nil pointer dereference at `sim/mage/talents_fire.go:135`
when a Mage with Ignite throws a Goblin Sapper Charge and the charge's hit on the Mage
crits. Community commit `8fb1a2d75a`, in the adopted base, deals that hit through its own
spell with `ProcMaskSpellDamage` and School Fire. The Mage's `OnSpellHitDealt` hears it,
and Ignite's trigger (`ProcMaskSpellDamage`, `OutcomeCrit`, Fire) calls
`mage.Ignite.Dot(result.Target)` with the Mage as the target. Dots exist only on enemies,
so `dot.IsActive()` dereferences nil. The handler is unchanged on community `master` at
`b49be9e13` (2026-10-04).

- Reproduction: run the engine before `74127c6c8` on
  `fixtures/mage/prepared-v2/fire-mage-goblin-sapper.request.json`, the production Fire
  request with `goblinSapper` set. The first charge whose hit on the Mage crit panicked,
  inside `Character.newBasicExplosiveSpellConfig` (`sim/core/consumes.go:650`).
- Affected records: the Arcane and Fire Mage `goblinSapper` variants in
  `validation/2026-10-04-production-gear-swaps.json`, recorded as `go_error`.
- Report: [ElliotWood/Forever#699](https://github.com/ElliotWood/Forever/issues/699)
  (2026-10-06), with the reproduction and the trace above.
- Fixed in the fork's patch 93 (`8c34bdf376`, patch 88 and `8979ea9ac0` at the old pin), in the
  reference since `74127c6c8`: Ignite's
  trigger also requires an enemy target. Rust's Ignite ignores hits on the player, the Mage
  gate no longer refuses an Ignite build that throws the charge, and
  `fire-mage-goblin-sapper` is accepted with Go goldens. Before the fix the gate refused
  those builds, and before the guard Rust rolled the Mage's own crit into an Ignite on the
  target.

### A pushback after the hardcast completed

The "Pushback trigger" in `sim/core/character.go` checks that a hardcast is in progress
when a landed hit passes its conditions, but its handler runs a spell batch window later
and does not check again. If the hardcast completes inside that 10 ms window, the handler
still calls `Hardcast.pushBack` (`sim/core/cast.go`), which extends the finished cast's
`Expires` by up to 500 ms, and `newHardcastAction` schedules the cast again. The cast
completes a second time at once and its effect lands twice. Unlike the Ignite crash, this
did not block a comparison: Rust reproduced it in `Fight::pushback_handler` until the fix.

- Reproduction: run the engine before `74127c6c8` on
  `fixtures/mage/prepared-v2/feral-bear-druid-boomerang-pushback-after-cast.request.json`,
  a tanking Feral (bear) Druid that hardcasts Linken's Boomerang for 500 ms. The target's
  swing lands 10 ms before the cast completes. The Go log showed `Completed cast
  {ItemID: 11905}` at 0.50, then `pushed back 500ms while casting` and a second `Completed
  cast` at the same time, and the Boomerang deals two hits at 1.00.
- Report: [ElliotWood/Forever#700](https://github.com/ElliotWood/Forever/issues/700)
  (2026-10-06), proposing to re-check `Hardcast.Expires > sim.CurrentTime` in the handler.
- Fixed in the fork's patch 94 (`23f4e681e1`, patch 89 and `74127c6c84` at the old pin), in the
  reference since `74127c6c8`: the
  handler returns before the pushback roll when the hardcast has ended. Rust's
  `Fight::pushback_handler` does the same, and the
  `feral-bear-druid-boomerang-pushback-after-cast` golden now completes the cast once, with
  no pushback. A hit in the last batch window of a cast no longer pushes it back at all.

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
