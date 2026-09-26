# MythicSim downstream patches

MythicSim runs this engine from its fork (`sage3648/mythicsim-forever-engine`, branch
`mythicsim/forever-70009-latest-sep26`). The branch is ElliotWood/Forever master, which is built on the
official wowsims/forever, plus the six patches below. The first base was `442076902` (Merge
wowsims/forever master ea5412873). The current base is `bb00613a3b` (2026-09-26). It includes the client hotfix databases, Hunter ranged scaling, Rogue Hack and Slash cooldown, Shaman Flametongue and Fire Nova fixes, and the merged Penance timing and cost fixes, Demonic Pact pre-pull sacrifice, Mana Tide Totem party restoration, Frost Mage talent fixes, and rank 4 Trueshot Aura. It also carries client 1.60.1.70009 and the earlier lower-rank spell, aura-cap, and consumable fixes.

Keep the set small. Each patch exists because MythicSim needs something upstream does not do
yet. Drop a patch as soon as upstream covers it; do not keep ours alongside an upstream version.

| # | Commit subject | Why MythicSim needs it |
|---|---|---|
| 1 | `cli: sim --strict rejects unknown fields and enum names` | The worker builds requests in code. Without it, a misspelt field or a race the build does not know is dropped silently and the sim runs a different character. |
| 2 | `core: a player option to disable racials` | The race comparison page sims each character with and without its racials to show what they are worth. |
| 3 | `rotation: Destruction casts Conflagrate for Shadow and Flame` | The Destruction rotation never casts Conflagrate, so Shadow and Flame's Shadow buff never applies to the Shadow Bolt filler. |
| 4 | `hunter: Aspect of the Beast` | Forever made Beast the melee aspect. Upstream models only Hawk, so a melee hunter has no aspect. |
| 5 | `rotation: a melee Survival rotation` | Upstream's Survival rotation shoots from range, so Raptor Strike, Mongoose Bite and Strider Kick never fire. MythicSim ranks melee Survival. |
| 6 | `items: Iceblade Hacker and Warblade of Caer Darrow proc from their own hand` | The two hand-written weapon procs fired off both hands, so a main-hand Iceblade Hacker added its Frost damage to every off-hand swing. |

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

## 4. `hunter: Aspect of the Beast`

- **What it does.** Registers Aspect of the Beast at its top rank (1299447): 110 melee attack
  power, exclusive with Aspect of the Hawk. With Deadly Aspects, a landed melee auto attack has
  Deadly Aspects' second effect as its chance (2% a rank) to trigger Quick Strikes (1299448), 30%
  melee haste for 12 sec. Beast states no proc chance of its own, so there is no Quick Strikes
  without the talent. Raptor Strike takes a swing's place as a special attack and does not proc it
  (Beast's proc flags are melee auto attacks, 0x4).
- **Files.** `sim/hunter/aspects.go`, the `AspectOfTheBeast` fields and spell mask in
  `sim/hunter/hunter.go`, and `sim/hunter/aspects_test.go`.
- **Tests.** `TestAspectOfTheBeastMeleeAttackPower`, `TestQuickStrikesNeedsDeadlyAspects`. No
  golden moves: the ranged rotations cast Hawk.
- **Drop it when** upstream models Beast. Check that its melee attack power and Quick Strikes
  match before dropping.

## 5. `rotation: a melee Survival rotation`

- **What it does.** `ui/specs/hunter/dps/apls/sv_melee.apl.json` is a melee rotation: Aspect of
  the Beast before the pull, then Raptor Strike queued on cooldown, Mongoose Bite whenever Expose
  Prey opens it, Summon Hawk, Strider Kick, Immolation Trap, and Wing Clip in any global where
  Raptor Strike and Strider Kick are more than half a second away. Wing Clip is a landed melee
  special, so it can proc Expose Prey (1310532's proc flags include melee specials, 0x10), and
  more Wing Clips mean more Mongoose Bites. Spells a build lacks are skipped, so the file serves
  any talents. `presets.ts` publishes it as `SurvivalMeleeRotation`, which MythicSim's
  `scripts/build-forever-gear.mjs` requires before it copies the file into the worker presets as
  `hunter_survival`.
- **Measured** on MythicSim's Beast Mastery reference character moved to 5 yards, with wowtbc.gg's
  5/10/35 build, at 10,000 iterations. Raptor Strike, Mongoose Bite and Strider Kick alone sim 380
  DPS. Adding Wing Clip takes it to 423, and Wing Clip plus Immolation Trap to 466. Explosive Trap
  sims 1 to 2% behind Immolation Trap. Upstream's `sv.apl.json`
  is untouched; it is the ranged Survival rotation.
- **Tests.** `sim/hunter/survival_melee_test.go` (`TestSurvivalMelee`, 5 yards,
  `SurvivalMeleeTalents` = wowtbc.gg's 5/10/35 with the spare point in Focused Fire) and its golden
  `TestSurvivalMelee.results` (average 398.57 DPS on the suite's weapons-only gear).
- **Drop it when** upstream ships a melee Survival rotation. Compare the two on the golden first.

## 6. `items: Iceblade Hacker and Warblade of Caer Darrow proc from their own hand`

- **What it does.** The two weapon procs in `sim/common/classic/items_store_gaps.go` ("Melee
  attacks with this weapon deal 41 / 28 Frost damage") get a
  `NewDynamicLegacyProcForWeapon(item, 0, 1)` proc manager, as every generated weapon proc has.
  Their proc masks alone named both hands' autos and specials. A main-hand Iceblade Hacker also
  procced off every off-hand swing, which was worth 12.5% of a dual-wielding melee hunter's damage.
- **Tests.** `sim/rogue/weapon_proc_hand_test.go` (`TestIcebladeHackerProcsOnlyFromItsHand`). The
  AllItems rows for the two weapons move in `TestFury`, `TestArms`, `TestProtectionWarrior` and
  `TestRetribution`: the harness equips Warblade in the off hand beside a main-hand weapon, where
  it used to proc off main-hand hits too (112 procs a fight on Arms, 31 now). No other row moves.
- **Drop it when** upstream's generator carries these procs (the file says to remove an entry
  then), or upstream scopes them to their hand. A generated `CreateWeaponCoHProcDamage` already
  does.

## Rebasing onto a newer upstream

1. Fetch ElliotWood/Forever master. Rebase the patches onto it:
   `git rebase --onto <new upstream> <old base> <fork branch>`.
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
   go test --tags=with_db ./sim/hunter -run 'Aspect|QuickStrikes|SurvivalMelee'
   go test --tags=with_db ./sim/rogue -run IcebladeHacker
   ```

4. Run the full suite, `go test --tags=with_db $(go list ./sim/... | grep -v sim/web)`. Only
   `sim/warlock/TestDestruction` should fail, from patch 3's Conflagrate, and
   `sim/hunter/TestSurvivalMelee` wherever upstream moved hunter numbers (re-bless it into patch 5).
   If upstream's AllItems rows for Iceblade Hacker or Warblade of Caer Darrow move, re-bless them
   into patch 6. Re-bless it (copy
   `TestDestruction.results.tmp` over `TestDestruction.results`) and fold it into patch 3, so
   the patch carries the golden it moves. Any other failure is upstream's or the rebase's, not a
   golden to re-bless.
5. Check each patch's "drop it when" condition above, and update this file when a patch goes.
