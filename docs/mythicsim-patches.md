# MythicSim downstream patches

MythicSim runs this engine from its fork (`sage3648/mythicsim-forever-engine`, branch
`mythicsim/wowsims-forever`). The branch is ElliotWood/Forever master, which is built on the
official wowsims/forever, plus the three patches below. The first base was `442076902` (Merge
wowsims/forever master ea5412873). The current base is `6cb2603d`, which carries client
1.60.1.70009 and its 2026-09-24 patch notes.

Keep the set small. Each patch exists because MythicSim needs something upstream does not do
yet. Drop a patch as soon as upstream covers it; do not keep ours alongside an upstream version.

| # | Commit subject | Why MythicSim needs it |
|---|---|---|
| 1 | `cli: sim --strict rejects unknown fields and enum names` | The worker builds requests in code. Without it, a misspelt field or a race the build does not know is dropped silently and the sim runs a different character. |
| 2 | `core: a player option to disable racials` | The race comparison page sims each character with and without its racials to show what they are worth. |
| 3 | `rotation: Destruction casts Conflagrate for Shadow and Flame` | The Destruction rotation never casts Conflagrate, so Shadow and Flame's Shadow buff never applies to the Shadow Bolt filler. |

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

## Dropped: `core: Forever races from client 1.60.1.69977`

Dropped in the rebase onto `6cb2603d`. Upstream now models the Forever races itself: the Skyborne
(`RaceHighOrderSkyborne`, `RaceWindshaperSkyborne`), the Forever race/class pairings, and the
client's racials. Blood Elf, Draenei and Bloodthistle are gone. The patch's client tests were run
against upstream's version:
- Blood Fury, Berserking, Elune's Light, Touch of the Grave, Tauren Endurance, the Human Spirit and
  the weapon specializations match.
- The Skyborne match. Upstream applies Wind Blessed as one attack speed multiplier, and it takes
  the Skyborne base stat offsets (±1) from level 1 character sheets where the patch had zero.
- Eureka! is the 70009 client's 10% cost cut on every class. The patch had 69977's per-class
  figures.
- Upstream also models Gnome Expansive Mind, which the patch left out.
- The pairings are the ones MythicSim's race pages offer.

## 3. `rotation: Destruction casts Conflagrate for Shadow and Flame`

- **What it does.** `ui/specs/warlock/dps/apls/destruction.apl.json` casts Conflagrate (18932)
  after the Immolate refresh, while Immolate is up and either Immolate has under 4 seconds
  left or the warlock knows Shadow and Flame (Shadow, 1293816) and its buff is down. Without
  the talent it only Conflagrates at the end of Immolate. The rule is the one MythicSim's
  previous engine line measured (+5.9% on the 5/5 Destruction reference, neutral for 2/5
  builds). MythicSim copies this rotation into its worker presets.
- **Goldens.** `sim/warlock/TestDestruction.results` (average 410.50 to 420.67 DPS).
- **Drop it when** upstream's Destruction rotation casts Conflagrate.

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
   go test --tags=with_db ./sim/core -run 'DisableRacials|Racial|Skyborne'
   go test --tags=with_db ./sim/priest -run 'Starshards|Arena'
   ```

4. Run the full suite, `go test --tags=with_db $(go list ./sim/... | grep -v sim/web)`. Only
   `sim/warlock/TestDestruction` should fail, from patch 3's Conflagrate. Re-bless it (copy
   `TestDestruction.results.tmp` over `TestDestruction.results`) and fold it into patch 3, so
   the patch carries the golden it moves. Any other failure is upstream's or the rebase's, not a
   golden to re-bless.
5. Check each patch's "drop it when" condition above, and update this file when a patch goes.
