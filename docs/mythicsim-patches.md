# MythicSim downstream patches

MythicSim runs this engine from its fork (`sage3648/mythicsim-forever-engine`, branch
`mythicsim/wowsims-forever`). The branch is ElliotWood/Forever master, which is built on the
official wowsims/forever, plus the three patches below. The first base was `442076902` (Merge
wowsims/forever master ea5412873).

Keep the set small. Each patch exists because MythicSim needs something upstream does not do
yet. Drop a patch as soon as upstream covers it; do not keep ours alongside an upstream version.

| # | Commit subject | Why MythicSim needs it |
|---|---|---|
| 1 | `cli: sim --strict rejects unknown fields and enum names` | The worker builds requests in code. Without it, a misspelt field or a race the build does not know is dropped silently and the sim runs a different character. |
| 2 | `core: a player option to disable racials` | The race comparison page sims each character with and without its racials to show what they are worth. |
| 3 | `core: Forever races from client 1.60.1.69977` | Upstream has only the Classic/TBC races and their TBC racials. MythicSim's race pages need the Skyborne, the Forever pairings and the client's racials. |

## 1. `cli: sim --strict`

- **What it does.** `wowsimcli sim --strict` loads the request with protojson `DiscardUnknown`
  off and exits non-zero with the protojson error. The flag defaults to off, which is
  upstream's behaviour.
- **Files.** `cmd/wowsimcli/cmd/basic_sim.go` (`loadRaidSimRequest`) and `basic_sim_test.go`.
- **Drop it when** upstream's CLI can reject unknown names itself. If upstream's flag has a
  different name, switch the worker to it and drop this patch.
- **Conflicts** can only happen where `simMain` loads the file.

## 2. `core: a player option to disable racials`

- **What it does.** `Player.disable_racials` (field 59, JSON `disableRacials`) skips every racial
  effect and keeps the race's base stats. Besides `applyRaceEffects`, it gates the race-only
  effects that live elsewhere: the night elf priest's Starshards (`sim/priest/priest.go`),
  Bloodthistle (`sim/core/consumes.go`) and the racial multipliers the reforge optimizer models
  (`sim/core/reforge_optimizer/model.go`). The rest of the code checks
  `Character.RacialsDisabled()`.
- **Tests.** `sim/core/racials_test.go` (`TestDisableRacials*`) and
  `sim/priest/racials_test.go`.
- **When rebasing**, grep upstream's new code for race checks outside `applyRaceEffects`
  (`\.Race ==`, `Race_Race`, `GetRace()`) and gate any new ones on `RacialsDisabled()`.
- **Field number.** The worker sends protojson, which reads the field by name, so the number
  only matters for binary protos and saved UI links. If upstream takes 59 for something else,
  move ours to the next free number and leave the name alone.
- **Drop it when** upstream has an equivalent option. Point the worker's `disableRacials`
  (`worker/cmd/refresh-forever-races`) at upstream's name first.

## 3. `core: Forever races from client 1.60.1.69977`

The race model MythicSim's previous engine line (the wowsims/classic-based fork, commits
c3628f8ef, 4d3f73525 and a4db4e96c) verified against client 1.60.1.69977 from the wago.tools
tables. `docs/forever_rules.md` (Racials) lists each rule and its source, and
`docs/beta-pass/*.md` records the same spell ids.

- **Races.** `RaceSkyborneHighOrder = 11` and `RaceSkyborneWindshaper = 12` are appended to
  `Race` in `proto/common.proto`. Faction comes from race, so each Skyborne half is its own race:
  High Order is Alliance and Windshaper is Horde. Go derives it in `Character.GetFaction()`
  (`sim/core/racials.go`, used by the Draenei party aura). The UI derives it in
  `raceToFaction` (`ui/sim/proto/utils.ts`).
- **Base stats and pairings.** `sim/core/base_stats.go` adds the Skyborne offsets (zero) and
  these rows: Skyborne Warrior, Hunter, Rogue and Druid on both halves, High Order Mage,
  Windshaper Shaman, Human Hunter, Undead Paladin and Dwarf Shaman.
  `ClassRaceCapabilities` (`sim/core/character_constants.go`) offers those and also Orc Mage,
  Troll Warlock and Gnome Priest, which already had base stats. Regenerate
  `ui/sim/player/classes/capabilities_auto_gen.ts` with
  `go run ./tools/database/gen_db -outDir=./assets -gen=go-to-ts`.
- **Racials** are in `sim/core/racials.go`. Blood Elf and Draenei are left as upstream has them.
  - Blood Fury (20572) is a 10% multiplier. Eureka! is one spell per class. Berserking is a
    flat 10% and costs nothing. Touch of the Grave, Elune's Light, and the Skyborne Wind
    Blessed and Elemental Insight are all in.
  - The weapon racials pay crit instead of expertise. Dwarves get Big Game Hunter. Tauren
    Endurance adds hit. The Human Spirit is 5%.
  - Every resistance racial is gone, and so are Command and the troll ranged
    specializations.
- **Decisions to keep in mind:**
  - Berserking keeps action id `26297` tag 2, the previous line's fixed 10% option, so
    rotations saved against that line still find it. The client id is 20554, which the
    manifest records as `foreverId`.
  - Stoneform is a survival cooldown that only an APL action casts, as on the previous line.
    Upstream auto-casts it as a DPS cooldown on the GCD.
  - Blood Fury's spell power part multiplies `SpellDamage` and `HealingPower`. The school
    damage stats cannot carry a stat dependency, so they get a 10% snapshot when the buff
    lands.
- **UI.** Race names, i18n keys (`assets/locales/en/character.json` and
  `schemas/character.schema.json`) and the evidence manifest (`ui/sim/spells/core.json`: the
  racial entries). `Player.getActiveRacialExpertiseBonuses` returns none, because no race has
  racial expertise now. `ui/app/arena/results.json` is not regenerated here; the arena rebuild
  refreshes it.
- **Tests.**
  - `sim/core/racials_test.go`: each racial's figures, the resistances, faction, and base stats
    for every pairing on offer.
  - `sim/warrior/dps/forever_racials_test.go`: the Skyborne halves sim alike, every warrior race
    sims, and the weapon racials pay crit.
  - `sim/rogue/touch_of_the_grave_test.go` and `sim/mage/eureka_test.go`.
- **Goldens.** This patch re-blessed every suite whose races changed, which is all of them
  except the Draenei resto shaman.
- **Drop a piece when** upstream models it. Compare upstream's version against the tests
  above and the client figures in `docs/forever_rules.md`. Keep whichever matches the client
  and delete the other rather than stacking both. If upstream adds the Skyborne to `Race`
  under other numbers, take upstream's numbers. The worker sends race names, so only saved UI
  links would notice.

## Rebasing onto a newer upstream

1. Fetch ElliotWood/Forever master. Rebase the patches onto it:
   `git rebase --onto <new upstream> <old base> mythicsim/wowsims-forever`.
2. Resolve `*.results` conflicts by taking upstream's side. During a rebase `--ours` is the
   upstream side and `--theirs` is the patch being replayed, so use
   `git checkout --ours -- <file>`. Never hand-merge a golden.
3. Regenerate the protos (the `-I=/usr/include` is libprotobuf-dev's `descriptor.proto`),
   build, and run the patches' own tests:

   ```
   protoc -I=./proto -I=/usr/include --go_opt=Mgoogle/protobuf/descriptor.proto=google.golang.org/protobuf/types/descriptorpb --go_out=./sim/core ./proto/*.proto
   go build --tags=with_db ./sim/... ./cmd/...
   go test ./cmd/wowsimcli/...
   go test --tags=with_db ./sim/core -run 'Racial|Skyborne|Forever|Faction|BaseStats|Bloodthistle'
   go test --tags=with_db ./sim/warrior/dps -run 'Forever|Skyborne|EveryWarrior'
   go test --tags=with_db ./sim/rogue -run TouchOfTheGrave
   go test --tags=with_db ./sim/mage -run Eureka
   go test --tags=with_db ./sim/priest -run 'Starshards|Arena'
   ```

4. Run the full suite, `go test --tags=with_db $(go list ./sim/... | grep -v sim/web)`. Goldens
   that fail only because of patch 3's racials are re-blessed: copy `X.results.tmp` over
   `X.results`. Commit them with patch 3 (a fixup) so each patch still carries the goldens it
   moves. Do this only after checking that the change is the racials' doing. A quick check is
   to compare the suite's rows for the races patch 3 touches and confirm the others are
   unchanged.
5. Check each patch's "drop it when" condition above, and update this file when a patch goes.
